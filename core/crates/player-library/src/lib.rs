//! Rebuildable SQLite index. Scanning never writes to the music collection.
use lofty::{
    file::{AudioFile, TaggedFileExt},
    tag::{Accessor, ItemKey},
};
use player_domain::{Folder, Track};
mod playlists;
use rusqlite::{Connection, OptionalExtension, params};
use std::{
    fs,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

type Result<T> = std::result::Result<T, String>;
pub struct Library {
    db: Connection,
}
#[derive(Default, Debug)]
pub struct ScanReport {
    pub tracks: u64,
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
fn artwork(folder: &Path) -> String {
    for file in [
        "cover.jpg",
        "folder.jpg",
        "front.jpg",
        "cover.png",
        "folder.png",
    ] {
        let path = folder.join(file);
        if path.is_file() {
            return display_path(&path);
        }
    }
    String::new()
}
fn read_track(path: &Path, folder_id: i64) -> Track {
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
        artwork: artwork(path.parent().unwrap_or(path)),
        ..Default::default()
    };
    if let Ok(file) = lofty::read_from_path(path) {
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
        if schema_version > 3 {
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
        Ok(Self { db })
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
                self.db.execute("INSERT INTO folders(root_id,parent_id,name,path,epoch) VALUES(?,?,?,?,?) ON CONFLICT(path) DO UPDATE SET name=excluded.name,parent_id=excluded.parent_id,epoch=excluded.epoch",params![root_id,parent,name,folder_path,epoch]).map_err(|e|e.to_string())?;
                let folder_id: i64 = self
                    .db
                    .query_row("SELECT id FROM folders WHERE path=?", [&folder_path], |r| {
                        r.get(0)
                    })
                    .map_err(|e| e.to_string())?;
                let entries = match fs::read_dir(&folder) {
                    Ok(e) => e,
                    Err(e) => {
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
                            report.warnings.push(e.to_string());
                            continue;
                        }
                    };
                    let path = entry.path();
                    let meta = match fs::symlink_metadata(&path) {
                        Ok(m) => m,
                        Err(e) => {
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
                        pending.push((path, Some(folder_id)));
                        continue;
                    }
                    if !meta.is_file() || !is_audio(&path) {
                        continue;
                    }
                    report.tracks += 1;
                    let stored_path = display_path(&path);
                    let modified = stamp(&meta);
                    let old: Option<(i64, i64)> = self
                        .db
                        .query_row(
                            "SELECT size,modified FROM tracks WHERE path=?",
                            [&stored_path],
                            |r| Ok((r.get(0)?, r.get(1)?)),
                        )
                        .optional()
                        .map_err(|e| e.to_string())?;
                    if old == Some((meta.len() as i64, modified)) {
                        self.db
                            .execute(
                                "UPDATE tracks SET epoch=?,artwork=? WHERE path=?",
                                params![epoch, artwork(&folder), stored_path],
                            )
                            .map_err(|e| e.to_string())?;
                        continue;
                    }
                    let t = read_track(&path, folder_id);
                    self.db.execute("INSERT INTO tracks(folder_id,path,title,artist,album,album_artist,artwork,codec,duration,number,year,size,modified,epoch) VALUES(?,?,?,?,?,?,?,?,?,?,?,?,?,?) ON CONFLICT(path) DO UPDATE SET folder_id=excluded.folder_id,title=excluded.title,artist=excluded.artist,album=excluded.album,album_artist=excluded.album_artist,artwork=excluded.artwork,codec=excluded.codec,duration=excluded.duration,number=excluded.number,year=excluded.year,size=excluded.size,modified=excluded.modified,epoch=excluded.epoch",params![t.folder_id,t.path,t.title,t.artist,t.album,t.album_artist,t.artwork,t.codec,t.duration,t.number,t.year,meta.len() as i64,modified,epoch]).map_err(|e|e.to_string())?;
                }
            }
            // A partially unreadable root must not make cached tracks disappear.
            if report.warnings.is_empty() {
                self.db.execute("DELETE FROM tracks WHERE epoch!=? AND folder_id IN (SELECT id FROM folders WHERE root_id=?)",params![epoch,root_id]).map_err(|e|e.to_string())?;
                self.db
                    .execute(
                        "DELETE FROM folders WHERE root_id=? AND epoch!=?",
                        params![root_id, epoch],
                    )
                    .map_err(|e| e.to_string())?;
            }
            Ok(())
        })();
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
    pub fn selected_tracks(&self, ids: &[i64]) -> Result<Vec<Track>> {
        if ids.is_empty() || ids.len() > 100_000 {
            return Err("Select tracks first (up to 100,000).".into());
        }
        let mut stmt = self.db.prepare("SELECT id,folder_id,path,title,artist,album,album_artist,artwork,codec,duration,number,year FROM tracks WHERE id=?").map_err(|e|e.to_string())?;
        ids.iter()
            .map(|id| {
                stmt.query_row([id], track_row).map_err(|_| {
                    format!("Track {id} is no longer in the library. Rescan its folder.")
                })
            })
            .collect()
    }
    pub fn folders(&self, parent: i64) -> Result<Vec<Folder>> {
        let mut stmt=self.db.prepare("SELECT f.id,coalesce(f.parent_id,0),f.name,f.path,EXISTS(SELECT 1 FROM folders c WHERE c.parent_id=f.id) FROM folders f WHERE coalesce(f.parent_id,0)=? ORDER BY f.name COLLATE NOCASE").map_err(|e|e.to_string())?;
        stmt.query_map([parent], |r| {
            Ok(Folder {
                id: r.get(0)?,
                parent_id: r.get(1)?,
                name: r.get(2)?,
                path: r.get(3)?,
                has_children: r.get(4)?,
            })
        })
        .map_err(|e| e.to_string())?
        .collect::<std::result::Result<_, _>>()
        .map_err(|e| e.to_string())
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
            "SELECT id,folder_id,path,title,artist,album,album_artist,artwork,codec,duration,number,year FROM tracks WHERE folder_id=? ORDER BY {order} LIMIT ? OFFSET ?"
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
        let mut stmt=self.db.prepare("SELECT t.id,t.folder_id,t.path,t.title,t.artist,t.album,t.album_artist,t.artwork,t.codec,t.duration,t.number,t.year FROM track_search JOIN tracks t ON t.id=track_search.rowid WHERE track_search MATCH ? ORDER BY rank LIMIT 200").map_err(|e|e.to_string())?;
        let tracks = stmt
            .query_map([&query], track_row)
            .map_err(|e| e.to_string())?
            .collect::<std::result::Result<Vec<_>, _>>()
            .map_err(|e| e.to_string())?;
        let mut stmt=self.db.prepare("SELECT f.id,coalesce(f.parent_id,0),f.name,f.path,EXISTS(SELECT 1 FROM folders c WHERE c.parent_id=f.id) FROM folder_search JOIN folders f ON f.id=folder_search.rowid WHERE folder_search MATCH ? ORDER BY rank LIMIT 30").map_err(|e|e.to_string())?;
        let folders = stmt
            .query_map([&query], |row| {
                Ok(Folder {
                    id: row.get(0)?,
                    parent_id: row.get(1)?,
                    name: row.get(2)?,
                    path: row.get(3)?,
                    has_children: row.get(4)?,
                })
            })
            .map_err(|e| e.to_string())?
            .collect::<std::result::Result<Vec<_>, _>>()
            .map_err(|e| e.to_string())?;
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
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};
    static NEXT_SANDBOX: AtomicU64 = AtomicU64::new(0);
    fn sandbox() -> PathBuf {
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
    fn wav(path: &Path) {
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
