use super::*;
use player_domain::Playlist;
impl Library {
    pub fn playlists(&self) -> Result<Vec<Playlist>> {
        let mut stmt=self.db.prepare("SELECT p.id,p.name,(SELECT count(*) FROM playlist_items i WHERE i.playlist_id=p.id) FROM playlists p WHERE deleted=0 ORDER BY p.name COLLATE NOCASE,p.id").map_err(|e|e.to_string())?;
        stmt.query_map([], |r| {
            Ok(Playlist {
                id: r.get(0)?,
                name: r.get(1)?,
                count: r.get::<_, i64>(2)? as u64,
            })
        })
        .map_err(|e| e.to_string())?
        .collect::<std::result::Result<_, _>>()
        .map_err(|e| e.to_string())
    }
    pub fn playlist_tracks(&self, id: i64, offset: u32, limit: u32) -> Result<Vec<Track>> {
        if !self.playlists()?.iter().any(|p| p.id == id) {
            return Err("This playlist is no longer available.".into());
        }
        let mut stmt=self.db.prepare("SELECT coalesce(t.id,0),coalesce(t.folder_id,0),coalesce(t.path,i.path),coalesce(t.title,'Unavailable · '||i.title),coalesce(t.artist,i.artist),coalesce(t.album,i.album),coalesce(t.album_artist,''),coalesce(t.artwork,''),coalesce(t.codec,i.codec),coalesce(t.duration,i.duration),coalesce(t.number,0),coalesce(t.year,0),EXISTS(SELECT 1 FROM favorites f WHERE f.original_id=t.id),coalesce(t.id,0),coalesce(t.added_at,0) FROM playlist_items i LEFT JOIN tracks t ON t.id=i.track_id WHERE i.playlist_id=? ORDER BY i.position,i.id LIMIT ? OFFSET ?").map_err(|e|e.to_string())?;
        stmt.query_map(params![id, limit, offset], track_row)
            .map_err(|e| e.to_string())?
            .collect::<std::result::Result<_, _>>()
            .map_err(|e| e.to_string())
    }
    pub fn playlist_command(
        &self,
        id: i64,
        command: u32,
        name: &str,
        ids: &[i64],
        index: usize,
    ) -> Result<i64> {
        let name = name.trim();
        if matches!(command, 0 | 1) && (name.is_empty() || name.chars().count() > 100) {
            return Err("Choose a playlist name of 1–100 characters.".into());
        }
        if command == 0 {
            let tracks = if ids.is_empty() {
                Vec::new()
            } else {
                self.selected_tracks(ids)?
            };
            let tx = self.db.unchecked_transaction().map_err(|e| e.to_string())?;
            tx.execute("INSERT INTO playlists(name) VALUES(?)", [name])
                .map_err(|e| e.to_string())?;
            let created = tx.last_insert_rowid();
            for (position, t) in tracks.into_iter().enumerate() {
                tx.execute("INSERT INTO playlist_items(playlist_id,track_id,path,title,artist,album,position,duration,codec) VALUES(?,?,?,?,?,?,?,?,?)",params![created,t.id,t.path,t.title,t.artist,t.album,position as i64,t.duration,t.codec]).map_err(|e|e.to_string())?;
            }
            tx.commit().map_err(|e| e.to_string())?;
            return Ok(created);
        }
        if command != 7 && !self.playlists()?.iter().any(|p| p.id == id) {
            return Err("This playlist is no longer available.".into());
        }
        match command {
            1 => {
                self.db
                    .execute("UPDATE playlists SET name=? WHERE id=?", params![name, id])
                    .map_err(|e| e.to_string())?;
            }
            2 | 7 => {
                self.db
                    .execute(
                        "UPDATE playlists SET deleted=? WHERE id=?",
                        params![i32::from(command == 2), id],
                    )
                    .map_err(|e| e.to_string())?;
            }
            3 => {
                let tracks = self.selected_tracks(ids)?;
                let start:i64=self.db.query_row("SELECT coalesce(max(position)+1,0) FROM playlist_items WHERE playlist_id=?",[id],|r|r.get(0)).map_err(|e|e.to_string())?;
                let tx = self.db.unchecked_transaction().map_err(|e| e.to_string())?;
                for (i, t) in tracks.into_iter().enumerate() {
                    tx.execute("INSERT INTO playlist_items(playlist_id,track_id,path,title,artist,album,position,duration,codec) VALUES(?,?,?,?,?,?,?,?,?)",params![id,t.id,t.path,t.title,t.artist,t.album,start+i as i64,t.duration,t.codec]).map_err(|e|e.to_string())?;
                }
                tx.commit().map_err(|e| e.to_string())?;
            }
            4..=6 => {
                let mut stmt = self
                    .db
                    .prepare(
                        "SELECT id FROM playlist_items WHERE playlist_id=? ORDER BY position,id",
                    )
                    .map_err(|e| e.to_string())?;
                let mut rows = stmt
                    .query_map([id], |r| r.get::<_, i64>(0))
                    .map_err(|e| e.to_string())?
                    .collect::<std::result::Result<Vec<_>, _>>()
                    .map_err(|e| e.to_string())?;
                if index >= rows.len() {
                    return Err("This playlist item no longer exists.".into());
                }
                let removed = if command == 4 {
                    Some(rows.remove(index))
                } else {
                    let target = if command == 5 {
                        index.checked_sub(1)
                    } else {
                        index.checked_add(1)
                    }
                    .filter(|i| *i < rows.len())
                    .ok_or("Cannot move this item further.")?;
                    rows.swap(index, target);
                    None
                };
                let tx = self.db.unchecked_transaction().map_err(|e| e.to_string())?;
                if let Some(item) = removed {
                    tx.execute("DELETE FROM playlist_items WHERE id=?", [item])
                        .map_err(|e| e.to_string())?;
                }
                for (position, item) in rows.iter().enumerate() {
                    tx.execute(
                        "UPDATE playlist_items SET position=? WHERE id=?",
                        params![position as i64, item],
                    )
                    .map_err(|e| e.to_string())?;
                }
                tx.commit().map_err(|e| e.to_string())?;
            }
            _ => return Err("Unknown playlist action.".into()),
        }
        Ok(id)
    }
}
