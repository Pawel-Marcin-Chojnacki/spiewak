//! Typed C ABI. All returned memory belongs to a snapshot, released exactly once.
use player_audio::{Audio, Command};
use player_domain::{Folder, Track};
use player_library::Library;
use std::{
    ffi::{CStr, CString, c_char},
    path::PathBuf,
    sync::{
        Mutex,
        atomic::{AtomicBool, Ordering},
    },
};

pub struct Core {
    library: Mutex<Library>,
    path: PathBuf,
    audio: Audio,
    scanning: AtomicBool,
}
#[repr(C)]
pub struct FolderDto {
    id: i64,
    parent: i64,
    name: *const c_char,
    path: *const c_char,
    has_children: u32,
}
#[repr(C)]
pub struct TrackDto {
    id: i64,
    folder: i64,
    title: *const c_char,
    artist: *const c_char,
    album: *const c_char,
    path: *const c_char,
    artwork: *const c_char,
    codec: *const c_char,
    duration: f64,
    number: u32,
    year: u32,
}
#[repr(C)]
pub struct Snapshot {
    folders: *const FolderDto,
    folder_count: usize,
    tracks: *const TrackDto,
    track_count: usize,
    position: f64,
    current_id: i64,
    current_index: i64,
    revision: u64,
    playing: u32,
    volume: f32,
    flags: u32,
    error: *const c_char,
    message: *const c_char,
    strings: Vec<CString>,
    folder_rows: Vec<FolderDto>,
    track_rows: Vec<TrackDto>,
}
impl Default for Snapshot {
    fn default() -> Self {
        Self {
            folders: std::ptr::null(),
            folder_count: 0,
            tracks: std::ptr::null(),
            track_count: 0,
            position: 0.0,
            current_id: 0,
            current_index: -1,
            revision: 0,
            playing: 0,
            volume: 0.75,
            flags: 0,
            error: std::ptr::null(),
            message: std::ptr::null(),
            strings: Vec::new(),
            folder_rows: Vec::new(),
            track_rows: Vec::new(),
        }
    }
}
impl Snapshot {
    fn string(&mut self, value: &str) -> *const c_char {
        let string = CString::new(value.replace('\0', "�")).expect("NULs removed");
        let ptr = string.as_ptr();
        self.strings.push(string);
        ptr
    }
    fn folders(mut self, rows: Vec<Folder>) -> Self {
        for f in rows {
            let name = self.string(&f.name);
            let path = self.string(&f.path);
            self.folder_rows.push(FolderDto {
                id: f.id,
                parent: f.parent_id,
                name,
                path,
                has_children: u32::from(f.has_children),
            });
        }
        self.folder_count = self.folder_rows.len();
        self.folders = self.folder_rows.as_ptr();
        self
    }
    fn tracks(mut self, rows: Vec<Track>) -> Self {
        for t in rows {
            let title = self.string(&t.title);
            let artist = self.string(&t.artist);
            let album = self.string(&t.album);
            let path = self.string(&t.path);
            let artwork = self.string(&t.artwork);
            let codec = self.string(&t.codec);
            self.track_rows.push(TrackDto {
                id: t.id,
                folder: t.folder_id,
                title,
                artist,
                album,
                path,
                artwork,
                codec,
                duration: t.duration,
                number: t.number,
                year: t.year,
            });
        }
        self.track_count = self.track_rows.len();
        self.tracks = self.track_rows.as_ptr();
        self
    }
}
fn guard(action: impl FnOnce() -> Result<Snapshot, String>) -> *mut Snapshot {
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(action));
    let snapshot = match result {
        Ok(Ok(s)) => s,
        other => {
            let error = match other {
                Ok(Err(e)) => e,
                _ => "The native core encountered an internal error.".into(),
            };
            let mut s = Snapshot::default();
            s.error = s.string(&error);
            s
        }
    };
    Box::into_raw(Box::new(snapshot))
}
unsafe fn core<'a>(ptr: *mut Core) -> Result<&'a Core, String> {
    unsafe { ptr.as_ref() }.ok_or("Native core is closed.".into())
}
unsafe fn text<'a>(ptr: *const c_char) -> Result<&'a str, String> {
    if ptr.is_null() {
        return Err("A path is required.".into());
    }
    unsafe { CStr::from_ptr(ptr) }
        .to_str()
        .map_err(|_| "Path is not valid UTF-8.".into())
}
#[unsafe(no_mangle)]
pub extern "C" fn spiewak_abi_version() -> u32 {
    2
}
/// # Safety
/// `path` must point to a valid NUL-terminated UTF-8 database path during this call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn spiewak_open(path: *const c_char) -> *mut Core {
    std::panic::catch_unwind(|| {
        let path = PathBuf::from(unsafe { text(path) }.ok()?);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).ok()?
        }
        let library = Library::open(&path).ok()?;
        Some(Box::into_raw(Box::new(Core {
            library: Mutex::new(library),
            path,
            audio: Audio::new(),
            scanning: AtomicBool::new(false),
        })))
    })
    .ok()
    .flatten()
    .unwrap_or(std::ptr::null_mut())
}
/// # Safety
/// `ptr` must be a live core returned by open, with no calls still using it. Close exactly once.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn spiewak_close(ptr: *mut Core) {
    if !ptr.is_null() {
        let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            drop(unsafe { Box::from_raw(ptr) })
        }));
    }
}
/// # Safety
/// `ptr` must be a live returned snapshot. Release exactly once, after reading all strings/rows.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn spiewak_snapshot_free(ptr: *mut Snapshot) {
    if !ptr.is_null() {
        drop(unsafe { Box::from_raw(ptr) });
    }
}
/// # Safety
/// `ptr` must be a live core for the duration of the call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn spiewak_folders(ptr: *mut Core, parent: i64) -> *mut Snapshot {
    guard(|| {
        let c = unsafe { core(ptr) }?;
        let lib = c.library.lock().map_err(|_| "Library lock unavailable.")?;
        Ok(Snapshot::default().folders(lib.folders(parent)?))
    })
}
/// # Safety
/// `ptr` must be a live core for the duration of the call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn spiewak_tracks(
    ptr: *mut Core,
    folder: i64,
    offset: u32,
    limit: u32,
) -> *mut Snapshot {
    guard(|| {
        let c = unsafe { core(ptr) }?;
        let lib = c.library.lock().map_err(|_| "Library lock unavailable.")?;
        Ok(Snapshot::default().tracks(lib.tracks(folder, offset, limit.clamp(1, 1000))?))
    })
}
struct ScanGuard<'a>(&'a AtomicBool);
impl Drop for ScanGuard<'_> {
    fn drop(&mut self) {
        self.0.store(false, Ordering::Release);
    }
}
fn scan(c: &Core, path: Option<PathBuf>) -> Result<Snapshot, String> {
    if c.scanning.swap(true, Ordering::AcqRel) {
        return Err("A library scan is already running.".into());
    }
    let _guard = ScanGuard(&c.scanning);
    let lib = Library::open(&c.path)?;
    let report = if let Some(path) = path {
        lib.add_root(&path)?
    } else {
        lib.reconcile()?
    };
    let mut s = Snapshot::default();
    s.message = s.string(&format!(
        "{} tracks indexed{}",
        report.tracks,
        if report.warnings.is_empty() {
            String::new()
        } else {
            format!(" · {}", report.warnings.join("; "))
        }
    ));
    Ok(s)
}
/// # Safety
/// `ptr` must be live and `path` must be a valid NUL-terminated UTF-8 folder path.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn spiewak_add_root(ptr: *mut Core, path: *const c_char) -> *mut Snapshot {
    guard(|| {
        scan(
            unsafe { core(ptr) }?,
            Some(PathBuf::from(unsafe { text(path) }?)),
        )
    })
}
/// # Safety
/// `ptr` must be a live core for the duration of the call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn spiewak_rescan(ptr: *mut Core) -> *mut Snapshot {
    guard(|| scan(unsafe { core(ptr) }?, None))
}
/// # Safety
/// `ptr` must be a live core for the duration of the call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn spiewak_play_folder(
    ptr: *mut Core,
    folder: i64,
    track: i64,
) -> *mut Snapshot {
    guard(|| {
        let c = unsafe { core(ptr) }?;
        let (tracks, index) = c
            .library
            .lock()
            .map_err(|_| "Library unavailable.")?
            .folder_queue(folder, track)?;
        c.audio.command(Command::Replace(tracks, index))?;
        Ok(Snapshot::default())
    })
}
/// # Safety
/// `ptr` must be live; `ids` must reference `count` valid i64 IDs for this call.
/// Mode 0 plays selection; 1 appends; 2 inserts immediately after current.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn spiewak_selection(
    ptr: *mut Core,
    ids: *const i64,
    count: usize,
    mode: u32,
) -> *mut Snapshot {
    guard(|| {
        if ids.is_null() || count == 0 || count > 100_000 {
            return Err("Select tracks first.".into());
        }
        let c = unsafe { core(ptr) }?;
        let rows = c
            .library
            .lock()
            .map_err(|_| "Library unavailable.")?
            .selected_tracks(unsafe { std::slice::from_raw_parts(ids, count) })?;
        let command = match mode {
            0 => Command::Replace(rows, 0),
            1 => Command::Append(rows, false),
            2 => Command::Append(rows, true),
            _ => return Err("Unknown selection action.".into()),
        };
        c.audio.command(command)?;
        Ok(Snapshot::default())
    })
}
/// # Safety
/// `ptr` must be live. Commands: 1 toggle, 2 stop, 3 next, 4 previous, 5 seek seconds, 6 volume [0,1].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn spiewak_audio_command(
    ptr: *mut Core,
    command: u32,
    value: f64,
) -> *mut Snapshot {
    guard(|| {
        let command = match command {
            1 => Command::Toggle,
            2 => Command::Stop,
            3 => Command::Next,
            4 => Command::Previous,
            5 => Command::Seek(value),
            6 => Command::Volume(value as f32),
            7 => Command::Mute,
            8 => Command::Repeat,
            9 => Command::Shuffle,
            10..=12 | 14 => {
                if !value.is_finite()
                    || value < 0.0
                    || value.fract() != 0.0
                    || value > usize::MAX as f64
                {
                    return Err("Invalid queue position.".into());
                }
                let index = value as usize;
                match command {
                    10 => Command::Remove(index),
                    11 => Command::Move(index, true),
                    12 => Command::Move(index, false),
                    _ => Command::Jump(index),
                }
            }
            13 => Command::Clear,
            _ => return Err("Unknown audio command.".into()),
        };
        unsafe { core(ptr) }?.audio.command(command)?;
        Ok(Snapshot::default())
    })
}
/// # Safety
/// `ptr` must be a live core for the duration of the call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn spiewak_now(ptr: *mut Core) -> *mut Snapshot {
    guard(|| {
        let playback = unsafe { core(ptr) }?.audio.snapshot()?;
        let mut s = Snapshot {
            position: playback.position,
            current_id: playback.track.as_ref().map(|t| t.id).unwrap_or(0),
            current_index: playback.current.map(|i| i as i64).unwrap_or(-1),
            revision: playback.revision,
            playing: u32::from(playback.playing),
            volume: playback.volume,
            flags: playback.flags,
            ..Default::default()
        };
        s.error = s.string(&playback.error);
        Ok(s.tracks(playback.track.into_iter().collect()))
    })
}
/// # Safety
/// `ptr` must be a live core for the duration of the call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn spiewak_queue(ptr: *mut Core) -> *mut Snapshot {
    guard(|| Ok(Snapshot::default().tracks(unsafe { core(ptr) }?.audio.queue()?)))
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn snapshot_layout_and_utf8_ownership() {
        assert_eq!(spiewak_abi_version(), 2);
        assert_eq!(std::mem::size_of::<FolderDto>(), 40);
        assert_eq!(std::mem::size_of::<TrackDto>(), 80);
        let s = Snapshot::default().tracks(vec![Track {
            title: "Śpiewak 🎵".into(),
            ..Default::default()
        }]);
        assert_eq!(s.track_count, 1);
        assert_eq!(
            unsafe { CStr::from_ptr((*s.tracks).title) }
                .to_str()
                .unwrap(),
            "Śpiewak 🎵"
        );
    }
    #[test]
    fn null_core_returns_owned_error_not_a_panic() {
        let s = unsafe { spiewak_now(std::ptr::null_mut()) };
        assert!(!unsafe { (*s).error }.is_null());
        unsafe { spiewak_snapshot_free(s) };
    }
}
