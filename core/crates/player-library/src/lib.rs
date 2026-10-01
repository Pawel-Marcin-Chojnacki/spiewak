//! Rebuildable SQLite index. Scanning never writes to the music collection.
use lofty::{
    file::{AudioFile, TaggedFileExt},
    tag::{Accessor, ItemKey},
};
use player_domain::{Folder, FolderKind, Track};
mod artwork;
mod identity;
mod listening;
mod monitor;
mod playlists;
mod presentation;
mod session;
pub use monitor::Monitor;
use rusqlite::{Connection, OptionalExtension, params};
use std::{
    fs,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

type Result<T> = std::result::Result<T, String>;
pub struct Library {
    db: Connection,
    artwork_dir: PathBuf,
}
#[derive(Default, Debug)]
pub struct ScanReport {
    pub tracks: u64,
    pub changed: bool,
    pub warnings: Vec<String>,
}

fn display_path(path: &Path) -> String {
    let value = path.to_string_lossy();
    if let Some(unc) = value.strip_prefix(r"\\?\UNC\") {
        format!(r"\\{unc}")
    } else {
        value.strip_prefix(r"\\?\").unwrap_or(&value).to_owned()
    }
}
fn stamp(meta: &fs::Metadata) -> i64 {
    meta.modified()
        .unwrap_or(SystemTime::UNIX_EPOCH)
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos() as i64
}
fn is_audio(path: &Path) -> bool {
    // AppleDouble sidecars from macOS shares have audio extensions but contain no audio.
    if path
        .file_name()
        .is_some_and(|n| n.to_string_lossy().starts_with("._"))
    {
        return false;
    }
    path.extension().is_some_and(|e| {
        matches!(
            e.to_string_lossy().to_ascii_lowercase().as_str(),
            "mp3" | "flac" | "m4a" | "aac" | "ogg" | "opus" | "wav" | "aiff" | "aif"
        )
    })
}
fn read_track(path: &Path, folder_id: i64, cache: &mut artwork::Cache) -> Track {
    cache.retry = false;
    let mut track = Track {
        folder_id,
        path: display_path(path),
        title: path
            .file_stem()
            .unwrap_or_default()
            .to_string_lossy()
            .into_owned(),
        codec: path
            .extension()
            .unwrap_or_default()
            .to_string_lossy()
            .to_ascii_uppercase(),
        ..Default::default()
    };
    lofty::config::apply_global_options(
        lofty::config::GlobalOptions::new()
            .allocation_limit(8 * 1024 * 1024)
            .preserve_format_specific_items(false),
    );
    let file = lofty::read_from_path(path).or_else(|_| {
        lofty::probe::Probe::open(path).and_then(|p| {
            p.options(lofty::config::ParseOptions::new().read_cover_art(false))
                .read()
        })
    });
    if let Ok(file) = file {
        track.artwork = cache.embedded(&file);
        track.duration = file.properties().duration().as_secs_f64();
        if let Some(tag) = file.primary_tag().or_else(|| file.first_tag()) {
            if let Some(value) = tag.title().filter(|v| !v.trim().is_empty()) {
                track.title = value.into_owned();
            }
            track.artist = tag.artist().map(|v| v.into_owned()).unwrap_or_default();
            track.album = tag.album().map(|v| v.into_owned()).unwrap_or_default();
            track.album_artist = tag
                .get_string(ItemKey::AlbumArtist)
                .unwrap_or_default()
                .to_owned();
            track.number = tag.track().unwrap_or(0);
            track.year = tag.date().map(|date| date.year as u32).unwrap_or(0);
        }
    }
    track
}
impl Library {
    pub fn open(path: &Path) -> Result<Self> {
        let db = Connection::open(path).map_err(|e| e.to_string())?;
        let schema_version: i64 = db
            .query_row("PRAGMA user_version", [], |r| r.get(0))
            .map_err(|e| e.to_string())?;
        if schema_version > 9 {
            return Err("This library cache belongs to a newer version of Spiewak. Update the app before opening it.".into());
        }
        db.busy_timeout(std::time::Duration::from_secs(10))
            .map_err(|e| e.to_string())?;
        db.execute_batch("PRAGMA journal_mode=WAL; PRAGMA foreign_keys=ON;
            CREATE TABLE IF NOT EXISTS library_roots(id INTEGER PRIMARY KEY,path TEXT NOT NULL UNIQUE COLLATE NOCASE);
            CREATE TABLE IF NOT EXISTS folders(id INTEGER PRIMARY KEY,root_id INTEGER NOT NULL REFERENCES library_roots(id),parent_id INTEGER REFERENCES folders(id) ON DELETE CASCADE,name TEXT NOT NULL,path TEXT NOT NULL UNIQUE COLLATE NOCASE,epoch INTEGER NOT NULL DEFAULT 0);
            CREATE INDEX IF NOT EXISTS folders_parent ON folders(parent_id);
            CREATE TABLE IF NOT EXISTS tracks(id INTEGER PRIMARY KEY,folder_id INTEGER NOT NULL REFERENCES folders(id) ON DELETE CASCADE,path TEXT NOT NULL UNIQUE COLLATE NOCASE,title TEXT NOT NULL,artist TEXT NOT NULL,album TEXT NOT NULL,album_artist TEXT NOT NULL,artwork TEXT NOT NULL,codec TEXT NOT NULL,duration REAL NOT NULL,number INTEGER NOT NULL,year INTEGER NOT NULL,size INTEGER NOT NULL,modified INTEGER NOT NULL,epoch INTEGER NOT NULL);
            CREATE INDEX IF NOT EXISTS tracks_folder ON tracks(folder_id,number,title);
            ").map_err(|e|e.to_string())?;
        if schema_version < 2 {
            db.execute_batch("SAVEPOINT migration;
                CREATE VIRTUAL TABLE IF NOT EXISTS track_search USING fts5(title,artist,album,path,content='tracks',content_rowid='id',tokenize='unicode61 remove_diacritics 2',prefix='2 3');
                CREATE TRIGGER IF NOT EXISTS track_search_insert AFTER INSERT ON tracks BEGIN INSERT INTO track_search(rowid,title,artist,album,path) VALUES(new.id,new.title,new.artist,new.album,new.path); END;
                CREATE TRIGGER IF NOT EXISTS track_search_delete AFTER DELETE ON tracks BEGIN INSERT INTO track_search(track_search,rowid,title,artist,album,path) VALUES('delete',old.id,old.title,old.artist,old.album,old.path); END;
                CREATE TRIGGER IF NOT EXISTS track_search_update AFTER UPDATE OF title,artist,album,path ON tracks BEGIN
                    INSERT INTO track_search(track_search,rowid,title,artist,album,path) VALUES('delete',old.id,old.title,old.artist,old.album,old.path);
                    INSERT INTO track_search(rowid,title,artist,album,path) VALUES(new.id,new.title,new.artist,new.album,new.path); END;
                CREATE VIRTUAL TABLE IF NOT EXISTS folder_search USING fts5(name,path,content='folders',content_rowid='id',tokenize='unicode61 remove_diacritics 2',prefix='2 3');
                CREATE TRIGGER IF NOT EXISTS folder_search_insert AFTER INSERT ON folders BEGIN INSERT INTO folder_search(rowid,name,path) VALUES(new.id,new.name,new.path); END;
                CREATE TRIGGER IF NOT EXISTS folder_search_delete AFTER DELETE ON folders BEGIN INSERT INTO folder_search(folder_search,rowid,name,path) VALUES('delete',old.id,old.name,old.path); END;
                CREATE TRIGGER IF NOT EXISTS folder_search_update AFTER UPDATE OF name,path ON folders WHEN old.name!=new.name OR old.path!=new.path BEGIN
                    INSERT INTO folder_search(folder_search,rowid,name,path) VALUES('delete',old.id,old.name,old.path);
                    INSERT INTO folder_search(rowid,name,path) VALUES(new.id,new.name,new.path); END;
                INSERT INTO track_search(track_search) VALUES('rebuild');
                INSERT INTO folder_search(folder_search) VALUES('rebuild');
                PRAGMA user_version=2; RELEASE migration;").map_err(|e|e.to_string())?;
        }
        if schema_version < 3 {
            db.execute_batch("SAVEPOINT migration;
              CREATE TABLE IF NOT EXISTS playlists(id INTEGER PRIMARY KEY,name TEXT NOT NULL,deleted INTEGER NOT NULL DEFAULT 0);
              CREATE TABLE IF NOT EXISTS playlist_items(id INTEGER PRIMARY KEY,playlist_id INTEGER NOT NULL REFERENCES playlists(id) ON DELETE CASCADE,track_id INTEGER REFERENCES tracks(id) ON DELETE SET NULL,path TEXT NOT NULL,title TEXT NOT NULL,artist TEXT NOT NULL,album TEXT NOT NULL,position INTEGER NOT NULL);
              CREATE INDEX IF NOT EXISTS playlist_order ON playlist_items(playlist_id,position,id);
              PRAGMA user_version=3; RELEASE migration;").map_err(|e|e.to_string())?;
        }
        if schema_version < 4 {
            db.execute_batch("SAVEPOINT migration;
              ALTER TABLE folders ADD COLUMN file_identity TEXT;
              ALTER TABLE tracks ADD COLUMN file_identity TEXT;
              CREATE INDEX folder_identity ON folders(file_identity);
              CREATE INDEX track_identity ON tracks(file_identity);
              CREATE TRIGGER playlist_track_snapshot BEFORE DELETE ON tracks BEGIN
                UPDATE playlist_items SET path=old.path,title=old.title,artist=old.artist,album=old.album WHERE track_id=old.id;
              END;
              PRAGMA user_version=4; RELEASE migration;").map_err(|e|e.to_string())?;
        }
        if schema_version < 5 {
            db.execute_batch(
                "SAVEPOINT migration;
              CREATE TABLE entity_ids(kind TEXT PRIMARY KEY,value INTEGER NOT NULL);
              INSERT INTO entity_ids SELECT 'folders',coalesce(max(id),0) FROM folders;
              INSERT INTO entity_ids SELECT 'tracks',coalesce(max(id),0) FROM tracks;
              PRAGMA user_version=5; RELEASE migration;",
            )
            .map_err(|e| e.to_string())?;
        }
        if schema_version < 6 {
            db.execute_batch("SAVEPOINT migration;
              ALTER TABLE folders ADD COLUMN presentation INTEGER NOT NULL DEFAULT 0 CHECK(presentation BETWEEN 0 AND 6);
              PRAGMA user_version=6; RELEASE migration;").map_err(|e|e.to_string())?;
        }
        if schema_version < 7 {
            db.execute_batch("SAVEPOINT migration;
              CREATE TABLE playback_session(id INTEGER PRIMARY KEY CHECK(id=1),current_index INTEGER,next_end INTEGER,position REAL NOT NULL,volume REAL NOT NULL,flags INTEGER NOT NULL);
              CREATE TABLE session_items(position INTEGER PRIMARY KEY,track_id INTEGER REFERENCES tracks(id) ON DELETE SET NULL,path TEXT NOT NULL,title TEXT NOT NULL,artist TEXT NOT NULL,album TEXT NOT NULL,album_artist TEXT NOT NULL,artwork TEXT NOT NULL,codec TEXT NOT NULL,duration REAL NOT NULL,number INTEGER NOT NULL,year INTEGER NOT NULL);
              CREATE TRIGGER session_track_snapshot BEFORE DELETE ON tracks BEGIN
                UPDATE session_items SET path=old.path,title=old.title,artist=old.artist,album=old.album,album_artist=old.album_artist,artwork=old.artwork,codec=old.codec,duration=old.duration,number=old.number,year=old.year WHERE track_id=old.id;
              END;
              PRAGMA user_version=7; RELEASE migration;").map_err(|e|e.to_string())?;
        }
        if schema_version < 8 {
            db.execute_batch(
                "SAVEPOINT migration;
              ALTER TABLE tracks ADD COLUMN embedded_art TEXT NOT NULL DEFAULT '';
              ALTER TABLE tracks ADD COLUMN art_checked INTEGER NOT NULL DEFAULT 0;
              ALTER TABLE folders ADD COLUMN artwork TEXT NOT NULL DEFAULT '';
              ALTER TABLE folders ADD COLUMN art_source TEXT NOT NULL DEFAULT '';
              PRAGMA user_version=8; RELEASE migration;",
            )
            .map_err(|e| e.to_string())?;
        }
        if schema_version < 9 {
            listening::migrate(&db)?;
        }
        Ok(Self {
            db,
            artwork_dir: path.with_extension("artwork"),
        })
    }
    pub fn roots(&self) -> Result<Vec<String>> {
        let mut stmt = self
            .db
            .prepare("SELECT path FROM library_roots ORDER BY id")
            .map_err(|e| e.to_string())?;
        stmt.query_map([], |row| row.get(0))
            .map_err(|e| e.to_string())?
            .collect::<std::result::Result<_, _>>()
            .map_err(|e| e.to_string())
    }
    pub fn add_root(&self, path: &Path) -> Result<ScanReport> {
        let path = fs::canonicalize(path).map_err(|e| format!("Cannot open music folder: {e}"))?;
        if !path.is_dir() {
            return Err("Choose a folder, not a file.".into());
        }
        let value = display_path(&path);
        // A nested root already belongs to its parent. Do not reparent the existing tree.
        for existing in self.roots()? {
            let existing = fs::canonicalize(&existing).unwrap_or_else(|_| PathBuf::from(existing));
            if path == existing {
                return self.scan_root(&value);
            }
            if path.starts_with(&existing) || existing.starts_with(&path) {
                return Err(
                    "This folder overlaps an existing library root. Choose a separate folder."
                        .into(),
                );
            }
        }
        self.db
            .execute("INSERT INTO library_roots(path) VALUES(?)", [&value])
            .map_err(|e| e.to_string())?;
        self.scan_root(&value)
    }
    pub fn reconcile(&self) -> Result<ScanReport> {
        let mut report = ScanReport::default();
        for root in self.roots()? {
            match self.scan_root(&root) {
                Ok(r) => {
                    report.tracks += r.tracks;
                    report.changed |= r.changed;
                    report.warnings.extend(r.warnings)
                }
                Err(e) => report.warnings.push(e),
            }
        }
        Ok(report)
    }
    fn scan_root(&self, root: &str) -> Result<ScanReport> {
        let root_path = PathBuf::from(root);
        if !root_path.is_dir() {
            return Err(format!(
                "Music folder is unavailable: {root}. Its cached index has been kept."
            ));
        }
        let root_id: i64 = self
            .db
            .query_row("SELECT id FROM library_roots WHERE path=?", [root], |r| {
                r.get(0)
            })
            .map_err(|e| e.to_string())?;
        // Clock resolution must not merge two rapid reconciliations into one generation.
        let epoch: i64 = self
            .db
            .query_row(
                "SELECT coalesce(max(epoch),0)+1 FROM folders WHERE root_id=?",
                [root_id],
                |r| r.get(0),
            )
            .map_err(|e| e.to_string())?;
        let mut pending = vec![(root_path, None)];
        let mut report = ScanReport::default();
        let mut incomplete = false;
        let mut cache = artwork::Cache::new(self.artwork_dir.clone());
        cache.prune(&self.db)?;
        // A savepoint makes reconciliation atomic; WAL readers still see the old complete index.
        self.db
            .execute_batch("SAVEPOINT scan")
            .map_err(|e| e.to_string())?;
        let result = (|| -> Result<()> {
            while let Some((folder, parent)) = pending.pop() {
                let folder_path = display_path(&folder);
                let name = folder
                    .file_name()
                    .unwrap_or(folder.as_os_str())
                    .to_string_lossy();
                let previous: Option<(String, Option<i64>)> = self
                    .db
                    .query_row(
                        "SELECT name,parent_id FROM folders WHERE path=?",
                        [&folder_path],
                        |r| Ok((r.get(0)?, r.get(1)?)),
                    )
                    .optional()
                    .map_err(|e| e.to_string())?;
                report.changed |= previous.as_ref().is_none_or(|(old_name, old_parent)| {
                    old_name != name.as_ref() || *old_parent != parent
                });
                let identity = identity::file_identity(&folder);
                self.relocate("folders", &folder_path, identity.as_deref(), root_id)?;
                let id = self.row_id("folders", &folder_path)?;
                self.db.execute("INSERT INTO folders(id,root_id,parent_id,name,path,epoch,file_identity) VALUES(?,?,?,?,?,?,?) ON CONFLICT(path) DO UPDATE SET path=excluded.path,name=excluded.name,parent_id=excluded.parent_id,epoch=excluded.epoch,file_identity=excluded.file_identity",params![id,root_id,parent,name,folder_path,epoch,identity]).map_err(|e|e.to_string())?;
                let folder_id: i64 = self
                    .db
                    .query_row("SELECT id FROM folders WHERE path=?", [&folder_path], |r| {
                        r.get(0)
                    })
                    .map_err(|e| e.to_string())?;
                let previous_art = self
                    .db
                    .query_row(
                        "SELECT artwork,art_source FROM folders WHERE id=?",
                        [folder_id],
                        |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)),
                    )
                    .map_err(|e| e.to_string())?;
                let (cover, source) = cache.folder(&folder, &previous_art);
                report.changed |= cover != previous_art.0;
                self.db
                    .execute(
                        "UPDATE folders SET artwork=?,art_source=? WHERE id=?",
                        params![cover, source, folder_id],
                    )
                    .map_err(|e| e.to_string())?;
                let entries = match fs::read_dir(&folder) {
                    Ok(e) => e,
                    Err(e) => {
                        incomplete = true;
                        report
                            .warnings
                            .push(format!("Cannot read {folder_path}: {e}"));
                        continue;
                    }
                };
                for entry in entries {
                    let entry = match entry {
                        Ok(e) => e,
                        Err(e) => {
                            incomplete = true;
                            report.warnings.push(e.to_string());
                            continue;
                        }
                    };
                    let path = entry.path();
                    let meta = match fs::symlink_metadata(&path) {
                        Ok(m) => m,
                        Err(e) => {
                            incomplete = true;
                            report.warnings.push(e.to_string());
                            continue;
                        }
                    };
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
                        if display_path(&path) == display_path(&self.artwork_dir) {
                            continue;
                        }
                        pending.push((path, Some(folder_id)));
                        continue;
                    }
                    if !meta.is_file() || !is_audio(&path) {
                        continue;
                    }
                    report.tracks += 1;
                    let stored_path = display_path(&path);
                    let modified = stamp(&meta);
                    let identity = identity::file_identity(&path);
                    self.relocate("tracks", &stored_path, identity.as_deref(), root_id)?;
                    let old: Option<(i64, i64, i64, String)> = self
                        .db
                        .query_row(
                            "SELECT size,modified,art_checked,embedded_art FROM tracks WHERE path=?",
                            [&stored_path],
                            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
                        )
                        .optional()
                        .map_err(|e| e.to_string())?;
                    if old.as_ref().is_some_and(|(size, time, checked, art)| {
                        *size == meta.len() as i64
                            && *time == modified
                            && (*checked == 1 || cache.full())
                            && (art.is_empty() || Path::new(art).is_file())
                    }) {
                        self.db
                            .execute(
                                "UPDATE tracks SET epoch=?,file_identity=? WHERE path=?",
                                params![epoch, identity, stored_path],
                            )
                            .map_err(|e| e.to_string())?;
                        continue;
                    }
                    report.changed |= old.as_ref().is_none_or(|(size, time, _, art)| {
                        *size != meta.len() as i64
                            || *time != modified
                            || !art.is_empty() && !Path::new(art).is_file()
                    });
                    let t = read_track(&path, folder_id, &mut cache);
                    report.changed |= old.as_ref().is_none_or(|(_, _, _, art)| art != &t.artwork);
                    let id = self.row_id("tracks", &stored_path)?;
                    self.db.execute("INSERT INTO tracks(id,folder_id,path,title,artist,album,album_artist,artwork,codec,duration,number,year,size,modified,epoch,file_identity,added_at) VALUES(?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,unixepoch()) ON CONFLICT(path) DO UPDATE SET folder_id=excluded.folder_id,title=excluded.title,artist=excluded.artist,album=excluded.album,album_artist=excluded.album_artist,artwork=excluded.artwork,codec=excluded.codec,duration=excluded.duration,number=excluded.number,year=excluded.year,size=excluded.size,modified=excluded.modified,epoch=excluded.epoch,file_identity=excluded.file_identity",params![id,t.folder_id,t.path,t.title,t.artist,t.album,t.album_artist,t.artwork,t.codec,t.duration,t.number,t.year,meta.len() as i64,modified,epoch,identity]).map_err(|e|e.to_string())?;
                    self.db
                        .execute(
                            "UPDATE tracks SET embedded_art=?,art_checked=? WHERE id=?",
                            params![t.artwork, i32::from(!cache.retry), id],
                        )
                        .map_err(|e| e.to_string())?;
                }
            }
            // A partially unreadable root must not make cached tracks disappear.
            if !incomplete {
                report.changed |= self.db.execute("DELETE FROM tracks WHERE epoch!=? AND folder_id IN (SELECT id FROM folders WHERE root_id=?)",params![epoch,root_id]).map_err(|e|e.to_string())? > 0;
                report.changed |= self
                    .db
                    .execute(
                        "DELETE FROM folders WHERE root_id=? AND epoch!=?",
                        params![root_id, epoch],
                    )
                    .map_err(|e| e.to_string())?
                    > 0;
            }
            let mut stmt = self
                .db
                .prepare("SELECT id FROM folders WHERE root_id=?")
                .map_err(|e| e.to_string())?;
            let ids = stmt
                .query_map([root_id], |r| r.get::<_, i64>(0))
                .map_err(|e| e.to_string())?
                .collect::<std::result::Result<Vec<_>, _>>()
                .map_err(|e| e.to_string())?;
            for id in ids {
                if let Some(folder) = self.folder_current(id)? {
                    report.changed |= self.apply_artwork(
                        id,
                        matches!(folder.kind, FolderKind::Album | FolderKind::Compilation),
                    )?;
                }
            }
            Ok(())
        })();
        report.warnings.extend(cache.warnings);
        match result {
            Ok(()) => self.db.execute_batch("RELEASE scan"),
            Err(e) => {
                let _ = self.db.execute_batch("ROLLBACK TO scan; RELEASE scan");
                return Err(e);
            }
        }
        .map_err(|e| e.to_string())?;
        Ok(report)
    }
    // SQLite INTEGER PRIMARY KEY can reuse a deleted highest ID. Runtime queues must
    // never accidentally resolve that old ID to an unrelated newly indexed song.
    fn row_id(&self, table: &str, path: &str) -> Result<i64> {
        let existing = self
            .db
            .query_row(
                &format!("SELECT id FROM {table} WHERE path=?"),
                [path],
                |r| r.get(0),
            )
            .optional()
            .map_err(|e| e.to_string())?;
        if let Some(id) = existing {
            return Ok(id);
        }
        self.db
            .query_row(
                "UPDATE entity_ids SET value=value+1 WHERE kind=? RETURNING value",
                [table],
                |r| r.get(0),
            )
            .map_err(|e| e.to_string())
    }
    // Match only a disappeared path in the same root. Hard links remain separate rows.
    // Creation time is part of identity to reduce accidental reuse of a deleted file ID.
    fn relocate(&self, table: &str, path: &str, identity: Option<&str>, root: i64) -> Result<()> {
        let Some(identity) = identity else {
            return Ok(());
        };
        let scope = if table == "tracks" {
            "folder_id IN (SELECT id FROM folders WHERE root_id=?)"
        } else {
            "root_id=?"
        };
        let sql = format!("SELECT id,path FROM {table} WHERE file_identity=? AND {scope}");
        let mut stmt = self.db.prepare(&sql).map_err(|e| e.to_string())?;
        let rows = stmt
            .query_map(params![identity, root], |r| {
                Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?))
            })
            .map_err(|e| e.to_string())?
            .collect::<std::result::Result<Vec<_>, _>>()
            .map_err(|e| e.to_string())?;
        if let Some((id, old)) = rows.iter().find(|(_, old)| old.eq_ignore_ascii_case(path)) {
            if old != path {
                let refresh = if table == "tracks" {
                    ",modified=-1"
                } else {
                    ""
                };
                self.db
                    .execute(
                        &format!("UPDATE {table} SET path=?{refresh} WHERE id=?"),
                        params![path, id],
                    )
                    .map_err(|e| e.to_string())?;
            }
            return Ok(());
        }
        let missing = rows
            .iter()
            .filter(|(_, old)| !Path::new(old).exists())
            .collect::<Vec<_>>();
        if let [candidate] = missing.as_slice() {
            // Do not overwrite an existing different file at the destination.
            let occupied: bool = self
                .db
                .query_row(
                    &format!("SELECT EXISTS(SELECT 1 FROM {table} WHERE path=?)"),
                    [path],
                    |r| r.get(0),
                )
                .map_err(|e| e.to_string())?;
            if !occupied {
                let refresh = if table == "tracks" {
                    ",modified=-1"
                } else {
                    ""
                };
                self.db
                    .execute(
                        &format!("UPDATE {table} SET path=?{refresh} WHERE id=?"),
                        params![path, candidate.0],
                    )
                    .map_err(|e| e.to_string())?;
            }
        }
        Ok(())
    }
    pub fn folder(&self, id: i64) -> Result<Option<Folder>> {
        let tx = self.db.unchecked_transaction().map_err(|e| e.to_string())?;
        let result = self.folder_current(id)?;
        tx.commit().map_err(|e| e.to_string())?;
        Ok(result)
    }
    // Caller owns the read/write transaction (also used inside a scan savepoint).
    fn folder_current(&self, id: i64) -> Result<Option<Folder>> {
        let folder = self.db.query_row("SELECT f.id,coalesce(f.parent_id,0),f.name,f.path,EXISTS(SELECT 1 FROM folders c WHERE c.parent_id=f.id) FROM folders f WHERE f.id=?", [id], folder_row).optional().map_err(|e|e.to_string())?;
        let result = folder
            .map(|f| {
                self.decorate_folders(vec![f])
                    .map(|mut rows| rows.remove(0))
            })
            .transpose()?;
        Ok(result)
    }
    pub fn selected_tracks(&self, ids: &[i64]) -> Result<Vec<Track>> {
        if ids.is_empty() || ids.len() > 100_000 {
            return Err("Select tracks first (up to 100,000).".into());
        }
        let mut stmt = self.db.prepare("SELECT id,folder_id,path,title,artist,album,album_artist,artwork,codec,duration,number,year,EXISTS(SELECT 1 FROM favorites f WHERE f.original_id=tracks.id),id,added_at FROM tracks WHERE id=?").map_err(|e|e.to_string())?;
        ids.iter()
            .map(|id| {
                stmt.query_row([id], track_row).map_err(|_| {
                    format!("Track {id} is no longer in the library. Rescan its folder.")
                })
            })
            .collect()
    }
    pub fn folders(&self, parent: i64) -> Result<Vec<Folder>> {
        let tx = self.db.unchecked_transaction().map_err(|e| e.to_string())?;
        let mut stmt=self.db.prepare("SELECT f.id,coalesce(f.parent_id,0),f.name,f.path,EXISTS(SELECT 1 FROM folders c WHERE c.parent_id=f.id) FROM folders f WHERE coalesce(f.parent_id,0)=? ORDER BY f.name COLLATE NOCASE").map_err(|e|e.to_string())?;
        let rows = stmt
            .query_map([parent], folder_row)
            .map_err(|e| e.to_string())?
            .collect::<std::result::Result<Vec<_>, _>>()
            .map_err(|e| e.to_string())?;
        let result = self.decorate_folders(rows)?;
        drop(stmt);
        tx.commit().map_err(|e| e.to_string())?;
        Ok(result)
    }
    pub fn tracks(&self, folder: i64, offset: u32, limit: u32) -> Result<Vec<Track>> {
        self.tracks_sorted(folder, offset, limit, 0)
    }
    pub fn tracks_sorted(
        &self,
        folder: i64,
        offset: u32,
        limit: u32,
        sort: u32,
    ) -> Result<Vec<Track>> {
        let order = match sort {
            0 => "number,title COLLATE NOCASE,id",
            1 => "title COLLATE NOCASE,id",
            2 => "artist COLLATE NOCASE,title COLLATE NOCASE,id",
            3 => "path COLLATE NOCASE,id",
            4 => "duration,id",
            5 => "duration DESC,id",
            _ => return Err("Unknown track sort order.".into()),
        };
        let sql = format!(
            "SELECT id,folder_id,path,title,artist,album,album_artist,artwork,codec,duration,number,year,EXISTS(SELECT 1 FROM favorites f WHERE f.original_id=tracks.id),id,added_at FROM tracks WHERE folder_id=? ORDER BY {order} LIMIT ? OFFSET ?"
        );
        let mut stmt = self.db.prepare(&sql).map_err(|e| e.to_string())?;
        stmt.query_map(params![folder, limit, offset], track_row)
            .map_err(|e| e.to_string())?
            .collect::<std::result::Result<_, _>>()
            .map_err(|e| e.to_string())
    }
    pub fn search(&self, input: &str) -> Result<(Vec<Folder>, Vec<Track>)> {
        if input.len() > 1024 {
            return Err("Search text is too long.".into());
        }
        let query = input
            .split(|c: char| !c.is_alphanumeric())
            .filter(|w| !w.is_empty())
            .take(8)
            .map(|w| format!("\"{w}\"*"))
            .collect::<Vec<_>>()
            .join(" AND ");
        if query.is_empty() {
            return Ok((vec![], vec![]));
        }
        let tx = self.db.unchecked_transaction().map_err(|e| e.to_string())?;
        let mut stmt=self.db.prepare("SELECT t.id,t.folder_id,t.path,t.title,t.artist,t.album,t.album_artist,t.artwork,t.codec,t.duration,t.number,t.year,EXISTS(SELECT 1 FROM favorites f WHERE f.original_id=t.id),t.id,t.added_at FROM track_search JOIN tracks t ON t.id=track_search.rowid WHERE track_search MATCH ? ORDER BY rank LIMIT 200").map_err(|e|e.to_string())?;
        let tracks = stmt
            .query_map([&query], track_row)
            .map_err(|e| e.to_string())?
            .collect::<std::result::Result<Vec<_>, _>>()
            .map_err(|e| e.to_string())?;
        let mut stmt=self.db.prepare("SELECT f.id,coalesce(f.parent_id,0),f.name,f.path,EXISTS(SELECT 1 FROM folders c WHERE c.parent_id=f.id) FROM folder_search JOIN folders f ON f.id=folder_search.rowid WHERE folder_search MATCH ? ORDER BY rank LIMIT 30").map_err(|e|e.to_string())?;
        let folders = stmt
            .query_map([&query], folder_row)
            .map_err(|e| e.to_string())?
            .collect::<std::result::Result<Vec<_>, _>>()
            .map_err(|e| e.to_string())?;
        let folders = self.decorate_folders(folders)?;
        drop(stmt);
        tx.commit().map_err(|e| e.to_string())?;
        Ok((folders, tracks))
    }
    pub fn folder_queue(&self, folder: i64, start: i64) -> Result<(Vec<Track>, usize)> {
        let tracks = self.tracks(folder, 0, u32::MAX)?;
        let index = tracks
            .iter()
            .position(|t| t.id == start)
            .ok_or("This track is no longer in the folder. Rescan your library.")?;
        Ok((tracks, index))
    }
}
fn folder_row(r: &rusqlite::Row<'_>) -> rusqlite::Result<Folder> {
    Ok(Folder {
        id: r.get(0)?,
        parent_id: r.get(1)?,
        name: r.get(2)?,
        path: r.get(3)?,
        has_children: r.get(4)?,
        kind: FolderKind::Auto,
        override_kind: FolderKind::Auto,
        artwork: String::new(),
        track_count: 0,
    })
}
fn track_row(r: &rusqlite::Row<'_>) -> rusqlite::Result<Track> {
    Ok(Track {
        id: r.get(0)?,
        folder_id: r.get(1)?,
        path: r.get(2)?,
        title: r.get(3)?,
        artist: r.get(4)?,
        album: r.get(5)?,
        album_artist: r.get(6)?,
        artwork: r.get(7)?,
        codec: r.get(8)?,
        duration: r.get(9)?,
        number: r.get(10)?,
        year: r.get(11)?,
        favorite: r.get(12)?,
        reference_id: r.get(13)?,
        event_time: r.get(14)?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};
    static NEXT_SANDBOX: AtomicU64 = AtomicU64::new(0);
    pub(super) fn sandbox() -> PathBuf {
        let p = std::env::temp_dir().join(format!(
            "spiewak-test-{}-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos(),
            NEXT_SANDBOX.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&p).unwrap();
        p
    }
    pub(super) fn wav(path: &Path) {
        let samples = vec![0u8; 8820];
        let mut bytes = Vec::new();
        bytes.extend(b"RIFF");
        bytes.extend((36 + samples.len() as u32).to_le_bytes());
        bytes.extend(b"WAVEfmt ");
        bytes.extend(16u32.to_le_bytes());
        bytes.extend(1u16.to_le_bytes());
        bytes.extend(1u16.to_le_bytes());
        bytes.extend(44100u32.to_le_bytes());
        bytes.extend(88200u32.to_le_bytes());
        bytes.extend(2u16.to_le_bytes());
        bytes.extend(16u16.to_le_bytes());
        bytes.extend(b"data");
        bytes.extend((samples.len() as u32).to_le_bytes());
        bytes.extend(samples);
        fs::write(path, bytes).unwrap();
    }
    #[test]
    fn index_preserves_hierarchy_and_rebuilds_after_external_changes() {
        let p = sandbox();
        let root = p.join("Music");
        let album = root.join("Artist").join("Album");
        fs::create_dir_all(&album).unwrap();
        wav(&album.join("01 - Song.wav"));
        fs::write(album.join("._01 - Song.wav"), b"AppleDouble").unwrap();
        let lib = Library::open(&p.join("index.db")).unwrap();
        assert_eq!(lib.add_root(&root).unwrap().tracks, 1);
        let top = lib.folders(0).unwrap();
        assert_eq!(top[0].name, "Music");
        let artist = lib.folders(top[0].id).unwrap();
        assert_eq!(artist[0].name, "Artist");
        let folders = lib.folders(artist[0].id).unwrap();
        let original = lib.tracks(folders[0].id, 0, 500).unwrap();
        assert_eq!(original[0].title, "01 - Song");
        assert!(original[0].duration > 0.0);
        let id = original[0].id;
        lib.reconcile().unwrap();
        assert_eq!(lib.tracks(folders[0].id, 0, 500).unwrap()[0].id, id);
        fs::rename(album.join("01 - Song.wav"), album.join("02 - Renamed.wav")).unwrap();
        wav(&album.join("03 - Added.wav"));
        lib.reconcile().unwrap();
        let tracks = lib.tracks(folders[0].id, 0, 500).unwrap();
        assert_eq!(tracks.len(), 2);
        assert_eq!(tracks[0].title, "02 - Renamed");
        fs::remove_file(album.join("03 - Added.wav")).unwrap();
        drop(lib);
        let lib = Library::open(&p.join("index.db")).unwrap();
        lib.reconcile().unwrap();
        assert_eq!(lib.tracks(folders[0].id, 0, 500).unwrap().len(), 1);
        drop(lib);
        fs::remove_dir_all(p).unwrap();
    }
    #[cfg(windows)]
    #[test]
    fn file_identity_preserves_case_renames_and_distinguishes_hard_links() {
        let p = sandbox();
        let root = p.join("Music");
        fs::create_dir(&root).unwrap();
        let original = root.join("Song.wav");
        wav(&original);
        let linked = root.join("Linked.wav");
        fs::hard_link(&original, &linked).unwrap();
        let lib = Library::open(&p.join("cache.db")).unwrap();
        lib.add_root(&root).unwrap();
        let folder = lib.folders(0).unwrap()[0].id;
        let rows = lib.tracks(folder, 0, 500).unwrap();
        let song = rows
            .iter()
            .find(|t| t.path.ends_with("Song.wav"))
            .unwrap()
            .id;
        let link = rows
            .iter()
            .find(|t| t.path.ends_with("Linked.wav"))
            .unwrap()
            .id;
        assert_ne!(song, link);
        let renamed = root.join("song.wav");
        fs::rename(&original, &renamed).unwrap();
        lib.reconcile().unwrap();
        assert_eq!(
            lib.selected_tracks(&[song]).unwrap()[0].path,
            display_path(&renamed)
        );
        let other = root.join("Other");
        fs::create_dir(&other).unwrap();
        let moved = other.join("Moved.wav");
        fs::rename(&renamed, &moved).unwrap();
        lib.reconcile().unwrap();
        assert_eq!(
            lib.selected_tracks(&[song]).unwrap()[0].path,
            display_path(&moved)
        );
        assert_eq!(
            lib.selected_tracks(&[link]).unwrap()[0].path,
            display_path(&linked)
        );
        assert!(!lib.reconcile().unwrap().changed);
        drop(lib);
        fs::remove_dir_all(p).unwrap();
    }
    #[test]
    fn folder_presentation_changes_only_cache_and_survives_folder_rename() {
        let p = sandbox();
        let root = p.join("Music");
        let folder = root.join("Random Songs");
        fs::create_dir_all(&folder).unwrap();
        let path = folder.join("one.wav");
        wav(&path);
        let original = fs::read(&path).unwrap();
        let db = p.join("cache.db");
        let lib = Library::open(&db).unwrap();
        lib.add_root(&root).unwrap();
        let root_id = lib.folders(0).unwrap()[0].id;
        let f = lib.folders(root_id).unwrap().remove(0);
        assert_eq!(f.kind, FolderKind::LooseTracks);
        assert_eq!(f.track_count, 1);
        lib.set_folder_kind(f.id, FolderKind::UserCollection as u32)
            .unwrap();
        lib.reconcile().unwrap();
        assert_eq!(
            lib.folder(f.id).unwrap().unwrap().kind,
            FolderKind::UserCollection
        );
        assert_eq!(fs::read(&path).unwrap(), original);
        drop(lib);
        let lib = Library::open(&db).unwrap();
        assert_eq!(
            lib.folder(f.id).unwrap().unwrap().override_kind,
            FolderKind::UserCollection
        );
        assert!(lib.set_folder_kind(f.id, 7).is_err());
        assert!(lib.set_folder_kind(i64::MAX, 1).is_err());
        #[cfg(windows)]
        {
            fs::rename(&folder, root.join("Renamed")).unwrap();
            lib.reconcile().unwrap();
            assert_eq!(
                lib.folder(f.id).unwrap().unwrap().kind,
                FolderKind::UserCollection
            );
        }
        lib.set_folder_kind(f.id, 0).unwrap();
        assert_eq!(
            lib.folder(f.id).unwrap().unwrap().kind,
            FolderKind::LooseTracks
        );
        drop(lib);
        fs::remove_dir_all(p).unwrap();
    }
    #[test]
    fn unavailable_root_keeps_cached_library_and_overlap_is_rejected() {
        let p = sandbox();
        let root = p.join("Music");
        fs::create_dir_all(root.join("Loose")).unwrap();
        wav(&root.join("one.wav"));
        let lib = Library::open(&p.join("index.db")).unwrap();
        lib.add_root(&root).unwrap();
        assert!(lib.add_root(&root.join("Loose")).is_err());
        let folder = lib.folders(0).unwrap()[0].id;
        fs::rename(&root, p.join("Offline")).unwrap();
        assert!(!lib.reconcile().unwrap().warnings.is_empty());
        assert_eq!(lib.tracks(folder, 0, 500).unwrap().len(), 1);
        drop(lib);
        fs::remove_dir_all(p).unwrap();
    }
    #[test]
    fn search_unicode_prefixes_and_external_changes_are_indexed() {
        let p = sandbox();
        let root = p.join("Śpiewak");
        fs::create_dir(&root).unwrap();
        wav(&root.join("Zebra.wav"));
        wav(&root.join("Alpha.wav"));
        let lib = Library::open(&p.join("index.db")).unwrap();
        lib.add_root(&root).unwrap();
        let folder = lib.folders(0).unwrap()[0].id;
        assert_eq!(lib.search("ŚPIE").unwrap().1.len(), 2);
        assert_eq!(lib.search("alp").unwrap().1[0].title, "Alpha");
        assert!(lib.search(r#"" OR *"#).unwrap().1.is_empty());
        assert_eq!(
            lib.tracks_sorted(folder, 0, 500, 1).unwrap()[0].title,
            "Alpha"
        );
        fs::remove_file(root.join("Alpha.wav")).unwrap();
        lib.reconcile().unwrap();
        assert!(lib.search("alp").unwrap().1.is_empty());
        drop(lib);
        let lib = Library::open(&p.join("index.db")).unwrap();
        assert_eq!(lib.search("zeb").unwrap().1.len(), 1);
        drop(lib);
        fs::remove_dir_all(p).unwrap();
    }
    #[test]
    fn playlist_persists_order_duplicates_and_missing_files() {
        let p = sandbox();
        let root = p.join("Music");
        fs::create_dir(&root).unwrap();
        wav(&root.join("one.wav"));
        wav(&root.join("two.wav"));
        let lib = Library::open(&p.join("index.db")).unwrap();
        lib.add_root(&root).unwrap();
        let folder = lib.folders(0).unwrap()[0].id;
        let tracks = lib.tracks(folder, 0, 500).unwrap();
        let id = lib.playlist_command(0, 0, "Evening", &[], 0).unwrap();
        lib.playlist_command(id, 3, "", &[tracks[0].id, tracks[1].id, tracks[0].id], 0)
            .unwrap();
        lib.playlist_command(id, 4, "", &[], 2).unwrap();
        lib.playlist_command(id, 6, "", &[], 0).unwrap();
        assert_eq!(lib.playlist_tracks(id, 0, 500).unwrap()[0].id, tracks[1].id);
        assert!(
            lib.playlist_command(id, 3, "", &[tracks[0].id, -999], 0)
                .is_err()
        );
        assert_eq!(lib.playlists().unwrap()[0].count, 2);
        lib.playlist_command(id, 2, "", &[], 0).unwrap();
        assert!(lib.playlists().unwrap().is_empty());
        lib.playlist_command(id, 7, "", &[], 0).unwrap();
        fs::remove_file(&tracks[1].path).unwrap();
        lib.reconcile().unwrap();
        drop(lib);
        let lib = Library::open(&p.join("index.db")).unwrap();
        let rows = lib.playlist_tracks(id, 0, 500).unwrap();
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].id, 0);
        assert!(rows[0].title.starts_with("Unavailable"));
        drop(lib);
        fs::remove_dir_all(p).unwrap();
    }
}
