//! Portable local playlists and explicit file references. Music inputs remain read-only.
use super::*;
use std::io::{Read, Write};
const TEXT_LIMIT: u64 = 8 * 1024 * 1024;
const ITEM_LIMIT: usize = 100_000;
#[derive(Clone, Debug)]
struct Reference {
    path: PathBuf,
    title: String,
    duration: f64,
}
struct CachedReference {
    id: i64,
    folder: Option<i64>,
    size: i64,
    modified: i64,
    identity: Option<String>,
    checked: i64,
    artwork: String,
}
// Folder membership is optional for explicitly opened files. IDs, foreign keys and all triggers remain intact.
pub(super) fn migrate(db: &Connection) -> Result<()> {
    let mut stmt=db.prepare("SELECT sql FROM sqlite_schema WHERE tbl_name='tracks' AND type IN ('index','trigger') AND sql IS NOT NULL ORDER BY type,name").map_err(|e|e.to_string())?;
    let objects = stmt
        .query_map([], |r| r.get::<_, String>(0))
        .map_err(|e| e.to_string())?
        .collect::<std::result::Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;
    drop(stmt);
    db.execute_batch("PRAGMA foreign_keys=OFF;")
        .map_err(|e| e.to_string())?;
    let result = (|| -> Result<()> {
        db.execute_batch("SAVEPOINT migration;
      CREATE TABLE tracks_new(id INTEGER PRIMARY KEY,folder_id INTEGER REFERENCES folders(id) ON DELETE CASCADE,path TEXT NOT NULL UNIQUE COLLATE NOCASE,title TEXT NOT NULL,artist TEXT NOT NULL,album TEXT NOT NULL,album_artist TEXT NOT NULL,artwork TEXT NOT NULL,codec TEXT NOT NULL,duration REAL NOT NULL,number INTEGER NOT NULL,year INTEGER NOT NULL,size INTEGER NOT NULL,modified INTEGER NOT NULL,epoch INTEGER NOT NULL,file_identity TEXT,embedded_art TEXT NOT NULL DEFAULT '',art_checked INTEGER NOT NULL DEFAULT 0,added_at INTEGER NOT NULL DEFAULT 0);
      INSERT INTO tracks_new SELECT id,folder_id,path,title,artist,album,album_artist,artwork,codec,duration,number,year,size,modified,epoch,file_identity,embedded_art,art_checked,added_at FROM tracks;").map_err(|e|e.to_string())?;
        db.execute_batch("DROP TABLE tracks; ALTER TABLE tracks_new RENAME TO tracks;")
            .map_err(|e| e.to_string())?;
        db.execute_batch("ALTER TABLE playlist_items ADD COLUMN duration REAL NOT NULL DEFAULT 0; ALTER TABLE playlist_items ADD COLUMN codec TEXT NOT NULL DEFAULT ''; UPDATE playlist_items SET duration=coalesce((SELECT duration FROM tracks WHERE tracks.id=playlist_items.track_id),0),codec=coalesce((SELECT codec FROM tracks WHERE tracks.id=playlist_items.track_id),'');").map_err(|e| e.to_string())?;
        for sql in objects {
            let sql = if sql.contains("playlist_track_snapshot") {
                sql.replace(
                    "album=old.album WHERE",
                    "album=old.album,duration=old.duration,codec=old.codec WHERE",
                )
            } else {
                sql
            };
            db.execute_batch(&sql).map_err(|e| e.to_string())?;
        }
        let valid: bool = db
            .query_row(
                "SELECT NOT EXISTS(SELECT 1 FROM pragma_foreign_key_check)",
                [],
                |r| r.get(0),
            )
            .map_err(|e| e.to_string())?;
        if !valid {
            return Err("Playlist file migration found invalid library references. Keep the database for recovery.".into());
        }
        db.execute_batch("PRAGMA user_version=10; RELEASE migration;")
            .map_err(|e| e.to_string())
    })();
    if result.is_err() {
        let _ = db.execute_batch("ROLLBACK TO migration; RELEASE migration;");
    }
    db.execute_batch("PRAGMA foreign_keys=ON;")
        .map_err(|e| e.to_string())?;
    result
}
fn local_path(base: &Path, text: &str) -> Result<PathBuf> {
    if text.is_empty()
        || text.len() > 65536
        || text.contains('\0')
        || text.contains("://")
        || text.to_ascii_lowercase().starts_with("file:")
    {
        return Err("Only local audio paths are supported, not URLs or empty paths.".into());
    }
    #[cfg(windows)]
    if text.starts_with(r"\\.\") || text.to_ascii_uppercase().starts_with(r"\\?\GLOBALROOT") {
        return Err("Device paths are not supported.".into());
    }
    let path = Path::new(text);
    #[cfg(windows)]
    if !path.is_absolute()
        && path.components().any(|c| {
            matches!(
                c,
                std::path::Component::Prefix(_) | std::path::Component::RootDir
            )
        })
    {
        return Err("Use a full drive/UNC path or a path relative to the playlist folder.".into());
    }
    let combined = if path.is_absolute() {
        path.to_owned()
    } else {
        base.join(path)
    };
    let absolute = std::path::absolute(combined).map_err(|e| e.to_string())?;
    let mut result = PathBuf::new();
    for component in absolute.components() {
        match component {
            std::path::Component::ParentDir => {
                if result.file_name().is_some() {
                    result.pop();
                }
            }
            std::path::Component::CurDir => {}
            other => result.push(other.as_os_str()),
        }
    }
    Ok(result)
}
fn read_m3u(path: &Path) -> Result<(String, Vec<Reference>)> {
    if !is_playlist(path) {
        return Err("Choose a UTF-8 .m3u8 or .m3u playlist.".into());
    }
    let path = std::fs::canonicalize(path).map_err(|e| format!("Cannot open playlist: {e}"))?;
    let mut bytes = Vec::new();
    fs::File::open(&path)
        .map_err(|e| e.to_string())?
        .take(TEXT_LIMIT + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    if bytes.len() as u64 > TEXT_LIMIT {
        return Err("Playlist exceeds the 8 MiB input limit.".into());
    }
    let text = std::str::from_utf8(&bytes)
        .map_err(|_| "This playlist is not UTF-8. Save it as UTF-8/M3U8 before importing.")?
        .trim_start_matches('\u{feff}');
    let mut name = path
        .file_stem()
        .unwrap_or_default()
        .to_string_lossy()
        .into_owned();
    let mut items = Vec::new();
    let mut title = String::new();
    let mut duration = 0.0;
    for (i, line) in text.lines().enumerate() {
        if line.contains('\0') || line.len() > 65536 {
            return Err(format!("Invalid playlist line {}.", i + 1));
        }
        if line.trim().is_empty() {
            continue;
        }
        if let Some(value) = line.strip_prefix("#PLAYLIST:") {
            name = value.trim().to_owned();
            continue;
        }
        if let Some(value) = line.strip_prefix("#EXTINF:") {
            let (seconds, label) = value
                .split_once(',')
                .ok_or_else(|| format!("Invalid EXTINF on line {}.", i + 1))?;
            title = label.to_owned();
            duration = seconds
                .parse::<f64>()
                .ok()
                .filter(|n| n.is_finite() && *n >= 0.0)
                .unwrap_or(0.0);
            continue;
        }
        if line.starts_with('#') {
            continue;
        }
        let target = local_path(path.parent().unwrap(), line)
            .map_err(|e| format!("Playlist line {}: {e}", i + 1))?;
        if !is_audio(&target) {
            return Err(format!(
                "Playlist line {} is not a supported local audio file.",
                i + 1
            ));
        }
        items.push(Reference {
            path: target,
            title: std::mem::take(&mut title),
            duration,
        });
        duration = 0.0;
        if items.len() > ITEM_LIMIT {
            return Err("Playlist exceeds 100,000 entries.".into());
        }
    }
    let name = name.trim().chars().take(100).collect::<String>();
    Ok((
        if name.is_empty() {
            "Imported playlist".into()
        } else {
            name
        },
        items,
    ))
}
fn is_playlist(path: &Path) -> bool {
    path.extension().is_some_and(|e| {
        matches!(
            e.to_string_lossy().to_ascii_lowercase().as_str(),
            "m3u" | "m3u8"
        )
    })
}
fn regular(meta: &fs::Metadata) -> bool {
    if !meta.is_file() || meta.is_symlink() {
        return false;
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        if meta.file_attributes() & 0x400 != 0 {
            return false;
        }
    }
    true
}
impl Library {
    fn referenced_track(&self, reference: &Reference, cache: &mut artwork::Cache) -> Result<Track> {
        let path = local_path(Path::new("."), &display_path(&reference.path))?;
        let mut t = Track {
            path: display_path(&path),
            title: if reference.title.is_empty() {
                path.file_stem()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .into_owned()
            } else {
                reference.title.clone()
            },
            duration: reference.duration,
            codec: path
                .extension()
                .unwrap_or_default()
                .to_string_lossy()
                .to_ascii_uppercase(),
            ..Default::default()
        };
        let meta = match fs::symlink_metadata(&path) {
            Ok(meta) => meta,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                t.title = format!("Unavailable · {}", t.title);
                return Ok(t);
            }
            Err(e) => return Err(format!("Cannot inspect {}: {e}", path.display())),
        };
        if !regular(&meta) {
            return Err(format!("Not a regular audio file: {}", path.display()));
        }
        let path = fs::canonicalize(path).map_err(|e| e.to_string())?;
        let stored = display_path(&path);
        let identity = identity::file_identity(&path);
        let old: Option<CachedReference> = self
            .db
            .query_row(
                "SELECT id,folder_id,size,modified,file_identity,art_checked,embedded_art FROM tracks WHERE path=?",
                [&stored],
                |r| Ok(CachedReference { id:r.get(0)?, folder:r.get(1)?, size:r.get(2)?, modified:r.get(3)?, identity:r.get(4)?, checked:r.get(5)?, artwork:r.get(6)? }),
            )
            .optional()
            .map_err(|e| e.to_string())?;
        if let Some(old) = &old {
            if old.identity.is_some() && identity.is_some() && old.identity != identity {
                self.db
                    .execute("DELETE FROM tracks WHERE id=?", [old.id])
                    .map_err(|e| e.to_string())?;
            } else if old.folder.is_some()
                || (old.size == meta.len() as i64
                    && old.modified == stamp(&meta)
                    && (old.checked == 1 || cache.full())
                    && (old.artwork.is_empty() || Path::new(&old.artwork).is_file()))
            {
                return self
                    .selected_tracks(&[old.id])
                    .map(|mut rows| rows.remove(0));
            }
        }
        let id = self.row_id("tracks", &stored)?;
        let t = read_track(&path, 0, cache);
        self.db.execute("INSERT INTO tracks(id,folder_id,path,title,artist,album,album_artist,artwork,codec,duration,number,year,size,modified,epoch,file_identity,embedded_art,art_checked,added_at) VALUES(?,NULL,?,?,?,?,?,?,?,?,?,?,?, ?,0,?,?,?,unixepoch()) ON CONFLICT(path) DO UPDATE SET title=excluded.title,artist=excluded.artist,album=excluded.album,album_artist=excluded.album_artist,artwork=excluded.artwork,codec=excluded.codec,duration=excluded.duration,number=excluded.number,year=excluded.year,size=excluded.size,modified=excluded.modified,file_identity=excluded.file_identity,embedded_art=excluded.embedded_art,art_checked=excluded.art_checked",params![id,stored,t.title,t.artist,t.album,t.album_artist,t.artwork,t.codec,t.duration,t.number,t.year,meta.len() as i64,stamp(&meta),identity,t.artwork,i32::from(!cache.retry)]).map_err(|e|e.to_string())?;
        self.selected_tracks(&[id]).map(|mut rows| rows.remove(0))
    }
    pub fn import_playlist(&self, path: &Path) -> Result<(i64, Vec<String>)> {
        let (name, references) = read_m3u(path)?;
        let tx = self.db.unchecked_transaction().map_err(|e| e.to_string())?;
        let mut cache = artwork::Cache::new(self.artwork_dir.clone());
        let rows = references
            .iter()
            .map(|r| self.referenced_track(r, &mut cache))
            .collect::<Result<Vec<_>>>()?;
        let id = self.create_playlist_tracks(&name, &rows)?;
        tx.commit().map_err(|e| e.to_string())?;
        Ok((id, cache.warnings))
    }
    // Caller owns the transaction. Snapshots support unavailable imported/queued occurrences.
    pub(super) fn create_playlist_tracks(&self, name: &str, rows: &[Track]) -> Result<i64> {
        let name = name.trim();
        if name.is_empty() || name.chars().count() > 100 || rows.len() > ITEM_LIMIT {
            return Err(
                "Choose a playlist name of 1–100 characters and at most 100,000 entries.".into(),
            );
        }
        self.db
            .execute("INSERT INTO playlists(name) VALUES(?)", [name])
            .map_err(|e| e.to_string())?;
        let id = self.db.last_insert_rowid();
        for (position, t) in rows.iter().enumerate() {
            self.db.execute("INSERT INTO playlist_items(playlist_id,track_id,path,title,artist,album,position,duration,codec) VALUES(?,(SELECT id FROM tracks WHERE id=?),?,?,?,?,?,?,?)",params![id,t.id,t.path,if t.id == 0 { t.title.strip_prefix("Unavailable · ").unwrap_or(&t.title) } else { &t.title },t.artist,t.album,position as i64,t.duration,t.codec]).map_err(|e|e.to_string())?;
        }
        Ok(id)
    }
    pub fn save_queue_playlist(&self, name: &str, rows: &[Track]) -> Result<i64> {
        let tx = self.db.unchecked_transaction().map_err(|e| e.to_string())?;
        let id = self.create_playlist_tracks(name, rows)?;
        tx.commit().map_err(|e| e.to_string())?;
        Ok(id)
    }
    pub fn dropped_tracks(&self, paths: &[PathBuf]) -> Result<(Vec<Track>, Vec<String>)> {
        if paths.is_empty() || paths.len() > ITEM_LIMIT {
            return Err("Drop local audio files, folders or M3U8 playlists.".into());
        }
        let mut references = Vec::new();
        for path in paths {
            expand(path, &mut references)?;
        }
        if references.is_empty() {
            return Err("No supported audio files in this drop.".into());
        }
        let tx = self.db.unchecked_transaction().map_err(|e| e.to_string())?;
        let mut cache = artwork::Cache::new(self.artwork_dir.clone());
        let rows = references
            .iter()
            .map(|r| self.referenced_track(r, &mut cache))
            .collect::<Result<Vec<_>>>()?;
        tx.commit().map_err(|e| e.to_string())?;
        Ok((rows, cache.warnings))
    }
    pub(super) fn reconcile_external(&self) -> Result<ScanReport> {
        let paths = self
            .db
            .prepare("SELECT path FROM tracks WHERE folder_id IS NULL")
            .map_err(|e| e.to_string())?
            .query_map([], |r| r.get::<_, String>(0))
            .map_err(|e| e.to_string())?
            .collect::<std::result::Result<Vec<_>, _>>()
            .map_err(|e| e.to_string())?;
        let tx = self.db.unchecked_transaction().map_err(|e| e.to_string())?;
        let mut cache = artwork::Cache::new(self.artwork_dir.clone());
        let mut report = ScanReport::default();
        for path in paths {
            let path = PathBuf::from(path);
            if !path.parent().is_some_and(Path::is_dir) {
                report.warnings.push(format!(
                    "External music folder is unavailable: {}. Its cached references were kept.",
                    path.display()
                ));
                continue;
            }
            match fs::symlink_metadata(&path) {
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                    report.changed |= self
                        .db
                        .execute(
                            "DELETE FROM tracks WHERE path=? AND folder_id IS NULL",
                            [display_path(&path)],
                        )
                        .map_err(|e| e.to_string())?
                        > 0;
                }
                Ok(meta) if regular(&meta) => {
                    let old: (i64, i64, Option<String>, i64, String) = self
                        .db
                        .query_row(
                            "SELECT size,modified,file_identity,art_checked,embedded_art FROM tracks WHERE path=?",
                            [display_path(&path)],
                            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?)),
                        )
                        .map_err(|e| e.to_string())?;
                    report.tracks += 1;
                    if old.0 != meta.len() as i64
                        || old.1 != stamp(&meta)
                        || old.2 != identity::file_identity(&path)
                        || (old.3 == 0 && !cache.full())
                        || (!old.4.is_empty() && !Path::new(&old.4).is_file())
                    {
                        self.referenced_track(
                            &Reference {
                                path,
                                title: String::new(),
                                duration: 0.0,
                            },
                            &mut cache,
                        )?;
                        report.changed = true;
                    }
                }
                other => report.warnings.push(format!(
                    "Cannot refresh external file {}: {:?}",
                    path.display(),
                    other.err()
                )),
            }
        }
        tx.commit().map_err(|e| e.to_string())?;
        report.warnings.extend(cache.warnings);
        Ok(report)
    }
    pub fn export_playlist(&self, id: i64, destination: &Path, relative: bool) -> Result<()> {
        if !destination
            .extension()
            .is_some_and(|e| e.eq_ignore_ascii_case("m3u8"))
        {
            return Err("Export to a .m3u8 file.".into());
        }
        let destination = std::path::absolute(destination).map_err(|e| e.to_string())?;
        let parent = fs::canonicalize(destination.parent().ok_or("Choose an export folder.")?)
            .map_err(|e| e.to_string())?;
        let destination = parent.join(destination.file_name().ok_or("Choose a file name.")?);
        let tx = self.db.unchecked_transaction().map_err(|e| e.to_string())?;
        let name = self
            .playlists()?
            .into_iter()
            .find(|p| p.id == id)
            .ok_or("This playlist is no longer available.")?
            .name;
        let rows = self.playlist_tracks(id, 0, ITEM_LIMIT as u32 + 1)?;
        tx.commit().map_err(|e| e.to_string())?;
        if rows.len() > ITEM_LIMIT {
            return Err("Playlist exceeds 100,000 entries.".into());
        }
        let clean = |s: &str| s.replace(['\r', '\n', '\0'], " ");
        let mut output = format!("#EXTM3U\n#PLAYLIST:{}\n", clean(&name));
        for t in rows {
            let path = local_path(Path::new("."), &t.path)?;
            let rendered = if relative {
                relative_path(&parent, &path).unwrap_or_else(|| display_path(&path))
            } else {
                display_path(&path)
            };
            if rendered.contains(['\r', '\n', '\0']) {
                return Err("A path contains characters that M3U8 cannot represent.".into());
            }
            let title = if t.id == 0 {
                t.title.strip_prefix("Unavailable · ").unwrap_or(&t.title)
            } else {
                &t.title
            };
            let label = if t.artist.is_empty() {
                clean(title)
            } else {
                format!("{} - {}", clean(&t.artist), clean(title))
            };
            output.push_str(&format!(
                "#EXTINF:{},{}\n{}\n",
                if t.duration > 0.0 {
                    t.duration.floor() as i64
                } else {
                    -1
                },
                label,
                rendered
            ));
            if output.len() as u64 > TEXT_LIMIT {
                return Err("Export exceeds 8 MiB. Split the playlist into smaller lists.".into());
            }
        }
        publish_export(&destination, output.as_bytes())
    }
}
fn expand(path: &Path, references: &mut Vec<Reference>) -> Result<()> {
    if is_playlist(path) {
        let (_, rows) = read_m3u(path)?;
        references.extend(rows);
    } else {
        let mut stack = vec![std::path::absolute(path).map_err(|e| e.to_string())?];
        let mut visited = 0;
        while let Some(path) = stack.pop() {
            visited += 1;
            if visited > 1_000_000 {
                return Err("This folder tree is too large for one queue drop.".into());
            }
            let meta = fs::symlink_metadata(&path)
                .map_err(|e| format!("Cannot read dropped path {}: {e}", path.display()))?;
            if meta.is_symlink() {
                continue;
            }
            #[cfg(windows)]
            {
                use std::os::windows::fs::MetadataExt;
                if meta.file_attributes() & 0x400 != 0 {
                    continue;
                }
            }
            if meta.is_dir() {
                let mut children = Vec::new();
                for entry in fs::read_dir(&path).map_err(|e| e.to_string())? {
                    if visited + stack.len() + children.len() >= 1_000_000 {
                        return Err("This folder tree is too large for one queue drop.".into());
                    }
                    children.push(entry.map_err(|e| e.to_string())?.path());
                }
                children.sort_by_cached_key(|p| p.to_string_lossy().to_lowercase());
                stack.extend(children.into_iter().rev());
            } else if meta.is_file() && is_audio(&path) {
                references.push(Reference {
                    path,
                    title: String::new(),
                    duration: 0.0,
                });
            } else if visited == 1 {
                return Err("Drop supported audio, a folder or a UTF-8 M3U8 playlist.".into());
            }
            if references.len() > ITEM_LIMIT {
                return Err("Drop exceeds 100,000 entries.".into());
            }
        }
    }
    if references.len() > ITEM_LIMIT {
        return Err("Drop exceeds 100,000 entries.".into());
    }
    Ok(())
}
fn relative_path(base: &Path, target: &Path) -> Option<String> {
    let base = PathBuf::from(display_path(base));
    let target = PathBuf::from(display_path(target));
    let b = base.components().collect::<Vec<_>>();
    let t = target.components().collect::<Vec<_>>();
    if b.first()?.as_os_str().to_string_lossy().to_lowercase()
        != t.first()?.as_os_str().to_string_lossy().to_lowercase()
    {
        return None;
    }
    let mut common = 0;
    while common < b.len()
        && common < t.len()
        && b[common].as_os_str().to_string_lossy().to_lowercase()
            == t[common].as_os_str().to_string_lossy().to_lowercase()
    {
        common += 1;
    }
    let mut result = PathBuf::new();
    for _ in common..b.len() {
        result.push("..");
    }
    for component in &t[common..] {
        result.push(component.as_os_str());
    }
    let text = result.to_string_lossy().replace('\\', "/");
    Some(if text.starts_with('#') {
        format!("./{text}")
    } else {
        text
    })
}
fn publish_export(destination: &Path, bytes: &[u8]) -> Result<()> {
    use std::sync::atomic::{AtomicU64, Ordering};
    static NEXT: AtomicU64 = AtomicU64::new(0);
    let temp = destination.with_extension(format!(
        "spiewak-{}-{}.tmp",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    let mut created = false;
    let result = (|| -> Result<()> {
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temp)
            .map_err(|e| e.to_string())?;
        created = true;
        file.write_all(bytes).map_err(|e| e.to_string())?;
        file.sync_all().map_err(|e| e.to_string())?;
        drop(file);
        publish_new(&temp,destination).map_err(|e|format!("Cannot export playlist: {e}. Choose a new filename; existing files are never overwritten."))?;
        Ok(())
    })();
    if created {
        let _ = fs::remove_file(temp);
    }
    result
}
#[cfg(windows)]
fn publish_new(source: &Path, target: &Path) -> std::io::Result<()> {
    use std::os::windows::ffi::OsStrExt;
    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn MoveFileExW(from: *const u16, to: *const u16, flags: u32) -> i32;
    }
    let from = source
        .as_os_str()
        .encode_wide()
        .chain(Some(0))
        .collect::<Vec<_>>();
    let to = target
        .as_os_str()
        .encode_wide()
        .chain(Some(0))
        .collect::<Vec<_>>();
    if unsafe { MoveFileExW(from.as_ptr(), to.as_ptr(), 0) } == 0 {
        Err(std::io::Error::last_os_error())
    } else {
        Ok(())
    }
}
#[cfg(not(windows))]
fn publish_new(source: &Path, target: &Path) -> std::io::Result<()> {
    fs::hard_link(source, target)
}

#[cfg(test)]
pub(super) fn drop_migration(db: &Connection) {
    db.execute_batch("DROP TRIGGER playlist_track_snapshot; ALTER TABLE playlist_items DROP COLUMN duration; ALTER TABLE playlist_items DROP COLUMN codec; CREATE TRIGGER playlist_track_snapshot BEFORE DELETE ON tracks BEGIN UPDATE playlist_items SET path=old.path,title=old.title,artist=old.artist,album=old.album WHERE track_id=old.id; END;").unwrap();
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::{sandbox, wav};
    #[test]
    fn utf8_import_preserves_duplicates_missing_paths_and_external_file_identity() {
        let p = sandbox();
        let song = p.join("Żółć.wav");
        wav(&song);
        let before = fs::read(&song).unwrap();
        let list = p.join("mix.m3u8");
        fs::write(&list,"\u{feff}#EXTM3U\r\n#PLAYLIST:Moje utwory\r\nŻółć.wav\r\n./Żółć.wav\r\n#EXTINF:42,Missing title\r\nmissing.wav\r\n").unwrap();
        let db = p.join("index.db");
        let lib = Library::open(&db).unwrap();
        let (id, warnings) = lib.import_playlist(&list).unwrap();
        assert!(warnings.is_empty());
        let rows = lib.playlist_tracks(id, 0, 500).unwrap();
        assert_eq!(rows.len(), 3);
        assert!(rows[0].id > 0);
        assert_eq!(rows[0].id, rows[1].id);
        assert_eq!(rows[0].folder_id, 0);
        assert_eq!(rows[2].id, 0);
        assert_eq!(rows[2].title, "Unavailable · Missing title");
        assert_eq!(rows[2].duration, 42.0);
        assert_eq!(lib.playlists().unwrap()[0].name, "Moje utwory");
        assert!(lib.roots().unwrap().is_empty());
        assert!(lib.folders(0).unwrap().is_empty());
        lib.set_favorites(&[rows[0].id], true).unwrap();
        let saved = lib.save_queue_playlist("Queue", &rows).unwrap();
        let exported = p.join("export.m3u8");
        lib.export_playlist(saved, &exported, true).unwrap();
        let text = fs::read_to_string(&exported).unwrap();
        assert!(text.contains("\nŻółć.wav\n"));
        assert!(!text.contains("\\?\\"));
        assert!(text.contains("#EXTINF:42,Missing title"));
        assert!(lib.export_playlist(saved, &exported, true).is_err());
        assert_eq!(fs::read_to_string(&exported).unwrap(), text);
        let round = lib.import_playlist(&exported).unwrap().0;
        assert_eq!(
            lib.playlist_tracks(round, 0, 500).unwrap()[2].duration,
            42.0
        );
        let literal = p.join("Unavailable · song.wav");
        wav(&literal);
        let literal_rows = lib
            .dropped_tracks(std::slice::from_ref(&literal))
            .unwrap()
            .0;
        let literal_list = lib
            .save_queue_playlist("Literal title", &literal_rows)
            .unwrap();
        let literal_export = p.join("literal.m3u8");
        lib.export_playlist(literal_list, &literal_export, true)
            .unwrap();
        assert!(
            fs::read_to_string(literal_export)
                .unwrap()
                .contains("#EXTINF:0,Unavailable · song")
        );
        let report = lib.add_root(&p).unwrap();
        assert!(report.changed);
        let promoted = lib.selected_tracks(&[rows[0].id]).unwrap();
        assert!(promoted[0].folder_id > 0);
        assert!(promoted[0].favorite);
        assert_eq!(fs::read(&song).unwrap(), before);
        drop(lib);
        fs::remove_dir_all(p).unwrap();
    }
    #[test]
    fn malformed_import_and_failed_drop_roll_back_without_creating_roots() {
        let p = sandbox();
        wav(&p.join("good.wav"));
        fs::write(
            p.join("bad.m3u8"),
            b"good.wav\nhttps://example.invalid/music.mp3\n",
        )
        .unwrap();
        let lib = Library::open(&p.join("index.db")).unwrap();
        assert!(lib.import_playlist(&p.join("bad.m3u8")).is_err());
        fs::write(p.join("bad.m3u8"), [0xff, 0xfe]).unwrap();
        assert!(lib.import_playlist(&p.join("bad.m3u8")).is_err());
        fs::create_dir(p.join("not-a-file.wav")).unwrap();
        fs::write(p.join("bad.m3u8"), "good.wav\nnot-a-file.wav\n").unwrap();
        assert!(lib.import_playlist(&p.join("bad.m3u8")).is_err());
        assert!(lib.playlists().unwrap().is_empty());
        assert!(lib.search("good").unwrap().1.is_empty());
        assert!(
            lib.dropped_tracks(&[p.join("good.wav"), p.join("bad.m3u8")])
                .is_err()
        );
        assert!(lib.roots().unwrap().is_empty());
        drop(lib);
        fs::remove_dir_all(p).unwrap();
    }
    #[test]
    fn folder_drop_keeps_input_order_and_does_not_index_neighbors() {
        let p = sandbox();
        fs::create_dir(p.join("folder")).unwrap();
        wav(&p.join("folder/b.wav"));
        wav(&p.join("folder/a.wav"));
        wav(&p.join("neighbor.wav"));
        let lib = Library::open(&p.join("index.db")).unwrap();
        let (rows, _) = lib
            .dropped_tracks(&[p.join("folder"), p.join("folder/b.wav")])
            .unwrap();
        assert_eq!(
            rows.iter().map(|t| t.title.as_str()).collect::<Vec<_>>(),
            ["a", "b", "b"]
        );
        assert!(rows.iter().all(|t| t.folder_id == 0));
        assert!(lib.search("neighbor").unwrap().1.is_empty());
        assert!(lib.folders(0).unwrap().is_empty());
        let id = lib.save_queue_playlist("Drop", &rows).unwrap();
        fs::remove_file(p.join("folder/b.wav")).unwrap();
        lib.reconcile().unwrap();
        assert_eq!(lib.playlist_tracks(id, 0, 500).unwrap()[1].id, 0);
        wav(&p.join("folder/b.wav"));
        lib.reconcile().unwrap();
        assert_eq!(lib.playlist_tracks(id, 0, 500).unwrap()[1].id, 0);
        drop(lib);
        fs::remove_dir_all(p).unwrap();
    }
    #[cfg(windows)]
    #[test]
    fn replaced_external_file_never_inherits_favorites_or_saved_occurrences() {
        let p = sandbox();
        let song = p.join("song.wav");
        wav(&song);
        let lib = Library::open(&p.join("index.db")).unwrap();
        let rows = lib.dropped_tracks(std::slice::from_ref(&song)).unwrap().0;
        lib.set_favorites(&[rows[0].id], true).unwrap();
        let id = lib.save_queue_playlist("Original", &rows).unwrap();
        let modified = fs::metadata(&song).unwrap().modified().unwrap();
        fs::rename(&song, p.join("old.wav")).unwrap();
        wav(&song);
        fs::File::options()
            .write(true)
            .open(&song)
            .unwrap()
            .set_modified(modified)
            .unwrap();
        lib.reconcile().unwrap();
        let new = lib.dropped_tracks(std::slice::from_ref(&song)).unwrap().0;
        assert_ne!(new[0].id, rows[0].id);
        assert!(!new[0].favorite);
        assert_eq!(lib.playlist_tracks(id, 0, 500).unwrap()[0].id, 0);
        drop(lib);
        fs::remove_dir_all(p).unwrap();
    }
    #[cfg(windows)]
    #[test]
    fn promotion_does_not_attach_a_replacement_to_an_old_external_reference() {
        let p = sandbox();
        let song = p.join("song.wav");
        wav(&song);
        let lib = Library::open(&p.join("index.db")).unwrap();
        let original = lib
            .dropped_tracks(std::slice::from_ref(&song))
            .unwrap()
            .0
            .remove(0);
        lib.set_favorites(&[original.id], true).unwrap();
        let playlist = lib
            .save_queue_playlist("Original", std::slice::from_ref(&original))
            .unwrap();
        let modified = fs::metadata(&song).unwrap().modified().unwrap();
        fs::rename(&song, p.join("old.wav")).unwrap();
        wav(&song);
        fs::File::options()
            .write(true)
            .open(&song)
            .unwrap()
            .set_modified(modified)
            .unwrap();
        lib.add_root(&p).unwrap();
        let new = lib
            .search("song")
            .unwrap()
            .1
            .into_iter()
            .find(|t| t.path == display_path(&song))
            .unwrap();
        assert_ne!(new.id, original.id);
        assert!(!new.favorite);
        assert_eq!(lib.playlist_tracks(playlist, 0, 500).unwrap()[0].id, 0);
        drop(lib);
        fs::remove_dir_all(p).unwrap();
    }
    #[test]
    fn migration_preserves_foreign_keys_search_and_snapshots_and_rolls_back_failure() {
        let p = sandbox();
        wav(&p.join("song.wav"));
        let db = p.join("index.db");
        let lib = Library::open(&db).unwrap();
        lib.add_root(&p).unwrap();
        let track = lib.search("song").unwrap().1.remove(0);
        lib.set_favorites(&[track.id], true).unwrap();
        let id = lib
            .save_queue_playlist("Saved", std::slice::from_ref(&track))
            .unwrap();
        lib.db.execute_batch("DROP TRIGGER playlist_track_snapshot; ALTER TABLE playlist_items DROP COLUMN duration; ALTER TABLE playlist_items DROP COLUMN codec; CREATE TRIGGER playlist_track_snapshot BEFORE DELETE ON tracks BEGIN UPDATE playlist_items SET path=old.path,title=old.title,artist=old.artist,album=old.album WHERE track_id=old.id; END; PRAGMA user_version=9;").unwrap();
        drop(lib);
        let lib = Library::open(&db).unwrap();
        assert_eq!(lib.search("song").unwrap().1[0].id, track.id);
        assert!(lib.favorite(track.id).unwrap());
        assert_eq!(lib.playlist_tracks(id, 0, 500).unwrap()[0].id, track.id);
        fs::remove_file(p.join("song.wav")).unwrap();
        lib.reconcile().unwrap();
        assert_eq!(lib.playlist_tracks(id, 0, 500).unwrap()[0].id, 0);
        assert!(lib.playlist_tracks(id, 0, 500).unwrap()[0].duration > 0.0);
        lib.db
            .execute_batch("PRAGMA user_version=9; CREATE TABLE tracks_new(collision INTEGER);")
            .unwrap();
        drop(lib);
        assert!(Library::open(&db).is_err());
        let db = Connection::open(&db).unwrap();
        assert_eq!(
            db.query_row("PRAGMA user_version", [], |r| r.get::<_, i64>(0))
                .unwrap(),
            9
        );
        assert_eq!(
            db.query_row(
                "SELECT count(*) FROM playlist_items WHERE playlist_id=?",
                [id],
                |r| r.get::<_, i64>(0)
            )
            .unwrap(),
            1
        );
        drop(db);
        fs::remove_dir_all(p).unwrap();
    }
    #[cfg(windows)]
    #[test]
    fn relative_paths_handle_unicode_comment_names_and_drive_boundaries() {
        assert_eq!(
            relative_path(Path::new(r"\\?\C:\Music"), Path::new(r"C:\Music\#Żółć.wav")).unwrap(),
            "./#Żółć.wav"
        );
        assert!(relative_path(Path::new(r"C:\Music"), Path::new(r"D:\Music\song.wav")).is_none());
        assert!(local_path(Path::new(r"C:\Music"), r"C:song.wav").is_err());
        assert!(local_path(Path::new(r"C:\Music"), r"\song.wav").is_err());
    }
}
