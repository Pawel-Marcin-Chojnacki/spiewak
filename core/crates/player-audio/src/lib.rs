//! Streaming playback on a dedicated Rust worker. Rodio uses Symphonia + CPAL/WASAPI.
use player_domain::{Listen, Queue, Session, Track};
use rodio::{Decoder, DeviceSinkBuilder, Player};
use std::{
    collections::VecDeque,
    fs::File,
    sync::{Arc, Mutex, mpsc},
    thread,
    time::Duration,
};

#[derive(Clone, Default)]
pub struct Playback {
    pub track: Option<Track>,
    pub position: f64,
    pub playing: bool,
    pub volume: f32,
    pub queue: Arc<[Track]>,
    pub current: Option<usize>,
    pub next_end: Option<usize>,
    pub revision: u64,
    pub error: String,
    pub flags: u32,
}
pub enum Command {
    Replace(Vec<Track>, usize),
    Toggle,
    Stop,
    Next,
    Previous,
    Seek(f64),
    Volume(f32),
    Append(Vec<Track>, bool),
    Mute,
    Repeat,
    Shuffle,
    Remove(usize),
    Move(usize, bool),
    Clear,
    Jump(usize),
    Refresh(Vec<Track>),
}
type Request = (Command, mpsc::Sender<Result<(), String>>);
#[derive(Default)]
struct StartLog {
    serial: u64,
    dropped: u64,
    pending: VecDeque<(u64, Listen)>,
}
impl StartLog {
    fn push(&mut self, track: Track) -> Result<(), String> {
        if self.pending.len() >= 1000 {
            self.dropped = self.dropped.saturating_add(1);
            return Err(
                "Listening history buffer is full. Some starts were not saved; playback continues."
                    .into(),
            );
        }
        self.serial += 1;
        self.pending.push_back((
            self.serial,
            Listen {
                track,
                started_at: std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs() as i64,
            },
        ));
        Ok(())
    }
    fn acknowledge(&mut self, serial: u64) {
        while self.pending.front().is_some_and(|(id, _)| *id <= serial) {
            self.pending.pop_front();
        }
    }
}
pub struct Audio {
    sender: mpsc::Sender<Request>,
    state: Arc<Mutex<Playback>>,
    worker: Option<thread::JoinHandle<()>>,
    starts: Arc<Mutex<StartLog>>,
}
impl Default for Audio {
    fn default() -> Self {
        Self::new()
    }
}
impl Audio {
    pub fn new() -> Self {
        Self::restore(Session::default()).expect("Default session is valid")
    }
    /// Restore paused without opening files or an audio device until explicit playback.
    pub fn restore(session: Session) -> Result<Self, String> {
        let mut queue = Queue::default();
        queue.restore(&session)?;
        let (sender, receiver) = mpsc::channel::<Request>();
        let state = Arc::new(Mutex::new(Playback {
            track: queue.selected().cloned(),
            queue: queue.tracks.clone(),
            current: queue.current,
            next_end: queue.priority_end(),
            position: session.position,
            volume: session.volume,
            flags: session.flags,
            revision: 1,
            ..Default::default()
        }));
        let starts = Arc::new(Mutex::new(StartLog::default()));
        let worker_starts = starts.clone();
        let shared = state.clone();
        let worker = thread::spawn(move || {
            let mut device = None;
            let mut player: Option<Player> = None;
            let mut volume = session.volume;
            let mut muted = session.flags & 1 != 0;
            let mut repeat = session.flags >> 1;
            let mut resume_position = session.position;
            let mut auto_advance = false;
            let mut changed = false;
            loop {
                let request = match receiver.recv_timeout(Duration::from_millis(50)) {
                    Ok(request) => Some(request),
                    Err(mpsc::RecvTimeoutError::Timeout) => None,
                    Err(_) => break,
                };
                let mut error = None;
                let mut started = None;
                let mut acknowledgement = None;
                if let Some((command, reply)) = request {
                    let previous_queue = queue.clone();
                    let previous_position = resume_position;
                    let result = (|| -> Result<(), String> {
                        let mut load = false;
                        let mut load_position = 0.0;
                        match command {
                            Command::Replace(tracks, start) => {
                                queue.replace(tracks, start)?;
                                load = true
                            }
                            Command::Next => {
                                if queue.advance(false, repeat).is_some() {
                                    load = true
                                } else if let Some(p) = &player {
                                    auto_advance = false;
                                    p.stop();
                                }
                            }
                            Command::Previous => {
                                if player
                                    .as_ref()
                                    .map(|p| p.get_pos().as_secs_f64())
                                    .unwrap_or(resume_position)
                                    > 3.0
                                {
                                    if let Some(p) = &player {
                                        p.try_seek(Duration::ZERO).map_err(|e| e.to_string())?;
                                    } else {
                                        resume_position = 0.0;
                                    }
                                    return Ok(());
                                }
                                if queue.previous().is_some() {
                                    load = true
                                }
                            }
                            Command::Toggle => {
                                if let Some(p) = &player {
                                    if p.empty() {
                                        load = true
                                    } else if p.is_paused() {
                                        p.play()
                                    } else {
                                        p.pause()
                                    }
                                } else {
                                    if queue.current.is_none() && !queue.tracks.is_empty() {
                                        queue.jump(0)?;
                                    }
                                    load = true;
                                    load_position = resume_position;
                                }
                            }
                            Command::Stop => {
                                auto_advance = false;
                                resume_position = 0.0;
                                if let Some(p) = player.take() {
                                    p.stop()
                                }
                            }
                            Command::Seek(position) => {
                                if !position.is_finite() || position < 0.0 {
                                    return Err("Invalid playback position.".into());
                                }
                                let track = queue.selected().ok_or("Choose a track first.")?;
                                let position = if track.duration > 0.0 {
                                    position.min(track.duration)
                                } else {
                                    position
                                };
                                if let Some(p) = &player {
                                    p.try_seek(
                                        Duration::try_from_secs_f64(position)
                                            .map_err(|_| "Playback position is too large.")?,
                                    )
                                    .map_err(|e| format!("Cannot seek this file: {e}"))?;
                                } else {
                                    resume_position = position;
                                }
                            }
                            Command::Append(tracks, next) => queue.append(tracks, next),
                            Command::Refresh(tracks) => {
                                let refreshed = tracks
                                    .into_iter()
                                    .filter(|t| t.id != 0)
                                    .map(|t| (t.id, t))
                                    .collect::<std::collections::HashMap<_, _>>();
                                queue.tracks = queue
                                    .tracks
                                    .iter()
                                    .map(|t| refreshed.get(&t.id).unwrap_or(t).clone())
                                    .collect::<Vec<_>>()
                                    .into();
                            }
                            Command::Mute => {
                                muted = !muted;
                                if let Some(p) = &player {
                                    p.set_volume(if muted { 0.0 } else { volume });
                                }
                            }
                            Command::Repeat => repeat = (repeat + 1) % 3,
                            Command::Shuffle => queue.shuffle(
                                std::time::SystemTime::now()
                                    .duration_since(std::time::UNIX_EPOCH)
                                    .unwrap_or_default()
                                    .as_nanos() as u64,
                            ),
                            Command::Remove(index) => queue.remove(index)?,
                            Command::Move(index, up) => queue.move_item(index, up)?,
                            Command::Clear => queue.clear_upcoming(),
                            Command::Jump(index) => {
                                queue.jump(index)?;
                                load = true;
                            }
                            Command::Volume(value) => {
                                if !value.is_finite() {
                                    return Err("Invalid volume.".into());
                                }
                                volume = value.clamp(0.0, 1.0);
                                if let Some(p) = &player {
                                    p.set_volume(if muted { 0.0 } else { volume })
                                }
                            }
                        }
                        if load {
                            let track = queue.selected().ok_or("Choose a track first.")?;
                            if track.id == 0 {
                                return Err("This queued file is unavailable. Locate it in your folders before playing.".into());
                            }
                            // Decode/open before stopping the current source: failed selections do not cut audio.
                            let file = File::open(&track.path)
                                .map_err(|e| format!("Cannot open {}: {e}", track.path))?;
                            let source = Decoder::try_from(file)
                                .map_err(|e| format!("Cannot decode {}: {e}", track.path))?;
                            if device.is_none() {
                                device =
                                    Some(DeviceSinkBuilder::open_default_sink().map_err(|e| {
                                        format!("Cannot open the default audio device: {e}")
                                    })?)
                            }
                            let next = Player::connect_new(device.as_ref().unwrap().mixer());
                            next.pause();
                            next.set_volume(if muted { 0.0 } else { volume });
                            next.append(source);
                            if load_position > 0.0 {
                                let position = if track.duration > 0.0 {
                                    load_position.min(track.duration)
                                } else {
                                    load_position
                                };
                                next.try_seek(
                                    Duration::try_from_secs_f64(position)
                                        .map_err(|_| "Saved playback position is too large.")?,
                                )
                                .map_err(|e| format!("Cannot resume this file: {e}"))?;
                            }
                            if let Some(p) = player.take() {
                                p.stop()
                            }
                            next.play();
                            player = Some(next);
                            resume_position = 0.0;
                            auto_advance = true;
                            started = Some(track.clone());
                        }
                        Ok(())
                    })();
                    if let Err(e) = &result {
                        queue = previous_queue.clone();
                        resume_position = previous_position;
                        error = Some(e.clone())
                    }
                    changed = queue.current != previous_queue.current
                        || !Arc::ptr_eq(&queue.tracks, &previous_queue.tracks);
                    if let Ok(mut s) = shared.lock() {
                        s.error = error.clone().unwrap_or_default();
                    }
                    acknowledgement = Some((reply, result));
                } else if auto_advance
                    && player.as_ref().is_some_and(|p| p.empty() && !p.is_paused())
                    && queue.advance(true, repeat).is_some()
                {
                    changed = true;
                    let track = queue.selected().unwrap();
                    match (if track.id == 0 {
                        Err("This queued file is unavailable.".to_owned())
                    } else {
                        File::open(&track.path).map_err(|e| e.to_string())
                    })
                    .and_then(|f| Decoder::try_from(f).map_err(|e| e.to_string()))
                    {
                        Ok(source) => {
                            let next = Player::connect_new(device.as_ref().unwrap().mixer());
                            next.set_volume(if muted { 0.0 } else { volume });
                            next.append(source);
                            player = Some(next);
                            started = Some(track.clone());
                        }
                        Err(e) => {
                            auto_advance = false;
                            player = None;
                            resume_position = 0.0;
                            error = Some(format!("Cannot play next track: {e}"));
                        }
                    }
                    if let Ok(mut s) = shared.lock()
                        && let Some(e) = error
                    {
                        s.error = e;
                    }
                }
                if let Some(track) = started {
                    let result = worker_starts
                        .lock()
                        .map_err(|_| "Listening history buffer unavailable.".to_owned())
                        .and_then(|mut log| log.push(track));
                    if let Err(e) = result
                        && let Ok(mut s) = shared.lock()
                    {
                        s.error = e;
                    }
                }
                if let Ok(mut s) = shared.lock() {
                    if changed {
                        s.revision += 1;
                        s.track = queue.selected().cloned();
                        s.queue = queue.tracks.clone();
                        s.current = queue.current;
                        changed = false;
                    }
                    s.next_end = queue.priority_end();
                    s.volume = volume;
                    s.flags = u32::from(muted) | (repeat << 1);
                    s.playing = player
                        .as_ref()
                        .is_some_and(|p| !p.is_paused() && !p.empty());
                    s.position = player
                        .as_ref()
                        .map(|p| p.get_pos().as_secs_f64())
                        .unwrap_or(resume_position);
                }
                // A successful command is acknowledged only after its complete state is published.
                if let Some((reply, result)) = acknowledgement {
                    let _ = reply.send(result);
                }
            }
        });
        Ok(Self {
            sender,
            state,
            worker: Some(worker),
            starts,
        })
    }
    pub fn history_warning(&self) -> Result<String, String> {
        self.starts.lock().map(|s| if s.dropped==0 {String::new()} else {format!("Listening history could not buffer {} playback starts. Those entries are missing; playback can continue.",s.dropped)}).map_err(|_|"Listening history buffer unavailable.".into())
    }
    pub fn pending_starts(&self) -> Result<Vec<(u64, Listen)>, String> {
        self.starts
            .lock()
            .map(|s| s.pending.iter().cloned().collect())
            .map_err(|_| "Listening history buffer unavailable.".into())
    }
    pub fn acknowledge_starts(&self, serial: u64) -> Result<(), String> {
        self.starts
            .lock()
            .map(|mut s| s.acknowledge(serial))
            .map_err(|_| "Listening history buffer unavailable.".into())
    }
    pub fn command(&self, command: Command) -> Result<(), String> {
        let (tx, rx) = mpsc::channel();
        self.sender
            .send((command, tx))
            .map_err(|_| "Audio worker unavailable.")?;
        rx.recv_timeout(Duration::from_secs(15))
            .map_err(|_| "Audio command timed out.".to_owned())?
    }
    pub fn snapshot(&self) -> Result<Playback, String> {
        self.state
            .lock()
            .map(|s| Playback {
                track: s.track.clone(),
                position: s.position,
                playing: s.playing,
                volume: s.volume,
                current: s.current,
                next_end: s.next_end,
                revision: s.revision,
                error: s.error.clone(),
                flags: s.flags,
                queue: Arc::from([]),
            })
            .map_err(|_| "Playback state unavailable.".into())
    }
    pub fn session(&self) -> Result<Session, String> {
        self.state
            .lock()
            .map(|s| Session {
                tracks: s.queue.clone(),
                current: s.current,
                next_end: s.next_end,
                position: s.position,
                volume: s.volume,
                flags: s.flags,
            })
            .map_err(|_| "Playback state unavailable.".into())
    }
    pub fn queue(&self) -> Result<Vec<Track>, String> {
        self.state
            .lock()
            .map(|s| s.queue.to_vec())
            .map_err(|_| "Queue unavailable.".into())
    }
}
impl Drop for Audio {
    fn drop(&mut self) {
        let (tx, _) = mpsc::channel();
        let old = std::mem::replace(&mut self.sender, tx);
        drop(old);
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn start_log_is_bounded_and_acknowledges_only_the_committed_prefix() {
        let mut log = StartLog::default();
        let t = Track {
            id: 1,
            ..Default::default()
        };
        for _ in 0..1000 {
            log.push(t.clone()).unwrap();
        }
        assert!(log.push(t.clone()).is_err());
        assert_eq!(log.pending.len(), 1000);
        assert_eq!(log.dropped, 1);
        log.acknowledge(999);
        log.push(t).unwrap();
        log.acknowledge(1000);
        assert_eq!(log.pending.len(), 1);
        assert_eq!(log.pending[0].0, 1001);
        let audio = Audio::new();
        *audio.starts.lock().unwrap() = log;
        audio.acknowledge_starts(1001).unwrap();
        audio.command(Command::Volume(0.5)).unwrap();
        assert!(audio.pending_starts().unwrap().is_empty());
        assert!(
            audio
                .history_warning()
                .unwrap()
                .contains("1 playback starts")
        );
    }
    #[test]
    fn restore_is_lazy_paused_and_retains_fifo_boundary() {
        let a = Track {
            id: 1,
            path: "missing-session-fixture.wav".into(),
            duration: 10.0,
            ..Default::default()
        };
        let b = Track { id: 2, ..a.clone() };
        let audio = Audio::restore(Session {
            tracks: vec![a.clone(), b, a.clone()].into(),
            current: Some(0),
            next_end: Some(2),
            position: 3.0,
            volume: 0.2,
            flags: 5,
        })
        .unwrap();
        assert!(audio.pending_starts().unwrap().is_empty());
        let before = audio.snapshot().unwrap();
        assert!(!before.playing);
        assert_eq!(before.position, 3.0);
        assert_eq!(before.flags, 5);
        audio.command(Command::Append(vec![a], true)).unwrap();
        assert_eq!(
            audio
                .queue()
                .unwrap()
                .iter()
                .map(|t| t.id)
                .collect::<Vec<_>>(),
            vec![1, 2, 1, 1]
        );
        audio.command(Command::Seek(4.0)).unwrap();
        assert_eq!(audio.session().unwrap().position, 4.0);
        assert!(audio.command(Command::Toggle).is_err());
        assert!(audio.pending_starts().unwrap().is_empty());
        assert_eq!(audio.session().unwrap().position, 4.0);
        assert_eq!(audio.session().unwrap().next_end, Some(3));
        audio.command(Command::Stop).unwrap();
        assert_eq!(audio.snapshot().unwrap().position, 0.0);
    }
}

#[cfg(test)]
mod missing_tests {
    use super::*;
    #[test]
    fn metadata_refresh_keeps_distinct_missing_occurrences() {
        let one = Track {
            id: 0,
            path: "missing-one.wav".into(),
            ..Default::default()
        };
        let two = Track {
            id: 0,
            path: "missing-two.wav".into(),
            ..Default::default()
        };
        let audio = Audio::restore(Session {
            tracks: vec![one.clone(), two.clone()].into(),
            ..Default::default()
        })
        .unwrap();
        audio.command(Command::Refresh(vec![one, two])).unwrap();
        assert_eq!(
            audio
                .queue()
                .unwrap()
                .iter()
                .map(|t| t.path.as_str())
                .collect::<Vec<_>>(),
            vec!["missing-one.wav", "missing-two.wav"]
        );
    }
}
