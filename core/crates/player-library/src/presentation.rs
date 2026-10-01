use super::{Library, Result};
use player_domain::{Folder, FolderEvidence, FolderKind};
impl Library {
    pub(super) fn decorate_folders(&self, mut folders: Vec<Folder>) -> Result<Vec<Folder>> {
        let mut stats = self.db.prepare("SELECT count(*),
          count(DISTINCT lower(trim(album)))=1 AND count(*)=sum(trim(album)!=''),
          count(DISTINCT lower(trim(CASE WHEN trim(album_artist)!='' THEN album_artist ELSE artist END)))=1
            AND count(*)=sum(trim(CASE WHEN trim(album_artist)!='' THEN album_artist ELSE artist END)!=''),
          count(*)=count(DISTINCT number) AND count(*)=sum(number>0)
          FROM tracks WHERE folder_id=?").map_err(|e|e.to_string())?;
        let mut overrides = self
            .db
            .prepare("SELECT presentation FROM folders WHERE id=?")
            .map_err(|e| e.to_string())?;
        for folder in &mut folders {
            let value: u32 = overrides
                .query_row([folder.id], |r| r.get(0))
                .map_err(|e| e.to_string())?;
            folder.override_kind = FolderKind::from_value(value)?;
            let evidence = stats
                .query_row([folder.id], |r| {
                    Ok(FolderEvidence {
                        track_count: r.get::<_, i64>(0)? as u64,
                        has_children: folder.has_children,
                        shared_album: r.get::<_, Option<bool>>(1)?.unwrap_or(false),
                        shared_artist: r.get::<_, Option<bool>>(2)?.unwrap_or(false),
                        unique_numbers: r.get::<_, Option<bool>>(3)?.unwrap_or(false),
                    })
                })
                .map_err(|e| e.to_string())?;
            folder.track_count = evidence.track_count;
            folder.kind = if folder.override_kind == FolderKind::Auto {
                FolderKind::infer(&evidence)
            } else {
                folder.override_kind
            };
        }
        Ok(folders)
    }
    pub fn set_folder_kind(&self, id: i64, value: u32) -> Result<Folder> {
        let kind = FolderKind::from_value(value)?;
        let changed = self
            .db
            .execute(
                "UPDATE folders SET presentation=? WHERE id=?",
                rusqlite::params![kind as u32, id],
            )
            .map_err(|e| e.to_string())?;
        if changed != 1 {
            return Err("This folder is no longer in the library.".into());
        }
        self.folder(id)?
            .ok_or_else(|| "This folder is no longer in the library.".into())
    }
}
