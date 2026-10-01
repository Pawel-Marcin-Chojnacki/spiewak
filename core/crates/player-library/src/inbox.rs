//! Opt-in, read-only intake. Only explicit metadata Apply may change music.
use super::*;

const STABLE_SECONDS: i64 = 10;

pub(super) fn migrate(db: &Connection) -> Result<()> {
    db.execute_batch("SAVEPOINT migration;
      CREATE TABLE IF NOT EXISTS inbox_folders(folder_id INTEGER PRIMARY KEY REFERENCES folders(id) ON DELETE CASCADE,baseline INTEGER NOT NULL);
      CREATE TABLE IF NOT EXISTS inbox_items(track_id INTEGER PRIMARY KEY REFERENCES tracks(id) ON DELETE CASCADE,folder_id INTEGER NOT NULL REFERENCES inbox_folders(folder_id) ON DELETE CASCADE,size INTEGER NOT NULL,modified INTEGER NOT NULL,identity TEXT NOT NULL,stable_since INTEGER NOT NULL,checked_at INTEGER NOT NULL,state INTEGER NOT NULL CHECK(state BETWEEN 0 AND 3),message TEXT NOT NULL);
      CREATE INDEX IF NOT EXISTS inbox_state ON inbox_items(state,stable_since,track_id);
      PRAGMA user_version=14; RELEASE migration;").map_err(|e|e.to_string())
}

impl Library {
    pub fn set_inbox(&self, folder: i64, enabled: bool) -> Result<()> {
        if !enabled {
            self.db
                .execute("DELETE FROM inbox_folders WHERE folder_id=?", [folder])
                .map_err(|e| e.to_string())?;
            return Ok(());
        }
        // Scan before the baseline: existing but not yet cached files are never new intake.
        let root: String = self
            .db
            .query_row(
                "SELECT r.path FROM folders f JOIN library_roots r ON r.id=f.root_id WHERE f.id=?",
                [folder],
                |r| r.get(0),
            )
            .map_err(|_| "Choose an existing library folder.".to_owned())?;
        let report = self.scan_root(&root)?;
        if !report.warnings.is_empty() {
            return Err(format!(
                "Cannot configure Inbox until this folder scans completely: {}",
                report.warnings.join("; ")
            ));
        }
        let current = self
            .folder(folder)?
            .ok_or("The folder is no longer available.")?;
        if !Path::new(&current.path).is_dir() {
            return Err("Reconnect the Inbox folder before enabling it.".into());
        }
        self.db.execute("INSERT INTO inbox_folders(folder_id,baseline) SELECT ?,coalesce(max(value),0) FROM entity_ids WHERE kind='tracks' ON CONFLICT(folder_id) DO NOTHING", [folder]).map_err(|e|e.to_string())?;
        Ok(())
    }

    pub fn inbox_status(&self) -> Result<(Vec<Folder>, String)> {
        let tx = self.db.unchecked_transaction().map_err(|e| e.to_string())?;
        let ids = self
            .db
            .prepare("SELECT folder_id FROM inbox_folders ORDER BY folder_id")
            .map_err(|e| e.to_string())?
            .query_map([], |r| r.get::<_, i64>(0))
            .map_err(|e| e.to_string())?
            .collect::<std::result::Result<Vec<_>, _>>()
            .map_err(|e| e.to_string())?;
        let mut folders = Vec::new();
        for id in ids {
            if let Some(folder) = self.folder_current(id)? {
                folders.push(folder);
            }
        }
        let (waiting, ready, blocked): (i64,i64,i64) = self.db.query_row("SELECT coalesce(sum(state=0),0),coalesce(sum(state=1),0),coalesce(sum(state=3),0) FROM inbox_items", [], |r|Ok((r.get(0)?,r.get(1)?,r.get(2)?))).map_err(|e|e.to_string())?;
        let mut message = format!(
            "{} Inbox folders · {ready} need review · {waiting} waiting for stable files · {blocked} unavailable or unreadable",
            folders.len()
        );
        let details = self.db.prepare("SELECT t.path,i.message FROM inbox_items i JOIN tracks t ON t.id=i.track_id WHERE i.state=3 ORDER BY i.track_id LIMIT 5").map_err(|e|e.to_string())?.query_map([], |r|Ok(format!("{}: {}",r.get::<_,String>(0)?,r.get::<_,String>(1)?))).map_err(|e|e.to_string())?.collect::<std::result::Result<Vec<_>,_>>().map_err(|e|e.to_string())?;
        for detail in details {
            message.push('\n');
            message.push_str(&detail);
        }
        tx.commit().map_err(|e| e.to_string())?;
        Ok((folders, message))
    }

    pub(super) fn inbox_tracks(&self, offset: u32, limit: u32) -> Result<Vec<Track>> {
        let mut stmt = self.db.prepare("SELECT t.id,t.folder_id,t.path,t.title,t.artist,t.album,t.album_artist,t.artwork,t.codec,t.duration,t.number,t.year,EXISTS(SELECT 1 FROM favorites f WHERE f.original_id=t.id),t.id,i.stable_since FROM inbox_items i JOIN tracks t ON t.id=i.track_id WHERE i.state=1 AND t.folder_id=i.folder_id ORDER BY i.stable_since DESC,t.id DESC LIMIT ? OFFSET ?").map_err(|e|e.to_string())?;
        stmt.query_map(params![limit, offset], track_row)
            .map_err(|e| e.to_string())?
            .collect::<std::result::Result<_, _>>()
            .map_err(|e| e.to_string())
    }

    /// Acknowledge only the version seen by the UI (stable_since is the view token).
    /// A later changed file re-enters intake. This never deletes or edits a music file.
    pub fn complete_inbox(&self, track: i64, observed: i64) -> Result<()> {
        let tx = self.db.unchecked_transaction().map_err(|e| e.to_string())?;
        let (path,size,modified,identity): (String,i64,i64,String) = tx.query_row("SELECT t.path,i.size,i.modified,i.identity FROM inbox_items i JOIN tracks t ON t.id=i.track_id WHERE i.track_id=? AND i.state=1 AND i.stable_since=? AND t.folder_id=i.folder_id",params![track,observed],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?))).map_err(|_|"This Inbox entry changed. Refresh it before marking it reviewed.".to_owned())?;
        let current =
            fs::metadata(&path).map_err(|e| format!("Cannot read this Inbox file: {e}"))?;
        if current.len() as i64 != size
            || stamp(&current) != modified
            || identity::file_identity(Path::new(&path)).unwrap_or_default() != identity
        {
            return Err("This Inbox file changed. Wait for it to stabilize again.".into());
        }
        tx.execute(
            "UPDATE inbox_items SET state=2,message='' WHERE track_id=?",
            [track],
        )
        .map_err(|e| e.to_string())?;
        tx.commit().map_err(|e| e.to_string())
    }

    pub fn refresh_inbox(&self) -> Result<bool> {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64;
        self.refresh_inbox_at(now)
    }

    fn refresh_inbox_at(&self, now: i64) -> Result<bool> {
        let tx = self.db.unchecked_transaction().map_err(|e| e.to_string())?;
        let mut changed = tx.execute("DELETE FROM inbox_items WHERE NOT EXISTS(SELECT 1 FROM tracks t WHERE t.id=inbox_items.track_id AND t.folder_id=inbox_items.folder_id)", []).map_err(|e|e.to_string())? > 0;
        changed |= tx.execute("INSERT INTO inbox_items(track_id,folder_id,size,modified,identity,stable_since,checked_at,state,message) SELECT t.id,t.folder_id,t.size,t.modified,coalesce(t.file_identity,''),?,0,0,'' FROM tracks t JOIN inbox_folders f ON f.folder_id=t.folder_id WHERE t.id>f.baseline ON CONFLICT(track_id) DO NOTHING", [now]).map_err(|e|e.to_string())? > 0;
        let rows = tx.prepare("SELECT i.track_id,t.path,i.size,i.modified,i.identity,i.stable_since,i.checked_at,i.state FROM inbox_items i JOIN tracks t ON t.id=i.track_id").map_err(|e|e.to_string())?.query_map([], |r|Ok((r.get::<_,i64>(0)?,r.get::<_,String>(1)?,r.get::<_,i64>(2)?,r.get::<_,i64>(3)?,r.get::<_,String>(4)?,r.get::<_,i64>(5)?,r.get::<_,i64>(6)?,r.get::<_,u32>(7)?))).map_err(|e|e.to_string())?.collect::<std::result::Result<Vec<_>,_>>().map_err(|e|e.to_string())?;
        for (id, path, size, modified, identity, since, checked, state) in rows {
            let path = Path::new(&path);
            let meta = match fs::metadata(path) {
                Ok(meta) if meta.is_file() => meta,
                _ => {
                    if state != 3 && state != 2 {
                        tx.execute("UPDATE inbox_items SET state=3,message='Reconnect the folder or finish downloading this file.',checked_at=? WHERE track_id=?",params![now,id]).map_err(|e|e.to_string())?;
                        changed = true;
                    }
                    continue;
                }
            };
            let current_identity = identity::file_identity(path).unwrap_or_default();
            if meta.len() as i64 != size
                || stamp(&meta) != modified
                || current_identity != identity
                || now < since
            {
                tx.execute("UPDATE inbox_items SET size=?,modified=?,identity=?,stable_since=?,checked_at=0,state=0,message='' WHERE track_id=?",params![meta.len() as i64,stamp(&meta),current_identity,now,id]).map_err(|e|e.to_string())?;
                changed = true;
                continue;
            }
            if state == 1
                || state == 2
                || now - since < STABLE_SECONDS
                || state == 3 && now - checked < 30
            {
                continue;
            }
            // Header/tag readability is not a guarantee that every audio frame is decodable.
            let valid = validate(path);
            let (next, message) = match valid {
                Ok(()) => (1, "".to_owned()),
                Err(e) => (3, e),
            };
            tx.execute(
                "UPDATE inbox_items SET state=?,message=?,checked_at=? WHERE track_id=?",
                params![next, message, now, id],
            )
            .map_err(|e| e.to_string())?;
            changed |= state != next;
        }
        tx.commit().map_err(|e| e.to_string())?;
        Ok(changed)
    }
}

fn validate(path: &Path) -> Result<()> {
    let before = fs::metadata(path).map_err(|e| e.to_string())?;
    let mut options = fs::OpenOptions::new();
    options.read(true);
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        options.share_mode(1); // Readers allowed, active writers/renames are refused.
    }
    let file = options
        .open(path)
        .map_err(|e| format!("Wait for the downloader to close this file: {e}"))?;
    lofty::config::apply_global_options(
        lofty::config::GlobalOptions::new().allocation_limit(8 * 1024 * 1024),
    );
    let parsed = lofty::probe::Probe::new(std::io::BufReader::new(file))
        .guess_file_type()
        .map_err(|e| e.to_string())?
        .options(lofty::config::ParseOptions::new().read_cover_art(false))
        .read()
        .map_err(|e| format!("Cannot read audio metadata; repair or replace this file: {e}"))?;
    if parsed.properties().duration().is_zero() {
        return Err("No readable audio duration. Finish downloading or repair this file.".into());
    }
    let after = fs::metadata(path).map_err(|e| e.to_string())?;
    if before.len() != after.len() || stamp(&before) != stamp(&after) {
        return Err("File is still changing. Waiting before review.".into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn setup() -> (PathBuf, Library, i64) {
        let dir = crate::tests::sandbox();
        let root = dir.join("Inbox Żółć");
        fs::create_dir(&root).unwrap();
        crate::tests::wav(&root.join("Existing.wav"));
        let lib = Library::open(&dir.join("index.db")).unwrap();
        lib.add_root(&root).unwrap();
        let folder = lib.folders(0).unwrap()[0].id;
        lib.set_inbox(folder, true).unwrap();
        (dir, lib, folder)
    }
    #[test]
    fn only_new_direct_files_enter_review_and_acknowledgement_survives_restart() {
        let (dir, lib, folder) = setup();
        let root = dir.join("Inbox Żółć");
        let new = root.join("New.wav");
        crate::tests::wav(&new);
        let bytes = fs::read(&new).unwrap();
        fs::create_dir(root.join("Album")).unwrap();
        crate::tests::wav(&root.join("Album/Keep.wav"));
        lib.reconcile().unwrap();
        lib.refresh_inbox_at(100).unwrap();
        assert!(lib.listening_tracks(3, 0, 500).unwrap().is_empty());
        lib.refresh_inbox_at(109).unwrap();
        assert!(lib.listening_tracks(3, 0, 500).unwrap().is_empty());
        lib.refresh_inbox_at(110).unwrap();
        let rows = lib.listening_tracks(3, 0, 500).unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].folder_id, folder);
        assert!(rows[0].path.ends_with("New.wav"));
        assert!(lib.complete_inbox(rows[0].id, 99).is_err());
        lib.complete_inbox(rows[0].id, rows[0].event_time).unwrap();
        assert!(lib.listening_tracks(3, 0, 500).unwrap().is_empty());
        let offline = dir.join("Offline.wav");
        fs::rename(&new, &offline).unwrap();
        lib.refresh_inbox_at(150).unwrap();
        fs::rename(&offline, &new).unwrap();
        drop(lib);
        let lib = Library::open(&dir.join("index.db")).unwrap();
        lib.refresh_inbox_at(200).unwrap();
        assert_eq!(lib.inbox_status().unwrap().0.len(), 1);
        assert!(lib.listening_tracks(3, 0, 500).unwrap().is_empty());
        assert_eq!(fs::read(new).unwrap(), bytes);
        drop(lib);
        fs::remove_dir_all(dir).unwrap();
    }
    #[test]
    fn changes_reset_stability_and_stale_ui_cannot_acknowledge_changed_bytes() {
        let (dir, lib, _) = setup();
        let path = dir.join("Inbox Żółć/New.wav");
        crate::tests::wav(&path);
        lib.reconcile().unwrap();
        lib.refresh_inbox_at(100).unwrap();
        let mut bytes = fs::read(&path).unwrap();
        bytes.extend_from_slice(&[0; 16]);
        fs::write(&path, &bytes).unwrap();
        lib.refresh_inbox_at(109).unwrap();
        lib.refresh_inbox_at(110).unwrap();
        assert!(lib.listening_tracks(3, 0, 500).unwrap().is_empty());
        lib.refresh_inbox_at(119).unwrap();
        let row = lib.listening_tracks(3, 0, 500).unwrap().remove(0);
        bytes.extend_from_slice(&[0; 16]);
        fs::write(&path, &bytes).unwrap();
        assert!(lib.complete_inbox(row.id, row.event_time).is_err());
        lib.refresh_inbox_at(120).unwrap();
        lib.refresh_inbox_at(130).unwrap();
        assert!(lib.complete_inbox(row.id, row.event_time).is_err());
        assert_eq!(fs::read(path).unwrap(), bytes);
        drop(lib);
        fs::remove_dir_all(dir).unwrap();
    }
    #[test]
    fn invalid_missing_and_moved_out_files_are_not_ready_and_disable_preserves_music() {
        let (dir, lib, folder) = setup();
        let root = dir.join("Inbox Żółć");
        let bad = root.join("Partial.mp3");
        fs::write(&bad, b"unfinished download").unwrap();
        lib.reconcile().unwrap();
        lib.refresh_inbox_at(100).unwrap();
        lib.refresh_inbox_at(110).unwrap();
        assert!(
            lib.inbox_status()
                .unwrap()
                .1
                .contains("1 unavailable or unreadable")
        );
        assert!(lib.listening_tracks(3, 0, 500).unwrap().is_empty());
        let path = root.join("New.wav");
        crate::tests::wav(&path);
        lib.reconcile().unwrap();
        lib.refresh_inbox_at(120).unwrap();
        lib.refresh_inbox_at(130).unwrap();
        assert_eq!(lib.listening_tracks(3, 0, 500).unwrap().len(), 1);
        fs::create_dir(root.join("Kept")).unwrap();
        fs::rename(&path, root.join("Kept/New.wav")).unwrap();
        lib.refresh_inbox_at(140).unwrap();
        assert!(lib.listening_tracks(3, 0, 500).unwrap().is_empty());
        lib.reconcile().unwrap();
        lib.refresh_inbox_at(150).unwrap();
        lib.set_inbox(folder, false).unwrap();
        assert!(lib.inbox_status().unwrap().0.is_empty());
        assert!(root.join("Kept/New.wav").exists());
        assert_eq!(fs::read(bad).unwrap(), b"unfinished download");
        lib.set_inbox(folder, true).unwrap();
        lib.refresh_inbox_at(200).unwrap();
        lib.refresh_inbox_at(210).unwrap();
        assert!(lib.listening_tracks(3, 0, 500).unwrap().is_empty());
        drop(lib);
        fs::remove_dir_all(dir).unwrap();
    }
    #[test]
    fn enabling_scans_before_baseline_and_repeated_enable_does_not_swallow_new_files() {
        let (dir, lib, folder) = setup();
        let root = dir.join("Inbox Żółć");
        lib.set_inbox(folder, false).unwrap();
        crate::tests::wav(&root.join("Uncached.wav"));
        lib.set_inbox(folder, true).unwrap();
        lib.refresh_inbox_at(100).unwrap();
        lib.refresh_inbox_at(110).unwrap();
        assert!(lib.listening_tracks(3, 0, 500).unwrap().is_empty());
        crate::tests::wav(&root.join("New.wav"));
        lib.set_inbox(folder, true).unwrap();
        lib.refresh_inbox_at(120).unwrap();
        lib.refresh_inbox_at(130).unwrap();
        assert_eq!(lib.listening_tracks(3, 0, 500).unwrap().len(), 1);
        drop(lib);
        fs::remove_dir_all(dir).unwrap();
    }
    #[test]
    fn schema_thirteen_upgrade_keeps_playlists_and_starts_with_no_inbox() {
        let dir = crate::tests::sandbox();
        let db = dir.join("index.db");
        let lib = Library::open(&db).unwrap();
        let playlist = lib.playlist_command(0, 0, "Keep", &[], 0).unwrap();
        lib.db
            .execute_batch(
                "DROP TABLE inbox_items; DROP TABLE inbox_folders; PRAGMA user_version=13;",
            )
            .unwrap();
        drop(lib);
        let lib = Library::open(&db).unwrap();
        assert_eq!(lib.playlists().unwrap()[0].id, playlist);
        assert!(lib.inbox_status().unwrap().0.is_empty());
        assert_eq!(
            lib.db
                .query_row("PRAGMA user_version", [], |r| r.get::<_, u32>(0))
                .unwrap(),
            14
        );
        drop(lib);
        fs::remove_dir_all(dir).unwrap();
    }
    #[cfg(windows)]
    #[test]
    fn active_windows_writer_is_not_ready_even_when_size_and_time_are_stable() {
        let (dir, lib, _) = setup();
        let path = dir.join("Inbox Żółć/New.wav");
        crate::tests::wav(&path);
        lib.reconcile().unwrap();
        lib.refresh_inbox_at(100).unwrap();
        let writer = fs::OpenOptions::new().write(true).open(&path).unwrap();
        lib.refresh_inbox_at(110).unwrap();
        assert!(lib.listening_tracks(3, 0, 500).unwrap().is_empty());
        drop(writer);
        lib.refresh_inbox_at(140).unwrap();
        assert_eq!(lib.listening_tracks(3, 0, 500).unwrap().len(), 1);
        drop(lib);
        fs::remove_dir_all(dir).unwrap();
    }
}
