//! User listening state stays in the application database, never in music tags.
use super::*;
use player_domain::Listen;
pub(super) fn migrate(db: &Connection) -> Result<()> {
    db.execute_batch("SAVEPOINT migration;
      ALTER TABLE tracks ADD COLUMN added_at INTEGER NOT NULL DEFAULT 0;
      UPDATE tracks SET added_at=unixepoch();
      CREATE INDEX tracks_added ON tracks(added_at DESC,id DESC);
      CREATE TABLE favorites(original_id INTEGER PRIMARY KEY,track_id INTEGER UNIQUE REFERENCES tracks(id) ON DELETE SET NULL,path TEXT NOT NULL,title TEXT NOT NULL,artist TEXT NOT NULL,album TEXT NOT NULL,album_artist TEXT NOT NULL,artwork TEXT NOT NULL,codec TEXT NOT NULL,duration REAL NOT NULL,number INTEGER NOT NULL,year INTEGER NOT NULL,favorited_at INTEGER NOT NULL);
      CREATE TABLE listening_history(id INTEGER PRIMARY KEY,original_id INTEGER NOT NULL,track_id INTEGER REFERENCES tracks(id) ON DELETE SET NULL,path TEXT NOT NULL,title TEXT NOT NULL,artist TEXT NOT NULL,album TEXT NOT NULL,album_artist TEXT NOT NULL,artwork TEXT NOT NULL,codec TEXT NOT NULL,duration REAL NOT NULL,number INTEGER NOT NULL,year INTEGER NOT NULL,started_at INTEGER NOT NULL);
      CREATE INDEX history_time ON listening_history(started_at DESC,id DESC);
      CREATE INDEX history_track ON listening_history(track_id);
      CREATE TRIGGER listening_track_snapshot BEFORE DELETE ON tracks BEGIN
        UPDATE favorites SET path=old.path,title=old.title,artist=old.artist,album=old.album,album_artist=old.album_artist,artwork=old.artwork,codec=old.codec,duration=old.duration,number=old.number,year=old.year WHERE track_id=old.id;
        UPDATE listening_history SET path=old.path,title=old.title,artist=old.artist,album=old.album,album_artist=old.album_artist,artwork=old.artwork,codec=old.codec,duration=old.duration,number=old.number,year=old.year WHERE track_id=old.id;
      END;
      PRAGMA user_version=9; RELEASE migration;").map_err(|e|e.to_string())
}
impl Library {
    pub fn listening_view(&self, view: u32, offset: u32, limit: u32) -> Result<(Vec<Track>, u64)> {
        let tx = self.db.unchecked_transaction().map_err(|e| e.to_string())?;
        let revision = self.listening_revision()?;
        let rows = self.listening_tracks(view, offset, limit)?;
        tx.commit().map_err(|e| e.to_string())?;
        Ok((rows, revision))
    }
    /// Views: 0 favorites, 1 recently indexed, 2 successful playback starts (newest first).
    pub fn listening_tracks(&self, view: u32, offset: u32, limit: u32) -> Result<Vec<Track>> {
        let sql = match view {
            1 => "SELECT id,folder_id,path,title,artist,album,album_artist,artwork,codec,duration,number,year,EXISTS(SELECT 1 FROM favorites f WHERE f.original_id=tracks.id),id,added_at FROM tracks ORDER BY added_at DESC,id DESC LIMIT ? OFFSET ?".to_owned(),
            0 | 2 => {
                let (table, time, order) = if view==0 { ("favorites","favorited_at","i.favorited_at DESC,i.original_id DESC") } else { ("listening_history","started_at","i.started_at DESC,i.id DESC") };
                format!("SELECT coalesce(t.id,0),coalesce(t.folder_id,0),coalesce(t.path,i.path),coalesce(t.title,'Unavailable · '||i.title),coalesce(t.artist,i.artist),coalesce(t.album,i.album),coalesce(t.album_artist,i.album_artist),coalesce(t.artwork,i.artwork),coalesce(t.codec,i.codec),coalesce(t.duration,i.duration),coalesce(t.number,i.number),coalesce(t.year,i.year),EXISTS(SELECT 1 FROM favorites f WHERE f.original_id=i.original_id),i.original_id,i.{time} FROM {table} i LEFT JOIN tracks t ON t.id=i.track_id ORDER BY {order} LIMIT ? OFFSET ?")
            }
            _ => return Err("Unknown listening view.".into()),
        };
        let mut stmt = self.db.prepare(&sql).map_err(|e| e.to_string())?;
        stmt.query_map(params![limit, offset], track_row)
            .map_err(|e| e.to_string())?
            .collect::<std::result::Result<_, _>>()
            .map_err(|e| e.to_string())
    }
    /// IDs are original stable library IDs; removing a missing favorite is still possible.
    pub fn set_favorites(&self, ids: &[i64], favorite: bool) -> Result<()> {
        if ids.is_empty() || ids.len() > 100_000 || ids.iter().any(|id| *id <= 0) {
            return Err("Choose library tracks first.".into());
        }
        let tx = self.db.unchecked_transaction().map_err(|e| e.to_string())?;
        if favorite {
            let tracks = self.selected_tracks(ids)?; // Resolve every ID before writing any favorite.
            let mut stmt=tx.prepare("INSERT INTO favorites(original_id,track_id,path,title,artist,album,album_artist,artwork,codec,duration,number,year,favorited_at) VALUES(?,?,?,?,?,?,?,?,?,?,?,?,unixepoch()) ON CONFLICT(original_id) DO NOTHING").map_err(|e|e.to_string())?;
            for t in tracks {
                stmt.execute(params![
                    t.id,
                    t.id,
                    t.path,
                    t.title,
                    t.artist,
                    t.album,
                    t.album_artist,
                    t.artwork,
                    t.codec,
                    t.duration,
                    t.number,
                    t.year
                ])
                .map_err(|e| e.to_string())?;
            }
        } else {
            for id in ids {
                tx.execute("DELETE FROM favorites WHERE original_id=?", [id])
                    .map_err(|e| e.to_string())?;
            }
        }
        tx.commit().map_err(|e| e.to_string())
    }
    /// Atomic batch; caller acknowledges audio events only after commit, so transient errors can retry.
    pub fn record_listens(&self, events: &[Listen]) -> Result<()> {
        if events.is_empty() {
            return Ok(());
        }
        let tx = self.db.unchecked_transaction().map_err(|e| e.to_string())?;
        for event in events {
            let original = &event.track;
            if original.id <= 0 || event.started_at < 0 {
                return Err("Invalid listening event.".into());
            }
            let t=self.db.query_row("SELECT id,folder_id,path,title,artist,album,album_artist,artwork,codec,duration,number,year,EXISTS(SELECT 1 FROM favorites f WHERE f.original_id=tracks.id),id,added_at FROM tracks WHERE id=?",[original.id],track_row).optional().map_err(|e|e.to_string())?.unwrap_or_else(||original.clone());
            tx.execute("INSERT INTO listening_history(original_id,track_id,path,title,artist,album,album_artist,artwork,codec,duration,number,year,started_at) VALUES(?,(SELECT id FROM tracks WHERE id=?),?,?,?,?,?,?,?,?,?,?,?)",params![original.id,original.id,t.path,t.title.strip_prefix("Unavailable · ").unwrap_or(&t.title),t.artist,t.album,t.album_artist,t.artwork,t.codec,t.duration,t.number,t.year,event.started_at]).map_err(|e|e.to_string())?;
        }
        tx.commit().map_err(|e| e.to_string())
    }
    pub fn listening_revision(&self) -> Result<u64> {
        self.db
            .query_row(
                "SELECT coalesce(max(id),0) FROM listening_history",
                [],
                |r| r.get::<_, i64>(0),
            )
            .map(|id| id.max(0) as u64)
            .map_err(|e| e.to_string())
    }
    pub fn favorite(&self, original_id: i64) -> Result<bool> {
        self.db
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM favorites WHERE original_id=?)",
                [original_id],
                |r| r.get(0),
            )
            .map_err(|e| e.to_string())
    }
}
#[cfg(test)]
pub(super) fn drop_migration(db: &Connection) {
    db.execute_batch("DROP TRIGGER listening_track_snapshot; DROP TABLE favorites; DROP TABLE listening_history; DROP INDEX tracks_added; ALTER TABLE tracks DROP COLUMN added_at;").unwrap();
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn favorites_recent_and_history_survive_rename_restart_and_deletion_without_file_writes() {
        let dir = crate::tests::sandbox();
        let root = dir.join("Music");
        fs::create_dir(&root).unwrap();
        let a = root.join("Żółty.wav");
        crate::tests::wav(&a);
        let bytes = fs::read(&a).unwrap();
        let db = dir.join("index.db");
        let lib = Library::open(&db).unwrap();
        lib.add_root(&root).unwrap();
        let t = lib.listening_tracks(1, 0, 10).unwrap().remove(0);
        assert!(t.event_time > 0);
        lib.set_favorites(&[t.id, t.id], true).unwrap();
        assert_eq!(lib.listening_tracks(0, 0, 10).unwrap().len(), 1);
        assert!(lib.selected_tracks(&[t.id]).unwrap()[0].favorite);
        assert!(lib.set_favorites(&[t.id, i64::MAX], true).is_err());
        let event = Listen {
            track: t.clone(),
            started_at: 10,
        };
        lib.record_listens(&[event.clone(), event]).unwrap();
        let moved = root.join("Renamed.wav");
        fs::rename(a, &moved).unwrap();
        lib.reconcile().unwrap();
        assert_eq!(
            lib.listening_tracks(0, 0, 10).unwrap()[0].path,
            display_path(&moved)
        );
        assert_eq!(lib.listening_tracks(2, 0, 10).unwrap().len(), 2);
        let b = root.join("New.wav");
        crate::tests::wav(&b);
        lib.reconcile().unwrap();
        let recent = lib.listening_tracks(1, 0, 10).unwrap();
        assert_ne!(recent[0].id, t.id);
        assert_eq!(recent[1].event_time, t.event_time);
        assert_eq!(fs::read(&moved).unwrap(), bytes);
        drop(lib);
        let lib = Library::open(&db).unwrap();
        assert!(lib.favorite(t.id).unwrap());
        fs::remove_file(moved).unwrap();
        lib.reconcile().unwrap();
        let ghost = lib.listening_tracks(0, 0, 10).unwrap().remove(0);
        assert_eq!(ghost.id, 0);
        assert_eq!(ghost.reference_id, t.id);
        assert!(ghost.favorite);
        assert!(ghost.title.starts_with("Unavailable · "));
        assert_eq!(lib.listening_tracks(2, 0, 10).unwrap()[0].id, 0);
        assert_eq!(lib.listening_tracks(2, 1, 1).unwrap().len(), 1);
        lib.set_favorites(&[ghost.reference_id], false).unwrap();
        assert!(lib.listening_tracks(0, 0, 10).unwrap().is_empty());
        assert!(!lib.listening_tracks(2, 0, 10).unwrap()[0].favorite);
        drop(lib);
        fs::remove_dir_all(dir).unwrap();
    }
    #[test]
    fn history_batch_rolls_back_and_schema_eight_migration_preserves_session_and_playlists() {
        let dir = crate::tests::sandbox();
        let db = dir.join("index.db");
        let lib = Library::open(&db).unwrap();
        let playlist = lib.playlist_command(0, 0, "Keep", &[], 0).unwrap();
        let session = player_domain::Session {
            tracks: vec![Track {
                path: "kept.wav".into(),
                ..Default::default()
            }]
            .into(),
            ..Default::default()
        };
        lib.save_session(&session, true).unwrap();
        drop_migration(&lib.db);
        lib.db.execute_batch("PRAGMA user_version=8").unwrap();
        drop(lib);
        let lib = Library::open(&db).unwrap();
        assert_eq!(lib.playlists().unwrap()[0].id, playlist);
        assert_eq!(lib.load_session().unwrap().tracks[0].path, "kept.wav");
        let event = Listen {
            track: Track {
                id: 12,
                path: "deleted.wav".into(),
                ..Default::default()
            },
            started_at: 20,
        };
        lib.db.execute_batch("CREATE TRIGGER fail_history BEFORE INSERT ON listening_history WHEN (SELECT count(*) FROM listening_history)=1 BEGIN SELECT RAISE(ABORT,'simulated history write failure'); END;").unwrap();
        assert!(lib.record_listens(&[event.clone(), event.clone()]).is_err());
        assert!(lib.listening_tracks(2, 0, 10).unwrap().is_empty());
        lib.db.execute_batch("DROP TRIGGER fail_history").unwrap();
        lib.record_listens(&[event]).unwrap();
        assert_eq!(lib.listening_tracks(2, 0, 10).unwrap()[0].id, 0);
        drop(lib);
        fs::remove_dir_all(dir).unwrap();
    }
}
