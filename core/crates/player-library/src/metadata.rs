//! Explicit metadata changes with durable originals. Indexing never calls these write APIs.
use super::*;
use lofty::{
    config::{GlobalOptions, ParsingMode, WriteOptions},
    picture::{Picture, PictureType},
    tag::{Tag, items::Timestamp},
};
use sha2::{Digest, Sha256};
use std::io::{Read, Seek, SeekFrom};
const FILE_LIMIT: u64 = 512 * 1024 * 1024;
const STORE_LIMIT: u64 = 4 * 1024 * 1024 * 1024;
#[repr(u32)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MetadataField {
    Title = 1,
    Artist,
    Album,
    AlbumArtist,
    TrackNumber,
    TrackTotal,
    DiscNumber,
    DiscTotal,
    Date,
    Genre,
    Composer,
    Comment,
    Artwork,
    FileName,
    ReplayGainTrack,
    ReplayGainAlbum,
    ReplayPeakTrack,
    ReplayPeakAlbum,
}
impl MetadataField {
    pub fn from_value(value: u32) -> Result<Self> {
        Self::all()
            .into_iter()
            .find(|f| *f as u32 == value)
            .ok_or("Unknown metadata field.".into())
    }
    pub fn all() -> [Self; 18] {
        use MetadataField::*;
        [
            Title,
            Artist,
            Album,
            AlbumArtist,
            TrackNumber,
            TrackTotal,
            DiscNumber,
            DiscTotal,
            Date,
            Genre,
            Composer,
            Comment,
            Artwork,
            FileName,
            ReplayGainTrack,
            ReplayGainAlbum,
            ReplayPeakTrack,
            ReplayPeakAlbum,
        ]
    }
    fn key(self) -> Option<ItemKey> {
        use MetadataField::*;
        Some(match self {
            Title => ItemKey::TrackTitle,
            Artist => ItemKey::TrackArtist,
            Album => ItemKey::AlbumTitle,
            AlbumArtist => ItemKey::AlbumArtist,
            TrackNumber => ItemKey::TrackNumber,
            TrackTotal => ItemKey::TrackTotal,
            DiscNumber => ItemKey::DiscNumber,
            DiscTotal => ItemKey::DiscTotal,
            Date => ItemKey::RecordingDate,
            Genre => ItemKey::Genre,
            Composer => ItemKey::Composer,
            Comment => ItemKey::Comment,
            ReplayGainTrack => ItemKey::ReplayGainTrackGain,
            ReplayGainAlbum => ItemKey::ReplayGainAlbumGain,
            ReplayPeakTrack => ItemKey::ReplayGainTrackPeak,
            ReplayPeakAlbum => ItemKey::ReplayGainAlbumPeak,
            Artwork | FileName => return None,
        })
    }
}
#[derive(Clone, Debug)]
pub struct MetadataValue {
    pub field: MetadataField,
    pub value: String,
}
#[repr(u32)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OperationState {
    Current = 0,
    Prepared = 1,
    Applying = 2,
    Applied = 3,
    Undoing = 4,
    Undone = 5,
    Conflict = 6,
    Cancelled = 7,
}
impl OperationState {
    fn from_value(v: u32) -> Result<Self> {
        Ok(match v {
            0 => Self::Current,
            1 => Self::Prepared,
            2 => Self::Applying,
            3 => Self::Applied,
            4 => Self::Undoing,
            5 => Self::Undone,
            6 => Self::Conflict,
            7 => Self::Cancelled,
            _ => return Err("Invalid operation state. Keep the database for recovery.".into()),
        })
    }
}
#[derive(Clone, Debug)]
pub struct MetadataRecord {
    pub operation_id: i64,
    pub track_id: i64,
    pub state: OperationState,
    pub created: i64,
    pub original_path: String,
    pub target_path: String,
    pub message: String,
    pub values: Vec<MetadataValue>,
}
struct Operation {
    record: MetadataRecord,
    before_hash: String,
    after_hash: String,
    identity: String,
}
pub(super) fn migrate(db: &Connection) -> Result<()> {
    db.execute_batch("SAVEPOINT migration; CREATE TABLE file_operations(id INTEGER PRIMARY KEY AUTOINCREMENT,track_id INTEGER NOT NULL,state INTEGER NOT NULL,created INTEGER NOT NULL DEFAULT(unixepoch()),original_path TEXT NOT NULL,target_path TEXT NOT NULL,identity TEXT NOT NULL,before_hash TEXT NOT NULL,after_hash TEXT NOT NULL DEFAULT '',message TEXT NOT NULL DEFAULT ''); CREATE INDEX operation_track ON file_operations(track_id,state); PRAGMA user_version=11; RELEASE migration;").map_err(|e|e.to_string())
}
fn options() {
    lofty::config::apply_global_options(
        GlobalOptions::new()
            .allocation_limit(16 * 1024 * 1024)
            .preserve_format_specific_items(true),
    );
}
fn digest(file: &mut fs::File) -> Result<String> {
    file.seek(SeekFrom::Start(0)).map_err(|e| e.to_string())?;
    let mut hash = Sha256::new();
    let mut buffer = [0u8; 65536];
    loop {
        let n = file.read(&mut buffer).map_err(|e| e.to_string())?;
        if n == 0 {
            break;
        }
        hash.update(&buffer[..n]);
    }
    Ok(format!("{:x}", hash.finalize()))
}
fn hash_path(path: &Path) -> Result<String> {
    digest(&mut fs::File::open(path).map_err(|e| e.to_string())?)
}
fn copy_to(source: &mut fs::File, target: &mut fs::File) -> Result<()> {
    source.seek(SeekFrom::Start(0)).map_err(|e| e.to_string())?;
    target.seek(SeekFrom::Start(0)).map_err(|e| e.to_string())?;
    std::io::copy(source, target).map_err(|e| e.to_string())?;
    let length = target.stream_position().map_err(|e| e.to_string())?;
    target.set_len(length).map_err(|e| e.to_string())?;
    target.sync_all().map_err(|e| e.to_string())
}
fn lock(path: &Path, write: bool) -> Result<fs::File> {
    let meta = fs::symlink_metadata(path).map_err(|e| e.to_string())?;
    if !meta.is_file() || meta.is_symlink() || meta.len() > FILE_LIMIT {
        return Err("Edit a regular audio file of at most 512 MiB.".into());
    }
    let mut open = fs::OpenOptions::new();
    open.read(true);
    #[cfg(windows)]
    {
        use std::os::windows::fs::{MetadataExt, OpenOptionsExt};
        if meta.file_attributes() & 0x400 != 0 {
            return Err("Editing reparse points is not supported.".into());
        }
        open.share_mode(1);
        if write {
            open.access_mode(0x80000000 | 0x40000000 | 0x00010000);
        }
    }
    #[cfg(not(windows))]
    if write {
        return Err("File mutation is supported only on Windows; use the Windows player.".into());
    }
    let file = open.open(path).map_err(|e| {
        format!(
            "Cannot lock {} for editing: {e}. Close other file editors and try again.",
            path.display()
        )
    })?;
    #[cfg(windows)]
    {
        use std::os::windows::io::AsRawHandle;
        #[repr(C)]
        #[derive(Default)]
        struct Standard {
            allocation: i64,
            end: i64,
            links: u32,
            pending: u8,
            directory: u8,
        }
        #[link(name = "kernel32")]
        unsafe extern "system" {
            fn GetFileInformationByHandleEx(
                h: *mut std::ffi::c_void,
                class: i32,
                info: *mut std::ffi::c_void,
                size: u32,
            ) -> i32;
        }
        let mut info = Standard::default();
        if unsafe {
            GetFileInformationByHandleEx(
                file.as_raw_handle(),
                1,
                (&mut info as *mut Standard).cast(),
                std::mem::size_of::<Standard>() as u32,
            )
        } == 0
            || info.links != 1
        {
            return Err(
                "Editing requires one regular file, not a hard-linked or unidentifiable file."
                    .into(),
            );
        }
    }
    Ok(file)
}
#[cfg(windows)]
fn rename_locked(file: &fs::File, target: &Path) -> Result<()> {
    use std::os::windows::{ffi::OsStrExt, io::AsRawHandle};
    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn SetFileInformationByHandle(
            h: *mut std::ffi::c_void,
            class: i32,
            info: *const std::ffi::c_void,
            size: u32,
        ) -> i32;
    }
    let name = target.as_os_str().encode_wide().collect::<Vec<_>>();
    let bytes = 20 + (name.len() + 1) * 2;
    let mut info = vec![0u64; bytes.div_ceil(8)];
    let ptr = info.as_mut_ptr().cast::<u8>();
    // FILE_RENAME_INFO: no replacement, NULL root, byte count excluding the NUL.
    // The Win32 DOS-path conversion requires a terminated filename even with an explicit length.
    unsafe {
        std::ptr::write_unaligned(ptr.add(16).cast::<u32>(), (name.len() * 2) as u32);
        std::ptr::copy_nonoverlapping(name.as_ptr().cast::<u8>(), ptr.add(20), name.len() * 2);
    }
    if unsafe { SetFileInformationByHandle(file.as_raw_handle(), 3, ptr.cast(), bytes as u32) } == 0
    {
        Err(format!(
            "Cannot rename file without replacement: {}",
            std::io::Error::last_os_error()
        ))
    } else {
        Ok(())
    }
}
#[cfg(not(windows))]
fn rename_locked(_file: &fs::File, _target: &Path) -> Result<()> {
    Err("Mutation requires Windows.".into())
}
fn filename(value: &str, original: &Path) -> Result<PathBuf> {
    if value.is_empty()
        || value.encode_utf16().count() > 240
        || value.contains([
            '\\', '/', ':', '\0', '\r', '\n', '<', '>', '"', '|', '?', '*',
        ])
        || value.chars().any(|c| c.is_control())
        || value.ends_with(['.', ' '])
    {
        return Err(
            "Choose a filename, without paths, control characters or Windows reserved punctuation."
                .into(),
        );
    }
    let stem = value.split('.').next().unwrap().to_ascii_uppercase();
    if ["CON", "PRN", "AUX", "NUL"].contains(&stem.as_str())
        || (stem.len() == 4
            && (stem.starts_with("COM") || stem.starts_with("LPT"))
            && stem.as_bytes()[3].is_ascii_digit()
            && stem.as_bytes()[3] != b'0')
    {
        return Err("This filename is reserved by Windows.".into());
    }
    let path = original
        .parent()
        .ok_or("Audio folder is unavailable.")?
        .join(value);
    if !path
        .extension()
        .zip(original.extension())
        .is_some_and(|(a, b)| a.eq_ignore_ascii_case(b))
    {
        return Err("Keep the original audio extension. Renaming does not convert audio.".into());
    }
    Ok(path)
}
fn values(path: &Path) -> Result<Vec<MetadataValue>> {
    options();
    let file = lofty::read_from_path(path).map_err(|e| {
        format!("Cannot read complete metadata: {e}. The original file is unchanged.")
    })?;
    let tag = file.primary_tag().or_else(|| file.first_tag());
    Ok(MetadataField::all()
        .into_iter()
        .map(|field| {
            let value = match field {
                MetadataField::FileName => path
                    .file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .into_owned(),
                MetadataField::Artwork => tag
                    .map(|t| format!("{} embedded picture(s)", t.pictures().len()))
                    .unwrap_or_default(),
                MetadataField::Date => tag
                    .and_then(|t| t.date())
                    .map(|d| d.to_string())
                    .unwrap_or_default(),
                _ => tag
                    .and_then(|t| t.get_string(field.key().unwrap()))
                    .unwrap_or_default()
                    .to_owned(),
            };
            MetadataValue { field, value }
        })
        .collect())
}
fn patch(path: &Path, changes: &[MetadataValue], cache: &mut artwork::Cache) -> Result<()> {
    if changes.iter().all(|e| e.field == MetadataField::FileName) {
        return Ok(());
    }
    options();
    let mut file = lofty::read_from_path(path).map_err(|e| e.to_string())?;
    let kind = file.primary_tag_type();
    if file.primary_tag().is_none() {
        file.insert_tag(Tag::new(kind));
    }
    let tag = file.primary_tag_mut().ok_or("No writable primary tag.")?;
    for edit in changes {
        match edit.field {
            MetadataField::FileName => {}
            MetadataField::Artwork => {
                if edit.value.is_empty() {
                    tag.remove_picture_type(PictureType::CoverFront);
                    continue;
                }
                let mut bytes = Vec::new();
                fs::File::open(&edit.value)
                    .map_err(|e| e.to_string())?
                    .take(8 * 1024 * 1024 + 1)
                    .read_to_end(&mut bytes)
                    .map_err(|e| e.to_string())?;
                if cache.store(&bytes)?.is_empty() {
                    return Err("Artwork must be a valid bounded JPEG/PNG.".into());
                }
                let mut picture = Picture::from_reader(&mut std::io::Cursor::new(bytes))
                    .map_err(|e| e.to_string())?;
                picture.set_pic_type(PictureType::CoverFront);
                tag.remove_picture_type(PictureType::CoverFront);
                tag.push_picture(picture);
            }
            MetadataField::Date => {
                if edit.value.is_empty() {
                    tag.remove_date();
                } else {
                    let date = Timestamp::parse(&mut edit.value.as_bytes(), ParsingMode::Strict)
                        .map_err(|e| e.to_string())?
                        .ok_or("Enter a year or ISO recording date.")?;
                    tag.set_date(date);
                }
            }
            field => {
                let key = field.key().unwrap();
                if edit.value.is_empty() {
                    tag.remove_key(key);
                } else {
                    tag.insert_text(key, edit.value.clone());
                }
            }
        }
    }
    file.save_to_path(path, WriteOptions::default())
        .map_err(|e| format!("Cannot prepare edited metadata: {e}"))?;
    fs::File::options()
        .write(true)
        .open(path)
        .map_err(|e| e.to_string())?
        .sync_all()
        .map_err(|e| e.to_string())?;
    // Refuse lossy/unsupported field conversions instead of applying silently different values.
    let after = values(path)?;
    for edit in changes
        .iter()
        .filter(|e| !matches!(e.field, MetadataField::Artwork | MetadataField::FileName))
    {
        if after
            .iter()
            .find(|v| v.field == edit.field)
            .map(|v| v.value.as_str())
            != Some(edit.value.as_str())
        {
            return Err(format!(
                "This format cannot preserve field {:?} exactly. The original is unchanged.",
                edit.field
            ));
        }
    }
    Ok(())
}
impl Library {
    fn reserve_operation_bytes(&self, additional: u64) -> Result<()> {
        let mut used = 0u64;
        for entry in fs::read_dir(&self.operation_dir).map_err(|e| e.to_string())? {
            let dir = entry.map_err(|e| e.to_string())?;
            if dir.file_type().map_err(|e| e.to_string())?.is_dir() {
                for entry in fs::read_dir(dir.path()).map_err(|e| e.to_string())? {
                    used = used.saturating_add(
                        entry
                            .map_err(|e| e.to_string())?
                            .metadata()
                            .map_err(|e| e.to_string())?
                            .len(),
                    );
                }
            }
        }
        if used.saturating_add(additional) > STORE_LIMIT {
            return Err("Operation backups reached 4 GiB. Existing originals were kept; no music was changed.".into());
        }
        Ok(())
    }
    fn operation(&self, id: i64) -> Result<Operation> {
        self.db.query_row("SELECT id,track_id,state,created,original_path,target_path,message,before_hash,after_hash,identity FROM file_operations WHERE id=?",[id],|r|Ok((r.get::<_,i64>(0)?,r.get::<_,i64>(1)?,r.get::<_,u32>(2)?,r.get::<_,i64>(3)?,r.get::<_,String>(4)?,r.get::<_,String>(5)?,r.get::<_,String>(6)?,r.get::<_,String>(7)?,r.get::<_,String>(8)?,r.get::<_,String>(9)?))).map_err(|e|e.to_string()).and_then(|(id,track,state,created,original_path,target_path,message,before_hash,after_hash,identity)|Ok(Operation{record:MetadataRecord{operation_id:id,track_id:track,state:OperationState::from_value(state)?,created,original_path,target_path,message,values:Vec::new()},before_hash,after_hash,identity}))
    }
    fn stage_paths(&self, id: i64, source: &Path) -> (PathBuf, PathBuf) {
        let ext = source.extension().unwrap_or_default().to_string_lossy();
        let dir = self.operation_dir.join(id.to_string());
        (
            dir.join(format!("original.{ext}")),
            dir.join(format!("prepared.{ext}")),
        )
    }
    fn set_operation(&self, id: i64, state: OperationState, message: &str) -> Result<()> {
        self.db
            .execute(
                "UPDATE file_operations SET state=?,message=? WHERE id=?",
                params![state as u32, message, id],
            )
            .map_err(|e| e.to_string())?;
        Ok(())
    }
    pub fn metadata(&self, track: i64) -> Result<MetadataRecord> {
        let t = self.selected_tracks(&[track])?.remove(0);
        let path = Path::new(&t.path);
        let file = lock(path, false)?;
        let values = values(path)?;
        drop(file);
        Ok(MetadataRecord {
            operation_id: 0,
            track_id: track,
            state: OperationState::Current,
            created: 0,
            original_path: t.path.clone(),
            target_path: t.path,
            message: String::new(),
            values,
        })
    }
    pub fn prepare_metadata(
        &self,
        track: i64,
        changes: &[MetadataValue],
    ) -> Result<Vec<MetadataRecord>> {
        if changes.is_empty() || changes.len() > 18 {
            return Err("Choose at least one metadata or filename change.".into());
        }
        let mut fields = std::collections::HashSet::new();
        for edit in changes {
            if !fields.insert(edit.field as u32)
                || edit.value.len() > 16384
                || edit.value.contains('\0')
            {
                return Err("Invalid or duplicate metadata field.".into());
            }
            if matches!(
                edit.field,
                MetadataField::TrackNumber
                    | MetadataField::TrackTotal
                    | MetadataField::DiscNumber
                    | MetadataField::DiscTotal
            ) && !edit.value.is_empty()
            {
                let n = edit
                    .value
                    .parse::<u32>()
                    .map_err(|_| "Track/disc numbers must be positive integers.")?;
                if n == 0 || n > 9999 {
                    return Err("Track/disc numbers must be between 1 and 9999.".into());
                }
            }
            if matches!(
                edit.field,
                MetadataField::ReplayGainTrack
                    | MetadataField::ReplayGainAlbum
                    | MetadataField::ReplayPeakTrack
                    | MetadataField::ReplayPeakAlbum
            ) && !edit.value.is_empty()
            {
                let n = edit
                    .value
                    .trim_end_matches(" dB")
                    .parse::<f64>()
                    .map_err(|_| "Enter a finite ReplayGain value.")?;
                if !n.is_finite() || n.abs() > 1000.0 {
                    return Err("ReplayGain value is outside the supported range.".into());
                }
            }
        }
        let t = self.selected_tracks(&[track])?.remove(0);
        let original = PathBuf::from(&t.path);
        let mut source = lock(&original, false)?;
        let identity = identity::file_identity(&original)
            .ok_or("File identity is unavailable; mutation is disabled for this file.")?;
        let target = changes
            .iter()
            .find(|v| v.field == MetadataField::FileName)
            .map(|v| filename(&v.value, &original))
            .transpose()?
            .unwrap_or_else(|| original.clone());
        if target != original
            && target.exists()
            && identity::file_identity(&target).as_deref() != Some(&identity)
        {
            return Err("The destination filename already exists. Nothing was changed.".into());
        }
        fs::create_dir_all(&self.operation_dir).map_err(|e| e.to_string())?;
        self.reserve_operation_bytes(
            source
                .metadata()
                .map_err(|e| e.to_string())?
                .len()
                .saturating_mul(2)
                .saturating_add(8 * 1024 * 1024 + 18 * 16384),
        )?;
        let before_hash = digest(&mut source)?;
        self.db.execute("INSERT INTO file_operations(track_id,state,original_path,target_path,identity,before_hash) VALUES(?,?,?,?,?,?)",params![track,OperationState::Cancelled as u32,t.path,display_path(&target),identity,before_hash]).map_err(|e|e.to_string())?;
        let id = self.db.last_insert_rowid();
        let (backup, prepared) = self.stage_paths(id, &original);
        let result = (|| -> Result<Vec<MetadataRecord>> {
            fs::create_dir(backup.parent().unwrap()).map_err(|e| e.to_string())?;
            let mut saved = fs::File::options()
                .read(true)
                .write(true)
                .create_new(true)
                .open(&backup)
                .map_err(|e| e.to_string())?;
            copy_to(&mut source, &mut saved)?;
            let mut staged = fs::File::options()
                .read(true)
                .write(true)
                .create_new(true)
                .open(&prepared)
                .map_err(|e| e.to_string())?;
            copy_to(&mut saved, &mut staged)?;
            drop(staged);
            let before_values = values(&backup)?;
            let mut cache = artwork::Cache::new(self.artwork_dir.clone());
            patch(&prepared, changes, &mut cache)?;
            if fs::metadata(&prepared).map_err(|e| e.to_string())?.len() > FILE_LIMIT {
                return Err("Prepared metadata exceeds the 512 MiB file limit; original music is unchanged.".into());
            }
            let after_hash = hash_path(&prepared)?;
            if before_hash == after_hash && target == original {
                return Err("These values would not change the file.".into());
            }
            self.db
                .execute(
                    "UPDATE file_operations SET state=?,after_hash=?,message=? WHERE id=?",
                    params![
                        OperationState::Prepared as u32,
                        after_hash,
                        cache.warnings.join(" "),
                        id
                    ],
                )
                .map_err(|e| e.to_string())?;
            let mut after = self.operation(id)?.record;
            after.values = values(&prepared)?;
            for v in &mut after.values {
                if v.field == MetadataField::FileName {
                    v.value = target.file_name().unwrap().to_string_lossy().into_owned();
                }
            }
            let mut before = after.clone();
            before.target_path = t.path.clone();
            before.values = before_values;
            for v in &mut before.values {
                if v.field == MetadataField::FileName {
                    v.value = original.file_name().unwrap().to_string_lossy().into_owned();
                }
            }
            Ok(vec![before, after])
        })();
        if result.is_err() {
            let _ = self.set_operation(
                id,
                OperationState::Cancelled,
                "Preparation failed; original music was unchanged.",
            );
        }
        result
    }
    pub fn metadata_operation(&self, id: i64) -> Result<MetadataRecord> {
        Ok(self.operation(id)?.record)
    }
    pub fn metadata_operations(&self) -> Result<Vec<MetadataRecord>> {
        let ids = self
            .db
            .prepare("SELECT id FROM file_operations ORDER BY id DESC LIMIT 100")
            .map_err(|e| e.to_string())?
            .query_map([], |r| r.get::<_, i64>(0))
            .map_err(|e| e.to_string())?
            .collect::<std::result::Result<Vec<_>, _>>()
            .map_err(|e| e.to_string())?;
        ids.into_iter()
            .map(|id| self.operation(id).map(|op| op.record))
            .collect()
    }
    fn refresh_edited(&self, id: i64, path: &Path) -> Result<()> {
        let folder: Option<i64> = self
            .db
            .query_row("SELECT folder_id FROM tracks WHERE id=?", [id], |r| {
                r.get(0)
            })
            .map_err(|_| "Track reference disappeared; keep the operation backup for recovery.")?;
        let mut cache = artwork::Cache::new(self.artwork_dir.clone());
        let t = read_track(path, folder.unwrap_or(0), &mut cache);
        let meta = fs::metadata(path).map_err(|e| e.to_string())?;
        self.db.execute("UPDATE tracks SET path=?,title=?,artist=?,album=?,album_artist=?,artwork=?,embedded_art=?,codec=?,duration=?,number=?,year=?,size=?,modified=?,file_identity=?,art_checked=? WHERE id=?",params![display_path(path),t.title,t.artist,t.album,t.album_artist,t.artwork,t.artwork,t.codec,t.duration,t.number,t.year,meta.len() as i64,stamp(&meta),identity::file_identity(path),i32::from(!cache.retry),id]).map_err(|e|e.to_string())?;
        if let Some(folder) = folder {
            let f = self.folder_current(folder)?.ok_or("Folder disappeared.")?;
            self.apply_artwork(
                folder,
                matches!(
                    f.kind,
                    player_domain::FolderKind::Album | player_domain::FolderKind::Compilation
                ),
            )?;
        }
        Ok(())
    }
    pub fn apply_metadata(&self, id: i64) -> Result<MetadataRecord> {
        self.mutate_metadata(id, false)
    }
    pub fn undo_metadata(&self, id: i64) -> Result<MetadataRecord> {
        self.mutate_metadata(id, true)
    }
    fn mutate_metadata(&self, id: i64, undo: bool) -> Result<MetadataRecord> {
        let op = self.operation(id)?;
        if op.record.state
            != if undo {
                OperationState::Applied
            } else {
                OperationState::Prepared
            }
        {
            return Err(
                "This operation is not ready to apply/undo. Keep its original backup for recovery."
                    .into(),
            );
        }
        let original = PathBuf::from(&op.record.original_path);
        let target = PathBuf::from(&op.record.target_path);
        let (from, to, expected, state, done) = if undo {
            (
                &target,
                &original,
                &op.after_hash,
                OperationState::Undoing,
                OperationState::Undone,
            )
        } else {
            (
                &original,
                &target,
                &op.before_hash,
                OperationState::Applying,
                OperationState::Applied,
            )
        };
        let mut source = lock(from, true)?;
        if identity::file_identity(from).as_deref() != Some(&op.identity)
            || digest(&mut source)? != *expected
        {
            return Err("This file changed since the preview/operation. Refresh its metadata; Undo never overwrites external edits.".into());
        }
        if from != to && to.exists() && identity::file_identity(to).as_deref() != Some(&op.identity)
        {
            return Err("The destination filename is occupied. Nothing was changed.".into());
        }
        let (backup, prepared) = self.stage_paths(id, &original);
        let input = if undo { &backup } else { &prepared };
        let expected_input = if undo {
            &op.before_hash
        } else {
            &op.after_hash
        };
        let mut input_file = lock(input, false)
            .map_err(|e| format!("Operation backup is unavailable: {e}. Nothing was changed."))?;
        if digest(&mut input_file)? != *expected_input {
            return Err("Operation backup verification failed. Nothing was changed.".into());
        }
        self.set_operation(
            id,
            state,
            "File write in progress; original backup retained.",
        )?;
        let renamed = from != to;
        if renamed && let Err(e) = rename_locked(&source, to) {
            self.set_operation(id, op.record.state, &e)?;
            return Err(e);
        }
        let result = (|| -> Result<()> {
            copy_to(&mut input_file, &mut source)?;
            if digest(&mut source)? != *expected_input {
                return Err("Written file verification failed.".into());
            }
            let tx = self.db.unchecked_transaction().map_err(|e| e.to_string())?;
            self.refresh_edited(op.record.track_id, to)?;
            self.set_operation(
                id,
                done,
                if undo {
                    "Original bytes and filename restored."
                } else {
                    "Metadata applied; original backup retained for Undo."
                },
            )?;
            tx.commit().map_err(|e| e.to_string())?;
            Ok(())
        })();
        if let Err(e) = result {
            // A durable original and journal always remain, including on SQL or disk failure.
            let rollback = if undo { &prepared } else { &backup };
            let rollback = (|| -> Result<()> {
                let mut f = lock(rollback, false)?;
                if digest(&mut f)? != *expected {
                    return Err("Rollback backup is invalid.".into());
                }
                copy_to(&mut f, &mut source)?;
                if renamed {
                    rename_locked(&source, from)?;
                }
                Ok(())
            })();
            let state = if rollback.is_ok() {
                op.record.state
            } else {
                OperationState::Conflict
            };
            let message = format!(
                "{e}; rollback: {:?}. Original backups were kept at {}",
                rollback.err(),
                self.operation_dir.display()
            );
            let _ = self.set_operation(id, state, &message);
            return Err(message);
        }
        let mut row = self.operation(id)?.record;
        row.values = values(to)?;
        Ok(row)
    }
    // Startup reconciliation changes only the index/journal. Unknown/torn bytes require explicit recovery.
    pub fn reconcile_metadata_operations(&self) -> Result<()> {
        let ids = self
            .db
            .prepare("SELECT id FROM file_operations WHERE state IN (2,4) ORDER BY id")
            .map_err(|e| e.to_string())?
            .query_map([], |r| r.get::<_, i64>(0))
            .map_err(|e| e.to_string())?
            .collect::<std::result::Result<Vec<_>, _>>()
            .map_err(|e| e.to_string())?;
        for id in ids {
            let op = self.operation(id)?;
            let mut resolved = None;
            let mut busy = false;
            for path in [&op.record.original_path, &op.record.target_path] {
                if identity::file_identity(Path::new(path)).as_deref() != Some(&op.identity) {
                    continue;
                }
                let Ok(mut file) = lock(Path::new(path), false) else {
                    busy = true;
                    continue;
                };
                let Ok(hash) = digest(&mut file) else {
                    continue;
                };
                if hash == op.before_hash || hash == op.after_hash {
                    resolved = Some((path.clone(), hash == op.before_hash));
                    break;
                }
            }
            if busy && resolved.is_none() {
                self.set_operation(id,op.record.state,"Interrupted operation is still locked or unavailable. Close other editors and retry startup recovery; backups were kept.")?;
                continue;
            }
            if let Some((path, before)) = resolved {
                let state = if op.record.state == OperationState::Undoing
                    && before
                    && path == op.record.original_path
                {
                    OperationState::Undone
                } else if path == op.record.target_path
                    && (!before || op.before_hash == op.after_hash)
                {
                    OperationState::Applied
                } else if before && path == op.record.original_path {
                    OperationState::Prepared
                } else {
                    OperationState::Conflict
                };
                let tx = self.db.unchecked_transaction().map_err(|e| e.to_string())?;
                if let Err(e) = self.refresh_edited(op.record.track_id, Path::new(&path)) {
                    drop(tx);
                    self.set_operation(
                        id,
                        OperationState::Conflict,
                        &format!("Interrupted operation: {e}. Original backup kept."),
                    )?;
                    continue;
                }
                self.set_operation(id,state,"Interrupted operation reconciled from verified file bytes. Original backup kept.")?;
                tx.commit().map_err(|e| e.to_string())?;
            } else {
                self.set_operation(id,OperationState::Conflict,"Interrupted write or external file change. Automatic restoration is disabled; original backup is retained for explicit recovery.")?;
            }
        }
        Ok(())
    }
    // Explicit recovery preserves even unrecognized current bytes before restoring the original.
    pub fn recover_metadata_original(&self, id: i64) -> Result<MetadataRecord> {
        let op = self.operation(id)?;
        if op.record.state != OperationState::Conflict {
            return Err("Only an interrupted/conflicting operation requires recovery.".into());
        }
        let original = PathBuf::from(&op.record.original_path);
        let from=[&op.record.original_path,&op.record.target_path].into_iter().find(|p|identity::file_identity(Path::new(p)).as_deref()==Some(&op.identity)).ok_or("Original file identity is unavailable. Keep the backups and recover manually; no file was overwritten.")?;
        let from = PathBuf::from(from);
        let mut source = lock(&from, true)?;
        if identity::file_identity(&from).as_deref() != Some(&op.identity) {
            return Err("File identity changed before recovery; no file was overwritten.".into());
        }
        if from != original
            && original.exists()
            && identity::file_identity(&original).as_deref() != Some(&op.identity)
        {
            return Err("Original filename is occupied. No file was overwritten.".into());
        }
        let (backup, _) = self.stage_paths(id, &original);
        let mut saved = lock(&backup, false)?;
        if digest(&mut saved)? != op.before_hash {
            return Err("Original backup verification failed.".into());
        }
        let hash = digest(&mut source)?;
        let recovery = backup.parent().unwrap().join(format!(
            "recovery-{hash}.{}",
            original.extension().unwrap().to_string_lossy()
        ));
        if !recovery.exists() {
            self.reserve_operation_bytes(source.metadata().map_err(|e| e.to_string())?.len())?;
            let mut snapshot = fs::File::options()
                .read(true)
                .write(true)
                .create_new(true)
                .open(&recovery)
                .map_err(|e| e.to_string())?;
            copy_to(&mut source, &mut snapshot)?;
        } else if hash_path(&recovery)? != hash {
            return Err("Recovery snapshot is invalid; no music was changed.".into());
        }
        self.set_operation(id,OperationState::Undoing,"Explicit recovery in progress. Current bytes also preserved in the operation directory.")?;
        let result = (|| -> Result<()> {
            if from != original {
                rename_locked(&source, &original)?;
            }
            copy_to(&mut saved, &mut source)?;
            if digest(&mut source)? != op.before_hash {
                return Err("Recovered bytes did not verify.".into());
            }
            let tx = self.db.unchecked_transaction().map_err(|e| e.to_string())?;
            self.refresh_edited(op.record.track_id, &original)?;
            self.set_operation(
                id,
                OperationState::Undone,
                "Original restored; pre-recovery bytes also retained.",
            )?;
            tx.commit().map_err(|e| e.to_string())?;
            Ok(())
        })();
        if let Err(e) = result {
            let _ = self.set_operation(
                id,
                OperationState::Conflict,
                &format!("Recovery interrupted: {e}. Both original and pre-recovery backups kept."),
            );
            return Err(e);
        }
        Ok(self.operation(id)?.record)
    }
    pub fn cancel_metadata(&self, id: i64) -> Result<()> {
        let op = self.operation(id)?;
        if op.record.state != OperationState::Prepared {
            return Err("Only a prepared proposal can be cancelled.".into());
        }
        self.set_operation(id, OperationState::Cancelled, "Cancelled; music unchanged.")
    }
    pub fn cleanup_suggestion(&self, track: i64) -> Result<Vec<MetadataValue>> {
        let meta = self.metadata(track)?;
        let path = Path::new(&meta.original_path);
        let stem = path.file_stem().unwrap_or_default().to_string_lossy();
        let cleaned = clean_download_name(&stem);
        if cleaned == stem {
            return Ok(Vec::new());
        }
        let mut result = vec![MetadataValue {
            field: MetadataField::FileName,
            value: format!(
                "{}.{}",
                cleaned,
                path.extension().unwrap_or_default().to_string_lossy()
            ),
        }];
        if let Some((artist, title)) = cleaned
            .split_once(" - ")
            .filter(|(a, t)| !a.trim().is_empty() && !t.trim().is_empty())
        {
            for (field, value) in [
                (MetadataField::Artist, artist),
                (MetadataField::Title, title),
            ] {
                if meta
                    .values
                    .iter()
                    .find(|v| v.field == field)
                    .is_some_and(|v| v.value.is_empty())
                {
                    result.push(MetadataValue {
                        field,
                        value: value.trim().into(),
                    });
                }
            }
        }
        Ok(result)
    }
}
fn clean_download_name(name: &str) -> String {
    let mut value = name.trim().to_owned();
    loop {
        let before = value.clone();
        for suffix in [
            " (official video)",
            " [official video]",
            " (official audio)",
            " [official audio]",
            " (lyrics)",
            " [lyrics]",
            " (320kbps)",
            " [320kbps]",
        ] {
            if value.to_ascii_lowercase().ends_with(suffix) {
                value.truncate(value.len() - suffix.len());
                value = value.trim_end().to_owned();
                break;
            }
        }
        if before == value {
            break;
        }
    }
    if value.is_empty() {
        name.to_owned()
    } else {
        value
    }
}

#[cfg(test)]
pub(super) fn drop_migration(db: &Connection) {
    db.execute_batch("DROP TABLE file_operations;").unwrap();
}

#[cfg(all(test, windows))]
mod tests {
    use super::*;
    use crate::tests::{sandbox, wav};
    use lofty::{id3::v2::Id3v2Tag, tag::TagExt};
    use std::io::Write;
    fn edit(field: MetadataField, value: &str) -> MetadataValue {
        MetadataValue {
            field,
            value: value.into(),
        }
    }
    fn indexed(p: &Path, name: &str) -> (Library, Track) {
        wav(&p.join(name));
        let lib = Library::open(&p.join("index.db")).unwrap();
        lib.add_root(p).unwrap();
        let track = lib
            .search(name.split('.').next().unwrap())
            .unwrap()
            .1
            .into_iter()
            .find(|t| Path::new(&t.path).file_name().unwrap() == name)
            .unwrap();
        (lib, track)
    }
    fn data(path: &Path) -> Vec<u8> {
        let bytes = fs::read(path).unwrap();
        let mut i = 12;
        while i + 8 <= bytes.len() {
            let n = u32::from_le_bytes(bytes[i + 4..i + 8].try_into().unwrap()) as usize;
            if &bytes[i..i + 4] == b"data" {
                return bytes[i + 8..i + 8 + n].to_vec();
            }
            i += 8 + n + n % 2;
        }
        panic!("Missing PCM data")
    }
    #[test]
    fn preview_apply_undo_preserves_audio_unknown_tags_references_and_real_folder() {
        let p = sandbox();
        let path = p.join("Artist - Song (Live) (Official Video).wav");
        wav(&path);
        options();
        let mut tag = Id3v2Tag::new();
        tag.insert_user_text("KeepCustomField".into(), "Leave me untouched".into());
        tag.set_genre("Existing genre".into());
        tag.save_to_path(&path, WriteOptions::default()).unwrap();
        let before = fs::read(&path).unwrap();
        let pcm = data(&path);
        let lib = Library::open(&p.join("index.db")).unwrap();
        lib.add_root(&p).unwrap();
        let track = lib.search("Song").unwrap().1.remove(0);
        lib.set_favorites(&[track.id], true).unwrap();
        let playlist = lib
            .save_queue_playlist("Keep", std::slice::from_ref(&track))
            .unwrap();
        let suggestion = lib.cleanup_suggestion(track.id).unwrap();
        assert_eq!(
            suggestion
                .iter()
                .find(|v| v.field == MetadataField::FileName)
                .unwrap()
                .value,
            "Artist - Song (Live).wav"
        );
        let mut changes = suggestion;
        changes.push(edit(MetadataField::Album, "Optional album tag"));
        changes.push(edit(MetadataField::TrackNumber, "1"));
        changes.push(edit(MetadataField::Date, "2026-10-01"));
        let preview = lib.prepare_metadata(track.id, &changes).unwrap();
        let id = preview[1].operation_id;
        assert_eq!(fs::read(&path).unwrap(), before);
        assert!(!p.join("Artist - Song (Live).wav").exists());
        let applied = lib.apply_metadata(id).unwrap();
        let after = PathBuf::from(&applied.target_path);
        assert!(!path.exists());
        assert!(after.exists());
        assert_eq!(data(&after), pcm);
        options();
        let tags = lofty::read_from_path(&after).unwrap();
        let tag = tags.primary_tag().unwrap();
        assert_eq!(
            Id3v2Tag::from(tag.clone()).get_user_text("KeepCustomField"),
            Some("Leave me untouched")
        );
        assert_eq!(tag.genre().as_deref(), Some("Existing genre"));
        assert_eq!(
            lib.playlist_tracks(playlist, 0, 500).unwrap()[0].path,
            display_path(&after)
        );
        assert!(lib.favorite(track.id).unwrap());
        assert_eq!(lib.folders(0).unwrap().len(), 1);
        assert_eq!(
            lib.folder(track.folder_id).unwrap().unwrap().kind,
            FolderKind::LooseTracks
        );
        drop(lib);
        let lib = Library::open(&p.join("index.db")).unwrap();
        let undo = lib.undo_metadata(id).unwrap();
        assert_eq!(undo.state, OperationState::Undone);
        assert_eq!(fs::read(&path).unwrap(), before);
        assert!(!after.exists());
        assert_eq!(
            lib.playlist_tracks(playlist, 0, 500).unwrap()[0].path,
            display_path(&path)
        );
        drop(lib);
        fs::remove_dir_all(p).unwrap();
    }
    #[test]
    fn front_cover_edit_preserves_other_pictures_and_operation_backups_stay_out_of_library() {
        use image::{DynamicImage, ImageBuffer, ImageFormat, Rgb};
        fn png(color: [u8; 3]) -> Vec<u8> {
            let mut out = std::io::Cursor::new(Vec::new());
            DynamicImage::ImageRgb8(ImageBuffer::from_pixel(2, 2, Rgb(color)))
                .write_to(&mut out, ImageFormat::Png)
                .unwrap();
            out.into_inner()
        }
        let p = sandbox();
        let path = p.join("song.wav");
        wav(&path);
        let mut tag = Id3v2Tag::new();
        let back = png([1, 2, 3]);
        for (kind, bytes) in [
            (PictureType::CoverFront, png([4, 5, 6])),
            (PictureType::CoverBack, back.clone()),
        ] {
            let mut picture = Picture::from_reader(&mut std::io::Cursor::new(bytes)).unwrap();
            picture.set_pic_type(kind);
            tag.insert_picture(picture);
        }
        tag.save_to_path(&path, WriteOptions::default()).unwrap();
        let before = fs::read(&path).unwrap();
        let lib = Library::open(&p.join("index.db")).unwrap();
        lib.add_root(&p).unwrap();
        let track = lib.search("song").unwrap().1.remove(0);
        let replacement = png([7, 8, 9]);
        let cover = p.join("cover.png");
        fs::write(&cover, &replacement).unwrap();
        let id = lib
            .prepare_metadata(
                track.id,
                &[edit(MetadataField::Artwork, &display_path(&cover))],
            )
            .unwrap()[1]
            .operation_id;
        lib.apply_metadata(id).unwrap();
        options();
        let file = lofty::read_from_path(&path).unwrap();
        let tag = file.primary_tag().unwrap();
        assert_eq!(
            tag.pictures()
                .iter()
                .find(|p| p.pic_type() == PictureType::CoverFront)
                .unwrap()
                .data(),
            replacement
        );
        assert_eq!(
            tag.pictures()
                .iter()
                .find(|p| p.pic_type() == PictureType::CoverBack)
                .unwrap()
                .data(),
            back
        );
        lib.reconcile().unwrap();
        assert_eq!(lib.search("song").unwrap().1.len(), 1);
        assert!(
            lib.folders(track.folder_id).unwrap().is_empty(),
            "App backup directories are excluded from a containing music root"
        );
        lib.undo_metadata(id).unwrap();
        assert_eq!(fs::read(&path).unwrap(), before);
        drop(lib);
        fs::remove_dir_all(p).unwrap();
    }
    #[test]
    fn filename_only_rename_preserves_bytes_and_reconciles_interrupted_apply() {
        let p = sandbox();
        let (lib, track) = indexed(&p, "song.wav");
        let path = PathBuf::from(&track.path);
        let before = fs::read(&path).unwrap();
        let preview = lib
            .prepare_metadata(track.id, &[edit(MetadataField::FileName, "renamed.wav")])
            .unwrap();
        let id = preview[1].operation_id;
        let (_, staged) = lib.stage_paths(id, &path);
        assert_eq!(fs::read(staged).unwrap(), before);
        lib.set_operation(
            id,
            OperationState::Applying,
            "fixture crash after filename-only rename",
        )
        .unwrap();
        let owned = lock(&path, true).unwrap();
        rename_locked(&owned, &p.join("renamed.wav")).unwrap();
        drop(owned);
        lib.reconcile_metadata_operations().unwrap();
        assert_eq!(
            lib.operation(id).unwrap().record.state,
            OperationState::Applied
        );
        lib.undo_metadata(id).unwrap();
        assert_eq!(fs::read(&path).unwrap(), before);
        drop(lib);
        fs::remove_dir_all(p).unwrap();
    }
    #[test]
    fn external_edits_collisions_and_damaged_backups_are_never_overwritten() {
        let p = sandbox();
        let (lib, track) = indexed(&p, "song.wav");
        let path = PathBuf::from(&track.path);
        let before = fs::read(&path).unwrap();
        let id = lib
            .prepare_metadata(
                track.id,
                &[
                    edit(MetadataField::Title, "Changed"),
                    edit(MetadataField::FileName, "new.wav"),
                ],
            )
            .unwrap()[1]
            .operation_id;
        wav(&p.join("new.wav"));
        assert!(lib.apply_metadata(id).is_err());
        assert_eq!(fs::read(&path).unwrap(), before);
        fs::remove_file(p.join("new.wav")).unwrap();
        fs::OpenOptions::new()
            .append(true)
            .open(&path)
            .unwrap()
            .write_all(b"External bytes")
            .unwrap();
        let external = fs::read(&path).unwrap();
        assert!(lib.apply_metadata(id).is_err());
        assert_eq!(fs::read(&path).unwrap(), external);
        fs::write(&path, &before).unwrap();
        let (_, prepared) = lib.stage_paths(id, &path);
        let staged = fs::read(&prepared).unwrap();
        fs::write(&prepared, b"damaged").unwrap();
        assert!(lib.apply_metadata(id).is_err());
        assert_eq!(fs::read(&path).unwrap(), before);
        fs::write(&prepared, staged).unwrap();
        lib.apply_metadata(id).unwrap();
        let after = p.join("new.wav");
        fs::OpenOptions::new()
            .append(true)
            .open(&after)
            .unwrap()
            .write_all(b"Later edit")
            .unwrap();
        let changed = fs::read(&after).unwrap();
        assert!(lib.undo_metadata(id).is_err());
        assert_eq!(fs::read(&after).unwrap(), changed);
        assert!(!path.exists());
        drop(lib);
        fs::remove_dir_all(p).unwrap();
    }
    #[test]
    fn sql_failure_rolls_back_file_bytes_and_rename_and_keeps_prepared_operation() {
        let p = sandbox();
        let (lib, track) = indexed(&p, "song.wav");
        let path = PathBuf::from(&track.path);
        let before = fs::read(&path).unwrap();
        let id = lib
            .prepare_metadata(
                track.id,
                &[
                    edit(MetadataField::Title, "New title"),
                    edit(MetadataField::FileName, "renamed.wav"),
                ],
            )
            .unwrap()[1]
            .operation_id;
        lib.db.execute_batch("CREATE TRIGGER fail_update BEFORE UPDATE OF title ON tracks BEGIN SELECT RAISE(FAIL,'fixture SQL failure'); END;").unwrap();
        assert!(lib.apply_metadata(id).is_err());
        assert_eq!(fs::read(&path).unwrap(), before);
        assert!(!p.join("renamed.wav").exists());
        assert_eq!(
            lib.operation(id).unwrap().record.state,
            OperationState::Prepared
        );
        lib.db.execute_batch("DROP TRIGGER fail_update;").unwrap();
        lib.apply_metadata(id).unwrap();
        lib.undo_metadata(id).unwrap();
        assert_eq!(fs::read(&path).unwrap(), before);
        drop(lib);
        fs::remove_dir_all(p).unwrap();
    }
    #[test]
    fn restart_reconciles_completed_write_and_explicit_recovery_preserves_torn_bytes() {
        let p = sandbox();
        let (lib, track) = indexed(&p, "song.wav");
        let path = PathBuf::from(&track.path);
        let original = fs::read(&path).unwrap();
        let id = lib
            .prepare_metadata(
                track.id,
                &[
                    edit(MetadataField::Title, "New title"),
                    edit(MetadataField::FileName, "renamed.wav"),
                ],
            )
            .unwrap()[1]
            .operation_id;
        let (_, prepared) = lib.stage_paths(id, &path);
        lib.set_operation(id, OperationState::Applying, "fixture crash")
            .unwrap();
        let mut owned = lock(&path, true).unwrap();
        rename_locked(&owned, &p.join("renamed.wav")).unwrap();
        copy_to(&mut fs::File::open(&prepared).unwrap(), &mut owned).unwrap();
        drop(owned);
        drop(lib);
        let lib = Library::open(&p.join("index.db")).unwrap();
        lib.reconcile_metadata_operations().unwrap();
        assert_eq!(
            lib.operation(id).unwrap().record.state,
            OperationState::Applied,
            "{:?}",
            lib.operation(id).unwrap().record
        );
        assert_eq!(
            lib.selected_tracks(&[track.id]).unwrap()[0].path,
            display_path(&p.join("renamed.wav"))
        );
        lib.undo_metadata(id).unwrap();
        assert_eq!(fs::read(&path).unwrap(), original);
        let id = lib
            .prepare_metadata(track.id, &[edit(MetadataField::Title, "Another title")])
            .unwrap()[1]
            .operation_id;
        lib.set_operation(id, OperationState::Applying, "fixture crash")
            .unwrap();
        fs::write(&path, b"Torn content").unwrap();
        drop(lib);
        let lib = Library::open(&p.join("index.db")).unwrap();
        lib.reconcile_metadata_operations().unwrap();
        assert_eq!(
            lib.operation(id).unwrap().record.state,
            OperationState::Conflict
        );
        assert_eq!(fs::read(&path).unwrap(), b"Torn content");
        lib.recover_metadata_original(id).unwrap();
        assert_eq!(fs::read(&path).unwrap(), original);
        let (backup, _) = lib.stage_paths(id, &path);
        let files = fs::read_dir(backup.parent().unwrap())
            .unwrap()
            .map(|e| e.unwrap().path())
            .collect::<Vec<_>>();
        assert!(files.iter().any(|p| {
            p.file_name()
                .unwrap()
                .to_string_lossy()
                .starts_with("recovery-")
                && fs::read(p).unwrap() == b"Torn content"
        }));
        drop(lib);
        fs::remove_dir_all(p).unwrap();
    }
    #[test]
    fn filename_validation_cleanup_and_exclusive_locks_keep_collection_safe() {
        let p = sandbox();
        let (lib, track) = indexed(&p, "song.wav");
        for name in [
            "../song.wav",
            "CON.wav",
            "new.mp3",
            "bad.wav ",
            "folder/song.wav",
            "song:stream.wav",
        ] {
            assert!(
                lib.prepare_metadata(track.id, &[edit(MetadataField::FileName, name)])
                    .is_err()
            );
        }
        assert_eq!(
            clean_download_name("Artist - Song (Live) [Official Video] (320kbps)"),
            "Artist - Song (Live)"
        );
        assert_eq!(clean_download_name("Song (Remix)"), "Song (Remix)");
        assert_eq!(clean_download_name("Song [Acoustic]"), "Song [Acoustic]");
        let other = fs::File::options().write(true).open(&track.path).unwrap();
        assert!(
            lib.prepare_metadata(track.id, &[edit(MetadataField::Title, "No")])
                .is_err()
        );
        drop(other);
        fs::hard_link(&track.path, p.join("link.wav")).unwrap();
        assert!(
            lib.prepare_metadata(track.id, &[edit(MetadataField::Title, "No")])
                .is_err()
        );
        fs::remove_file(p.join("link.wav")).unwrap();
        let id = lib
            .prepare_metadata(track.id, &[edit(MetadataField::FileName, "SONG.wav")])
            .unwrap()[1]
            .operation_id;
        lib.apply_metadata(id).unwrap();
        assert!(
            lib.selected_tracks(&[track.id]).unwrap()[0]
                .path
                .ends_with("SONG.wav")
        );
        lib.undo_metadata(id).unwrap();
        drop(lib);
        fs::remove_dir_all(p).unwrap();
    }
    #[test]
    fn metadata_schema_migration_preserves_existing_user_state() {
        let p = sandbox();
        let (lib, track) = indexed(&p, "song.wav");
        let playlist = lib
            .save_queue_playlist("Keep", std::slice::from_ref(&track))
            .unwrap();
        lib.set_favorites(&[track.id], true).unwrap();
        drop_migration(&lib.db);
        lib.db.execute_batch("PRAGMA user_version=10;").unwrap();
        drop(lib);
        let lib = Library::open(&p.join("index.db")).unwrap();
        assert!(lib.favorite(track.id).unwrap());
        assert_eq!(
            lib.playlist_tracks(playlist, 0, 500).unwrap()[0].id,
            track.id
        );
        assert!(lib.metadata_operations().unwrap().is_empty());
        drop(lib);
        fs::remove_dir_all(p).unwrap();
    }
}
