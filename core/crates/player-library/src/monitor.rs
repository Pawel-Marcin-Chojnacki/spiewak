//! Debounced native notifications plus a periodic reconciliation safety net.
use crate::{Library, ScanReport};
use notify::{EventKind, RecursiveMode, Watcher};
use std::{
    collections::BTreeSet,
    path::PathBuf,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    thread,
    time::{Duration, Instant},
};
#[derive(Clone, Default)]
pub struct MonitorState {
    pub revision: u64,
    pub message: String,
    pub ready: bool,
}
pub struct Monitor {
    state: Arc<Mutex<MonitorState>>,
    stop: Arc<AtomicBool>,
    worker: Option<thread::JoinHandle<()>>,
}
struct ScanLock(Arc<AtomicBool>);
impl Drop for ScanLock {
    fn drop(&mut self) {
        self.0.store(false, Ordering::Release);
    }
}
fn publish(state: &Mutex<MonitorState>, message: String) {
    if let Ok(mut s) = state.lock() {
        s.revision += 1;
        s.message = message;
    }
}
fn report_message(report: &ScanReport) -> String {
    format!(
        "{} tracks indexed{}",
        report.tracks,
        if report.warnings.is_empty() {
            String::new()
        } else {
            format!(" · {}", report.warnings.join("; "))
        }
    )
}
fn relevant_event(event: &notify::Event, database: &std::path::Path) -> bool {
    if event.need_rescan() {
        return true;
    }
    if matches!(event.kind, EventKind::Access(_)) {
        return false;
    }
    let db = database.to_string_lossy();
    event.paths.iter().any(|p| {
        let text = p.to_string_lossy();
        !p.starts_with(database.with_extension("artwork"))
            && !text.eq_ignore_ascii_case(&db)
            && !text.eq_ignore_ascii_case(&format!("{db}-wal"))
            && !text.eq_ignore_ascii_case(&format!("{db}-shm"))
            && !p
                .file_name()
                .is_some_and(|n| n.to_string_lossy().starts_with("._"))
    })
}
impl Monitor {
    pub fn start(path: PathBuf, scanning: Arc<AtomicBool>) -> Result<Self, String> {
        let state = Arc::new(Mutex::new(MonitorState::default()));
        let stop = Arc::new(AtomicBool::new(false));
        let shared = state.clone();
        let stopped = stop.clone();
        let worker = thread::Builder::new().name("library-watch".into()).spawn(move || {
            let library = match Library::open(&path) { Ok(l) => l, Err(e) => { publish(&shared, e); return; } };
            let (tx, rx) = mpsc::channel();
            let mut watcher = match notify::recommended_watcher(tx) {
                Ok(w) => Some(w), Err(e) => { publish(&shared, format!("Live folder monitoring unavailable: {e}. Periodic scans remain active.")); None }
            };
            let mut watched = BTreeSet::new();
            let mut configured = Vec::new();
            let mut available = BTreeSet::new();
            let mut watch_attempt = Instant::now() - Duration::from_secs(31);
            let mut roots_at = Instant::now() - Duration::from_secs(3);
            let mut periodic_at = Instant::now();
            let mut inbox_at = Instant::now();
            let mut dirty: Option<(Instant, Instant)> = None;
            while !stopped.load(Ordering::Acquire) {
                match rx.recv_timeout(Duration::from_millis(200)) {
                    Ok(event) => {
                        let relevant = match event {
                            Ok(event) => relevant_event(&event, &path),
                            Err(e) => { publish(&shared, format!("Folder notification failed: {e}. Reconciling the cached library.")); true }
                        };
                        if relevant { let now = Instant::now(); dirty = Some((dirty.map_or(now, |d|d.0), now)); }
                    }
                    Err(mpsc::RecvTimeoutError::Timeout) => {},
                    Err(mpsc::RecvTimeoutError::Disconnected) => { thread::sleep(Duration::from_millis(200)); }
                }
                if roots_at.elapsed() >= Duration::from_secs(2) {
                    roots_at = Instant::now();
                    if let Ok(roots) = library.roots() {
                        let desired = roots.iter().map(PathBuf::from).filter(|p|p.is_dir()).collect::<BTreeSet<_>>();
                        let changed = roots != configured || desired != available;
                        let retry = desired != watched && watch_attempt.elapsed() >= Duration::from_secs(30);
                        if changed || retry {
                            watch_attempt = Instant::now();
                            if let Some(w) = &mut watcher {
                                for old in watched.difference(&desired) { let _ = w.unwatch(old); }
                                watched.retain(|p|desired.contains(p));
                                for added in desired.difference(&watched).cloned().collect::<Vec<_>>() {
                                    match w.watch(&added, RecursiveMode::Recursive) {
                                        Ok(()) => { watched.insert(added); },
                                        Err(e) => publish(&shared, format!("Cannot monitor {}: {e}. Periodic scans remain active.", added.display()))
                                    }
                                }
                            }
                            configured = roots;
                            available = desired;
                            if changed || retry && watched == available {
                                let now = Instant::now(); dirty = Some((now, now));
                            }
                        }
                        if let Ok(mut s) = shared.lock() { s.ready = !configured.is_empty() && watched.len() == configured.len(); }
                    }
                }
                if inbox_at.elapsed() >= Duration::from_secs(2) && !scanning.swap(true, Ordering::AcqRel) {
                    let _guard = ScanLock(scanning.clone());
                    match library.refresh_inbox() {
                        Ok(true) => publish(&shared, "Inbox updated. New files stay in their folders.".into()),
                        Ok(false) => {},
                        Err(e) => publish(&shared, format!("Inbox check failed: {e}. Your music was not changed."))
                    }
                    inbox_at = Instant::now();
                }
                let due = dirty.is_some_and(|(first,last)|last.elapsed() >= Duration::from_millis(750) || first.elapsed() >= Duration::from_secs(8))
                    || periodic_at.elapsed() >= Duration::from_secs(60);
                if due && !scanning.swap(true, Ordering::AcqRel) {
                    let _guard = ScanLock(scanning.clone());
                    match library.reconcile() {
                        Ok(report) => {
                            let message = report_message(&report);
                            let status_changed = shared.lock().is_ok_and(|s|s.message != message);
                            if report.changed || status_changed { publish(&shared, message); }
                        },
                        Err(e) => publish(&shared, format!("Library synchronization failed: {e}"))
                    }
                    dirty = None; periodic_at = Instant::now();
                }
            }
        }).map_err(|e|e.to_string())?;
        Ok(Self {
            state,
            stop,
            worker: Some(worker),
        })
    }
    pub fn state(&self) -> MonitorState {
        self.state.lock().map(|s| s.clone()).unwrap_or_default()
    }
    pub fn notice(&self, message: &str) {
        publish(&self.state, message.to_owned());
    }
    pub fn scanned(&self, report: &ScanReport) -> String {
        let message = report_message(report);
        publish(&self.state, message.clone());
        message
    }
}
impl Drop for Monitor {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn notification_overflow_requests_reconciliation_without_a_path() {
        let event = notify::Event::new(EventKind::Other).set_flag(notify::event::Flag::Rescan);
        assert!(relevant_event(&event, std::path::Path::new("cache.db")));
        let own_write = notify::Event::new(EventKind::Any).add_path(PathBuf::from("cache.db-wal"));
        assert!(!relevant_event(
            &own_write,
            std::path::Path::new("cache.db")
        ));
    }
}
