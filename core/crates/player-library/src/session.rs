use super::*;
use player_domain::Session;
impl Library {
    pub fn load_session(&self) -> Result<Session> {
        let tx = self.db.unchecked_transaction().map_err(|e| e.to_string())?;
        let settings = self.db.query_row("SELECT current_index,next_end,position,volume,flags FROM playback_session WHERE id=1", [], |r|Ok((r.get::<_,Option<i64>>(0)?,r.get::<_,Option<i64>>(1)?,r.get::<_,f64>(2)?,r.get::<_,f32>(3)?,r.get::<_,u32>(4)?))).optional().map_err(|e|e.to_string())?;
        let Some((current, next_end, position, volume, flags)) = settings else {
            tx.commit().map_err(|e| e.to_string())?;
            return Ok(Session::default());
        };
        let mut stmt = self.db.prepare("SELECT coalesce(t.id,0),coalesce(t.folder_id,0),coalesce(t.path,i.path),coalesce(t.title,'Unavailable · '||i.title),coalesce(t.artist,i.artist),coalesce(t.album,i.album),coalesce(t.album_artist,i.album_artist),coalesce(t.artwork,i.artwork),coalesce(t.codec,i.codec),coalesce(t.duration,i.duration),coalesce(t.number,i.number),coalesce(t.year,i.year) FROM session_items i LEFT JOIN tracks t ON t.id=i.track_id ORDER BY i.position").map_err(|e|e.to_string())?;
        let rows = stmt
            .query_map([], track_row)
            .map_err(|e| e.to_string())?
            .collect::<std::result::Result<Vec<_>, _>>()
            .map_err(|e| e.to_string())?;
        let index = |v: Option<i64>| -> Result<Option<usize>> {
            v.map(|i| usize::try_from(i).map_err(|_| "Saved queue position is invalid.".to_owned()))
                .transpose()
        };
        let session = Session {
            tracks: rows.into(),
            current: index(current)?,
            next_end: index(next_end)?,
            position,
            volume,
            flags,
        };
        session.validate()?;
        drop(stmt);
        tx.commit().map_err(|e| e.to_string())?;
        Ok(session)
    }
    pub fn save_session(&self, session: &Session, replace_items: bool) -> Result<()> {
        session.validate()?;
        let tx = self.db.unchecked_transaction().map_err(|e| e.to_string())?;
        if replace_items {
            tx.execute("DELETE FROM session_items", [])
                .map_err(|e| e.to_string())?;
            let mut stmt=tx.prepare("INSERT INTO session_items(position,track_id,path,title,artist,album,album_artist,artwork,codec,duration,number,year) VALUES(?,(SELECT id FROM tracks WHERE id=?),?,?,?,?,?,?,?,?,?,?)").map_err(|e|e.to_string())?;
            for (i, t) in session.tracks.iter().enumerate() {
                stmt.execute(params![
                    i as i64,
                    t.id,
                    t.path,
                    t.title.strip_prefix("Unavailable · ").unwrap_or(&t.title),
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
        }
        tx.execute("INSERT INTO playback_session(id,current_index,next_end,position,volume,flags) VALUES(1,?,?,?,?,?) ON CONFLICT(id) DO UPDATE SET current_index=excluded.current_index,next_end=excluded.next_end,position=excluded.position,volume=excluded.volume,flags=excluded.flags",params![session.current.map(|i|i as i64),session.next_end.map(|i|i as i64),session.position,session.volume,session.flags]).map_err(|e|e.to_string())?;
        tx.commit().map_err(|e| e.to_string())
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn session_persists_duplicate_occurrences_cursor_and_unavailable_snapshots() {
        let p = super::super::tests::sandbox();
        let root = p.join("Music");
        fs::create_dir(&root).unwrap();
        let file = root.join("one.wav");
        super::super::tests::wav(&file);
        let db = p.join("index.db");
        let lib = Library::open(&db).unwrap();
        lib.add_root(&root).unwrap();
        let folder = lib.folders(0).unwrap()[0].id;
        let track = lib.tracks(folder, 0, 10).unwrap().remove(0);
        let mut session = Session {
            tracks: vec![track.clone(), track.clone(), track].into(),
            current: Some(1),
            next_end: Some(3),
            position: 0.05,
            volume: 0.2,
            flags: 5,
        };
        lib.save_session(&session, true).unwrap();
        session.position = 0.07;
        lib.save_session(&session, false).unwrap();
        drop(lib);
        let lib = Library::open(&db).unwrap();
        let restored = lib.load_session().unwrap();
        assert_eq!(restored.tracks.len(), 3);
        assert_eq!(restored.current, Some(1));
        assert_eq!(restored.next_end, Some(3));
        assert_eq!(restored.position, 0.07);
        assert_eq!(restored.flags, 5);
        assert_eq!(restored.volume, 0.2);
        fs::remove_file(file).unwrap();
        lib.reconcile().unwrap();
        let missing = lib.load_session().unwrap();
        assert_eq!(missing.tracks.len(), 3);
        assert_eq!(missing.tracks[0].id, 0);
        assert!(missing.tracks[0].title.starts_with("Unavailable · "));
        drop(lib);
        fs::remove_dir_all(p).unwrap();
    }
    #[test]
    fn failed_session_write_is_atomic_and_migration_preserves_app_state() {
        let p = super::super::tests::sandbox();
        let db = p.join("index.db");
        let lib = Library::open(&db).unwrap();
        let playlist = lib
            .playlist_command(0, 0, "Keep my playlist", &[], 0)
            .unwrap();
        lib.db.execute_batch("DROP TRIGGER session_track_snapshot; DROP TABLE session_items; DROP TABLE playback_session; ALTER TABLE tracks DROP COLUMN embedded_art; ALTER TABLE tracks DROP COLUMN art_checked; ALTER TABLE folders DROP COLUMN artwork; ALTER TABLE folders DROP COLUMN art_source; PRAGMA user_version=6;").unwrap();
        drop(lib);
        let lib = Library::open(&db).unwrap();
        assert_eq!(lib.playlists().unwrap()[0].id, playlist);
        assert!(lib.load_session().unwrap().tracks.is_empty());
        let old = Session {
            tracks: vec![
                Track {
                    path: "one.wav".into(),
                    ..Default::default()
                },
                Track {
                    path: "two.wav".into(),
                    ..Default::default()
                },
            ]
            .into(),
            volume: 0.2,
            ..Default::default()
        };
        lib.save_session(&old, true).unwrap();
        lib.db.execute_batch("CREATE TRIGGER fail_session BEFORE INSERT ON session_items WHEN new.position=1 BEGIN SELECT RAISE(ABORT,'simulated write failure'); END;").unwrap();
        let changed = Session {
            tracks: vec![
                Track {
                    path: "replacement.wav".into(),
                    ..Default::default()
                };
                3
            ]
            .into(),
            volume: 0.9,
            ..Default::default()
        };
        assert!(lib.save_session(&changed, true).is_err());
        let restored = lib.load_session().unwrap();
        assert_eq!(restored.volume, 0.2);
        assert_eq!(restored.tracks.len(), 2);
        assert_eq!(restored.tracks[0].path, "one.wav");
        assert_eq!(restored.tracks[1].path, "two.wav");
        drop(lib);
        fs::remove_dir_all(p).unwrap();
    }
}
