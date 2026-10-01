//! Owned provider snapshots. HTTP never holds the UI library, scan or audio locks.
use super::*;
use player_library::{Candidate, Discovery};
#[repr(C)]
pub struct CandidateDto {
    id: i64,
    track_id: i64,
    flags: u32,
    score: u32,
    duration: f64,
    title: *const c_char,
    artist: *const c_char,
    album: *const c_char,
    date: *const c_char,
    edition: *const c_char,
    evidence: *const c_char,
    source: *const c_char,
    artwork: *const c_char,
    recording: *const c_char,
    release: *const c_char,
}
#[repr(C)]
#[derive(Default)]
pub struct DiscoverySnapshot {
    error: *const c_char,
    message: *const c_char,
    enabled: u32,
    rows: *const CandidateDto,
    count: usize,
    title: *const c_char,
    artist: *const c_char,
    strings: Vec<CString>,
    records: Vec<CandidateDto>,
}
impl DiscoverySnapshot {
    fn string(&mut self, value: &str) -> *const c_char {
        let string = CString::new(value.replace('\0', "�")).expect("NULs removed");
        let ptr = string.as_ptr();
        self.strings.push(string);
        ptr
    }
    fn records(mut self, discovery: Discovery) -> Self {
        self.message = self.string(&discovery.message);
        self.enabled = u32::from(discovery.enabled);
        self.title = self.string(&discovery.title);
        self.artist = self.string(&discovery.artist);
        for row in discovery.candidates {
            let Candidate {
                id,
                track_id,
                flags,
                score,
                duration,
                title,
                artist,
                album,
                date,
                edition,
                evidence,
                source,
                artwork,
                recording,
                release,
            } = row;
            let record = CandidateDto {
                id,
                track_id,
                flags,
                score,
                duration,
                title: self.string(&title),
                artist: self.string(&artist),
                album: self.string(&album),
                date: self.string(&date),
                edition: self.string(&edition),
                evidence: self.string(&evidence),
                source: self.string(&source),
                artwork: self.string(&artwork),
                recording: self.string(&recording),
                release: self.string(&release),
            };
            self.records.push(record);
        }
        self.rows = self.records.as_ptr();
        self.count = self.records.len();
        self
    }
}
fn result(action: impl FnOnce() -> Result<Discovery, String>) -> *mut DiscoverySnapshot {
    let mut snapshot = DiscoverySnapshot::default();
    match std::panic::catch_unwind(std::panic::AssertUnwindSafe(action)) {
        Ok(Ok(discovery)) => snapshot = snapshot.records(discovery),
        Ok(Err(e)) => snapshot.error = snapshot.string(&e),
        Err(_) => {
            snapshot.error = snapshot.string(
                "Online discovery encountered an internal error. Your music was not changed.",
            )
        }
    }
    Box::into_raw(Box::new(snapshot))
}
/// # Safety
/// Live core; UTF-8 artist/title for action 1. Actions 0 cached view, 1 search, 2 enable online requests, 3 disable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn spiewak_discovery(
    ptr: *mut Core,
    track: i64,
    action: u32,
    title: *const c_char,
    artist: *const c_char,
) -> *mut DiscoverySnapshot {
    result(|| {
        let c = unsafe { core(ptr) }?;
        let lib = Library::open(&c.path)?;
        match action {
            0 => lib.discovery(track),
            1 => lib.discover_metadata(track, unsafe { text(title) }?, unsafe { text(artist) }?),
            2 | 3 => {
                lib.set_discovery_enabled(action == 2)?;
                lib.discovery(track)
            }
            _ => Err("Unknown discovery action.".into()),
        }
    })
}
/// # Safety
/// Live core and a cached candidate ID. Booleans are 0 or 1; preview only, never writes the source file.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn spiewak_discovery_prepare(
    ptr: *mut Core,
    candidate: i64,
    replace_tags: u32,
    replace_cover: u32,
) -> *mut metadata::MetadataSnapshot {
    metadata::result(|| {
        if replace_tags > 1 || replace_cover > 1 {
            return Err("Invalid discovery option.".into());
        }
        let c = unsafe { core(ptr) }?;
        let lib = Library::open(&c.path)?;
        let proposal = lib.discovery_proposal(candidate, replace_tags != 0, replace_cover != 0)?;
        metadata::library(c, |lib| lib.prepare_discovery(proposal))
    })
}
/// # Safety
/// Snapshot returned by this API, released exactly once after copying its strings/rows.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn spiewak_discovery_free(ptr: *mut DiscoverySnapshot) {
    if !ptr.is_null() {
        drop(unsafe { Box::from_raw(ptr) });
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn discovery_layout_owns_strings_and_handles_invalid_core() {
        assert_eq!(std::mem::size_of::<CandidateDto>(), 112);
        assert_eq!(std::mem::offset_of!(DiscoverySnapshot, strings), 56);
        let p = result(|| {
            Ok(Discovery {
                enabled: true,
                title: "Żółć".into(),
                candidates: vec![Candidate {
                    title: "Śpiew".into(),
                    release: "edition".into(),
                    ..Default::default()
                }],
                ..Default::default()
            })
        });
        unsafe {
            assert_eq!((*p).count, 1);
            assert_eq!(text((*p).title).unwrap(), "Żółć");
            assert_eq!(text((*(*p).rows).title).unwrap(), "Śpiew");
            spiewak_discovery_free(p);
        }
        let p = unsafe {
            spiewak_discovery(
                std::ptr::null_mut(),
                0,
                0,
                std::ptr::null(),
                std::ptr::null(),
            )
        };
        unsafe {
            assert!(!(*p).error.is_null());
            spiewak_discovery_free(p);
        }
    }
}
