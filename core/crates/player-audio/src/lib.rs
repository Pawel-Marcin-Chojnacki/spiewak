//! Streaming playback on a dedicated Rust worker. Rodio uses Symphonia + CPAL/WASAPI.
use player_domain::{Queue, Track};
use rodio::{Decoder, DeviceSinkBuilder, Player};
use std::{
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
}
type Request = (Command, mpsc::Sender<Result<(), String>>);
pub struct Audio {
    sender: mpsc::Sender<Request>,
    state: Arc<Mutex<Playback>>,
    worker: Option<thread::JoinHandle<()>>,
}
impl Default for Audio {
    fn default() -> Self {
        Self::new()
    }
}
impl Audio {
    pub fn new() -> Self {
        let (sender, receiver) = mpsc::channel::<Request>();
        let state = Arc::new(Mutex::new(Playback {
            volume: 0.75,
            ..Default::default()
        }));
        let shared = state.clone();
        let worker = thread::spawn(move || {
            let mut queue = Queue::default();
            let mut device = None;
            let mut player: Option<Player> = None;
            let mut volume = 0.75;
            let mut muted = false;
            let mut repeat = 0;
            let mut auto_advance = false;
            let mut changed = false;
            loop {
                let request = match receiver.recv_timeout(Duration::from_millis(50)) {
                    Ok(request) => Some(request),
                    Err(mpsc::RecvTimeoutError::Timeout) => None,
                    Err(_) => break,
                };
                let mut error = None;
                if let Some((command, reply)) = request {
                    let previous_queue = queue.clone();
                    let result = (|| -> Result<(), String> {
                        let mut load = false;
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
                                if let Some(p) = &player
                                    && p.get_pos().as_secs_f64() > 3.0
                                {
                                    p.try_seek(Duration::ZERO).map_err(|e| e.to_string())?;
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
                                }
                            }
                            Command::Stop => {
                                auto_advance = false;
                                if let Some(p) = &player {
                                    p.stop()
                                }
                            }
                            Command::Seek(position) => {
                                if !position.is_finite() || position < 0.0 {
                                    return Err("Invalid playback position.".into());
                                }
                                if let Some(p) = &player {
                                    p.try_seek(Duration::from_secs_f64(position.min(
                                        queue.selected().map(|t| t.duration).unwrap_or(position),
                                    )))
                                    .map_err(|e| format!("Cannot seek this file: {e}"))?;
                                }
                            }
                            Command::Append(tracks, next) => queue.append(tracks, next),
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
                            next.set_volume(if muted { 0.0 } else { volume });
                            next.append(source);
                            if let Some(p) = player.take() {
                                p.stop()
                            }
                            player = Some(next);
                            auto_advance = true;
                        }
                        Ok(())
                    })();
                    if let Err(e) = &result {
                        queue = previous_queue.clone();
                        error = Some(e.clone())
                    }
                    changed = queue.current != previous_queue.current
                        || !Arc::ptr_eq(&queue.tracks, &previous_queue.tracks);
                    if let Ok(mut s) = shared.lock() {
                        s.error = error.clone().unwrap_or_default();
                    }
                    let _ = reply.send(result);
                } else if auto_advance
                    && player.as_ref().is_some_and(|p| p.empty() && !p.is_paused())
                    && queue.advance(true, repeat).is_some()
                {
                    changed = true;
                    let track = queue.selected().unwrap();
                    match File::open(&track.path)
                        .map_err(|e| e.to_string())
                        .and_then(|f| Decoder::try_from(f).map_err(|e| e.to_string()))
                    {
                        Ok(source) => {
                            let next = Player::connect_new(device.as_ref().unwrap().mixer());
                            next.set_volume(if muted { 0.0 } else { volume });
                            next.append(source);
                            player = Some(next);
                        }
                        Err(e) => error = Some(format!("Cannot play next track: {e}")),
                    }
                    if let Ok(mut s) = shared.lock()
                        && let Some(e) = error
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
                    s.volume = volume;
                    s.flags = u32::from(muted) | (repeat << 1);
                    s.playing = player
                        .as_ref()
                        .is_some_and(|p| !p.is_paused() && !p.empty());
                    s.position = player
                        .as_ref()
                        .map(|p| p.get_pos().as_secs_f64())
                        .unwrap_or(0.0);
                }
            }
        });
        Self {
            sender,
            state,
            worker: Some(worker),
        }
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
                revision: s.revision,
                error: s.error.clone(),
                flags: s.flags,
                queue: Arc::from([]),
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
