//! Read-only artwork inputs; bounded, content-addressed thumbnails live beside the app database.
use super::{Library, Result, display_path, stamp};
use image::{ImageFormat, ImageReader, Limits};
use lofty::{file::TaggedFileExt, picture::PictureType};
use sha2::{Digest, Sha256};
use std::{
    fs,
    io::{Cursor, Read, Write},
    path::{Path, PathBuf},
};
const INPUT_LIMIT: u64 = 8 * 1024 * 1024;
const CACHE_LIMIT: u64 = 256 * 1024 * 1024;
const EDGE: u32 = 512;
pub(super) struct Cache {
    dir: PathBuf,
    used: u64,
    limit: u64,
    pub warnings: Vec<String>,
    pub retry: bool,
    exhausted: bool,
}
impl Cache {
    pub fn new(dir: PathBuf) -> Self {
        let used = fs::read_dir(&dir)
            .into_iter()
            .flatten()
            .flatten()
            .filter(|e| e.file_type().is_ok_and(|t| t.is_file()))
            .filter_map(|e| e.metadata().ok())
            .map(|m| m.len())
            .sum();
        Self {
            dir,
            used,
            limit: CACHE_LIMIT,
            warnings: Vec::new(),
            retry: false,
            exhausted: used >= CACHE_LIMIT,
        }
    }
    pub fn full(&self) -> bool {
        self.exhausted
    }
    pub fn prune(&mut self, db: &rusqlite::Connection) -> Result<()> {
        // Retain all library and saved-session references; only old generated orphans are disposable.
        let mut stmt=db.prepare("SELECT embedded_art FROM tracks UNION SELECT artwork FROM folders UNION SELECT artwork FROM session_items UNION SELECT artwork FROM favorites UNION SELECT artwork FROM listening_history").map_err(|e|e.to_string())?;
        let keep = stmt
            .query_map([], |r| r.get::<_, String>(0))
            .map_err(|e| e.to_string())?
            .collect::<std::result::Result<std::collections::HashSet<_>, _>>()
            .map_err(|e| e.to_string())?;
        for entry in fs::read_dir(&self.dir).into_iter().flatten().flatten() {
            let path = entry.path();
            let name = entry.file_name();
            let name = name.to_string_lossy();
            let owned = name.starts_with("v1-")
                && (name.ends_with(".png") || name.ends_with(".tmp"))
                && name.len() == 71
                && name.as_bytes()[3..67].iter().all(u8::is_ascii_hexdigit);
            if !owned || keep.contains(&display_path(&path)) {
                continue;
            }
            if let Ok(meta) = entry.metadata()
                && meta.is_file()
                && meta
                    .modified()
                    .ok()
                    .and_then(|t| t.elapsed().ok())
                    .is_some_and(|age| age.as_secs() >= 24 * 60 * 60)
                && fs::remove_file(&path).is_ok()
            {
                self.used = self.used.saturating_sub(meta.len());
                self.exhausted = self.used >= self.limit;
            }
        }
        Ok(())
    }
    fn warn(&mut self, error: String) {
        if self.warnings.len() < 5 && !self.warnings.contains(&error) {
            self.warnings.push(error);
        }
    }
    pub fn store(&mut self, data: &[u8]) -> Result<String> {
        if data.is_empty() || data.len() as u64 > INPUT_LIMIT {
            return Err("Artwork exceeds the 8 MiB input limit.".into());
        }
        let key = format!("v1-{:x}.png", Sha256::digest(data));
        let path = self.dir.join(key);
        if path.is_file() && fs::metadata(&path).is_ok_and(|m| m.len() > 0) {
            return Ok(display_path(&path));
        }
        if self.used >= self.limit || self.exhausted {
            self.retry = true;
            self.exhausted = true;
            return Err("Artwork cache reached its 256 MiB limit; music remains available.".into());
        }
        let mut reader = ImageReader::new(Cursor::new(data))
            .with_guessed_format()
            .map_err(|e| e.to_string())?;
        if !matches!(reader.format(), Some(ImageFormat::Png | ImageFormat::Jpeg)) {
            return Err("Artwork is not a supported JPEG or PNG image.".into());
        }
        let mut limits = Limits::default();
        limits.max_image_width = Some(4096);
        limits.max_image_height = Some(4096);
        limits.max_alloc = Some(64 * 1024 * 1024);
        reader.limits(limits);
        let decoded = reader
            .decode()
            .map_err(|e| format!("Cannot decode artwork: {e}"))?;
        let thumbnail = if decoded.width() > EDGE || decoded.height() > EDGE {
            decoded.thumbnail(EDGE, EDGE)
        } else {
            decoded
        };
        let mut encoded = Cursor::new(Vec::new());
        thumbnail
            .write_to(&mut encoded, ImageFormat::Png)
            .map_err(|e| e.to_string())?;
        let data = encoded.into_inner();
        if self.used.saturating_add(data.len() as u64) > self.limit {
            self.retry = true;
            self.exhausted = true;
            return Err("Artwork cache reached its 256 MiB limit; music remains available.".into());
        }
        fs::create_dir_all(&self.dir).map_err(|e| {
            self.retry = true;
            format!("Cannot create artwork cache: {e}")
        })?;
        // The scan lock serializes cache writers. Publish a complete file by same-directory rename.
        let temp = path.with_extension("tmp");
        let result = (|| -> Result<()> {
            let mut file = fs::File::create(&temp).map_err(|e| e.to_string())?;
            file.write_all(&data).map_err(|e| e.to_string())?;
            file.sync_all().map_err(|e| e.to_string())?;
            drop(file);
            if path.exists() {
                fs::remove_file(&path).map_err(|e| e.to_string())?;
            }
            fs::rename(&temp, &path).map_err(|e| e.to_string())?;
            Ok(())
        })();
        if result.is_err() {
            self.retry = true;
            let _ = fs::remove_file(temp);
        }
        result.map_err(|e| format!("Cannot save artwork cache: {e}"))?;
        self.used += data.len() as u64;
        Ok(display_path(&path))
    }
    pub fn embedded(&mut self, file: &lofty::file::TaggedFile) -> String {
        let pictures = file.tags().iter().flat_map(|t| t.pictures());
        // Prefer a front cover, then another decodable picture. A damaged first picture does not hide a valid one.
        for front in [true, false] {
            for picture in pictures
                .clone()
                .filter(|p| (p.pic_type() == PictureType::CoverFront) == front)
            {
                match self.store(picture.data()) {
                    Ok(path) => return path,
                    Err(e) => self.warn(e),
                }
            }
        }
        String::new()
    }
    pub fn folder(&mut self, folder: &Path, previous: &(String, String)) -> (String, String) {
        let names = [
            "cover.jpg",
            "folder.jpg",
            "front.jpg",
            "cover.png",
            "folder.png",
            "front.png",
            "cover.jpeg",
            "folder.jpeg",
            "front.jpeg",
        ];
        let entries = fs::read_dir(folder)
            .into_iter()
            .flatten()
            .flatten()
            .collect::<Vec<_>>();
        for name in names {
            let Some(entry) = entries
                .iter()
                .find(|e| e.file_name().to_string_lossy().eq_ignore_ascii_case(name))
            else {
                continue;
            };
            let path = entry.path();
            let Ok(meta) = fs::symlink_metadata(&path) else {
                continue;
            };
            if !meta.is_file() || meta.is_symlink() {
                continue;
            }
            #[cfg(windows)]
            {
                use std::os::windows::fs::MetadataExt;
                if meta.file_attributes() & 0x400 != 0 {
                    continue;
                }
            }
            let signature = format!("{}|{}|{}", display_path(&path), meta.len(), stamp(&meta));
            if signature == previous.1
                && (previous.0.is_empty() || Path::new(&previous.0).is_file())
            {
                return previous.clone();
            }
            let image = (|| -> Result<String> {
                if meta.len() > INPUT_LIMIT {
                    return Err("Folder artwork exceeds the 8 MiB input limit.".into());
                }
                let mut data = Vec::new();
                fs::File::open(&path)
                    .map_err(|e| e.to_string())?
                    .take(INPUT_LIMIT + 1)
                    .read_to_end(&mut data)
                    .map_err(|e| e.to_string())?;
                self.store(&data)
            })();
            match image {
                Ok(art) => return (art, signature),
                Err(e) => self.warn(format!("{}: {e}", path.display())),
            }
        }
        (String::new(), String::new())
    }
}
impl Library {
    pub(super) fn apply_artwork(&self, folder: i64, album: bool) -> Result<bool> {
        let shared: String = self
            .db
            .query_row("SELECT artwork FROM folders WHERE id=?", [folder], |r| {
                r.get(0)
            })
            .map_err(|e| e.to_string())?;
        let use_folder = album && !shared.is_empty();
        Ok(self.db.execute("UPDATE tracks SET artwork=CASE WHEN ? THEN ? ELSE embedded_art END WHERE folder_id=? AND artwork!=CASE WHEN ? THEN ? ELSE embedded_art END",rusqlite::params![use_folder,shared,folder,use_folder,shared]).map_err(|e|e.to_string())?>0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{DynamicImage, ImageBuffer, Rgb};
    use lofty::{
        picture::Picture,
        tag::{Accessor, Tag, TagExt, TagType},
    };
    fn png(width: u32, height: u32, color: [u8; 3]) -> Vec<u8> {
        let image = DynamicImage::ImageRgb8(ImageBuffer::from_pixel(width, height, Rgb(color)));
        let mut out = Cursor::new(Vec::new());
        image.write_to(&mut out, ImageFormat::Png).unwrap();
        out.into_inner()
    }
    fn tagged(path: &Path, number: u32, data: Option<&[u8]>) {
        crate::tests::wav(path);
        let mut tag = Tag::new(TagType::Id3v2);
        tag.set_title(format!("Song {number}"));
        tag.set_artist("Test artist".into());
        tag.set_album("Test album".into());
        tag.set_track(number);
        if let Some(data) = data {
            let mut p = Picture::from_reader(&mut Cursor::new(data)).unwrap();
            p.set_pic_type(PictureType::CoverFront);
            tag.push_picture(p);
        }
        tag.save_to_path(path, lofty::config::WriteOptions::default())
            .unwrap();
    }
    #[test]
    fn bounded_thumbnails_deduplicate_reject_invalid_inputs_and_cache_exhaustion() {
        let dir = crate::tests::sandbox();
        let mut cache = Cache::new(dir.join("art"));
        let data = png(800, 400, [12, 80, 150]);
        let path = cache.store(&data).unwrap();
        assert_eq!(image::image_dimensions(&path).unwrap(), (512, 256));
        let used = cache.used;
        assert_eq!(cache.store(&data).unwrap(), path);
        assert_eq!(cache.used, used);
        let small = cache.store(&png(100, 200, [1, 2, 4])).unwrap();
        assert_eq!(image::image_dimensions(small).unwrap(), (100, 200));
        let mut jpeg = Cursor::new(Vec::new());
        DynamicImage::ImageRgb8(ImageBuffer::from_pixel(600, 300, Rgb([5, 6, 7])))
            .write_to(&mut jpeg, ImageFormat::Jpeg)
            .unwrap();
        let jpeg = cache.store(&jpeg.into_inner()).unwrap();
        assert_eq!(image::image_dimensions(jpeg).unwrap(), (512, 256));
        assert!(cache.store(b"not a picture").is_err());
        assert!(cache.store(&vec![0; INPUT_LIMIT as usize + 1]).is_err());
        assert!(cache.store(&png(4097, 1, [0, 0, 0])).is_err());
        cache.limit = cache.used;
        assert!(cache.store(&png(20, 20, [0, 1, 2])).is_err());
        assert_eq!(cache.store(&data).unwrap(), path); // Existing covers work even when the cache is full.
        fs::remove_dir_all(dir).unwrap();
    }
    #[test]
    fn embedded_front_cover_wins_and_malformed_picture_falls_back() {
        let dir = crate::tests::sandbox();
        let mut cache = Cache::new(dir.join("art"));
        let red = png(60, 60, [200, 0, 0]);
        let blue = png(60, 60, [0, 0, 200]);
        let mut tag = Tag::new(TagType::Id3v2);
        let mut back = Picture::from_reader(&mut Cursor::new(&blue)).unwrap();
        back.set_pic_type(PictureType::CoverBack);
        tag.push_picture(back);
        let mut front = Picture::from_reader(&mut Cursor::new(&red)).unwrap();
        front.set_pic_type(PictureType::CoverFront);
        tag.push_picture(front);
        let file = dir.join("one.wav");
        crate::tests::wav(&file);
        tag.save_to_path(&file, lofty::config::WriteOptions::default())
            .unwrap();
        let tagged = lofty::read_from_path(&file).unwrap();
        assert_eq!(cache.embedded(&tagged), cache.store(&red).unwrap());
        let mut bad = Picture::from_reader(&mut Cursor::new(&red[..16])).unwrap();
        bad.set_pic_type(PictureType::CoverFront);
        tag.remove_picture_type(PictureType::CoverFront);
        tag.push_picture(bad);
        tag.save_to_path(&file, lofty::config::WriteOptions::default())
            .unwrap();
        let tagged = lofty::read_from_path(&file).unwrap();
        assert_eq!(cache.embedded(&tagged), cache.store(&blue).unwrap());
        assert!(!cache.warnings.is_empty());
        fs::remove_dir_all(dir).unwrap();
    }
    #[test]
    fn album_and_loose_artwork_stay_distinct_across_rescan_rename_and_missing_cache() {
        let dir = crate::tests::sandbox();
        let root = dir.join("Music");
        fs::create_dir(&root).unwrap();
        let red = png(80, 80, [180, 0, 0]);
        let blue = png(80, 80, [0, 0, 180]);
        let green = png(100, 60, [0, 180, 0]);
        let a = root.join("one.wav");
        let b = root.join("two.wav");
        tagged(&a, 1, Some(&red));
        tagged(&b, 2, Some(&blue));
        let before = (fs::read(&a).unwrap(), fs::read(&b).unwrap());
        fs::write(root.join("COVER.PNG"), &green).unwrap();
        let lib = Library::open(&dir.join("index.db")).unwrap();
        assert!(lib.add_root(&root).unwrap().warnings.is_empty());
        let folder = lib.folders(0).unwrap().remove(0);
        assert_eq!(folder.kind, crate::FolderKind::Album);
        assert!(!folder.artwork.is_empty());
        let rows = lib.tracks(folder.id, 0, 10).unwrap();
        assert!(rows.iter().all(|t| t.artwork == folder.artwork));
        assert!(!lib.reconcile().unwrap().changed);
        let loose = lib
            .set_folder_kind(folder.id, crate::FolderKind::LooseTracks as u32)
            .unwrap();
        assert!(loose.artwork.is_empty());
        let rows = lib.tracks(folder.id, 0, 10).unwrap();
        assert_ne!(rows[0].artwork, rows[1].artwork);
        assert!(rows.iter().all(|t| t.artwork != folder.artwork));
        let one_art = rows[0].artwork.clone();
        fs::remove_file(&one_art).unwrap();
        assert!(lib.reconcile().unwrap().changed);
        assert!(Path::new(&one_art).is_file());
        let moved = root.join("renamed.wav");
        fs::rename(&a, &moved).unwrap();
        lib.reconcile().unwrap();
        let renamed = lib.tracks(folder.id, 0, 10).unwrap();
        assert_eq!(
            renamed
                .iter()
                .find(|t| t.path == display_path(&moved))
                .unwrap()
                .id,
            rows[0].id
        );
        assert_eq!(fs::read(moved).unwrap(), before.0);
        assert_eq!(fs::read(&b).unwrap(), before.1);
        fs::remove_file(root.join("COVER.PNG")).unwrap();
        lib.set_folder_kind(folder.id, crate::FolderKind::Auto as u32)
            .unwrap();
        lib.reconcile().unwrap();
        assert_eq!(lib.folder(folder.id).unwrap().unwrap().artwork, one_art);
        // An artwork error is not an incomplete filesystem scan: removed tracks still disappear.
        fs::write(root.join("cover.png"), b"bad cover").unwrap();
        fs::remove_file(b).unwrap();
        let report = lib.reconcile().unwrap();
        assert!(!report.warnings.is_empty());
        assert_eq!(lib.tracks(folder.id, 0, 10).unwrap().len(), 1);
        assert_eq!(
            lib.folder(folder.id).unwrap().unwrap().kind,
            crate::FolderKind::LooseTracks
        );
        drop(lib);
        fs::remove_dir_all(dir).unwrap();
    }
    #[test]
    fn cache_pruning_keeps_session_references_and_never_removes_unknown_files() {
        let dir = crate::tests::sandbox();
        let lib = Library::open(&dir.join("index.db")).unwrap();
        let mut cache = Cache::new(lib.artwork_dir.clone());
        let kept = cache.store(&png(40, 40, [1, 2, 3])).unwrap();
        let orphan = cache.store(&png(40, 40, [4, 5, 6])).unwrap();
        let unknown = cache.dir.join("family.png");
        let unicode = cache.dir.join(format!("v1-{}é0.png", "a".repeat(61)));
        fs::write(&unicode, b"Unknown Unicode filename").unwrap();
        fs::write(&unknown, b"Keep this unknown file").unwrap();
        for path in [Path::new(&kept), Path::new(&orphan), &unknown, &unicode] {
            fs::OpenOptions::new()
                .write(true)
                .open(path)
                .unwrap()
                .set_times(fs::FileTimes::new().set_modified(std::time::UNIX_EPOCH))
                .unwrap();
        }
        lib.save_session(
            &player_domain::Session {
                tracks: vec![player_domain::Track {
                    artwork: kept.clone(),
                    ..Default::default()
                }]
                .into(),
                ..Default::default()
            },
            true,
        )
        .unwrap();
        cache.prune(&lib.db).unwrap();
        assert!(Path::new(&kept).is_file());
        assert!(!Path::new(&orphan).exists());
        assert!(unknown.is_file());
        assert!(unicode.is_file());
        drop(lib);
        fs::remove_dir_all(dir).unwrap();
    }
    #[test]
    fn cache_write_failure_keeps_metadata_and_retries_after_storage_is_repaired() {
        let dir = crate::tests::sandbox();
        let root = dir.join("Music");
        fs::create_dir(&root).unwrap();
        let file = root.join("song.wav");
        tagged(&file, 1, Some(&png(40, 40, [1, 4, 9])));
        let bytes = fs::read(&file).unwrap();
        let lib = Library::open(&dir.join("index.db")).unwrap();
        fs::write(&lib.artwork_dir, b"Blocked cache location").unwrap();
        let report = lib.add_root(&root).unwrap();
        assert!(!report.warnings.is_empty());
        let folder = lib.folders(0).unwrap()[0].id;
        let track = lib.tracks(folder, 0, 10).unwrap().remove(0);
        assert_eq!(track.title, "Song 1");
        assert!(track.duration > 0.0);
        assert!(track.artwork.is_empty());
        fs::remove_file(&lib.artwork_dir).unwrap();
        assert!(lib.reconcile().unwrap().changed);
        assert!(!lib.tracks(folder, 0, 10).unwrap()[0].artwork.is_empty());
        assert_eq!(fs::read(file).unwrap(), bytes);
        drop(lib);
        fs::remove_dir_all(dir).unwrap();
    }
    #[test]
    fn schema_seven_artwork_migration_preserves_saved_session() {
        let dir = crate::tests::sandbox();
        let db = dir.join("index.db");
        let lib = Library::open(&db).unwrap();
        let session = player_domain::Session {
            tracks: vec![
                player_domain::Track {
                    path: "kept.wav".into(),
                    ..Default::default()
                };
                2
            ]
            .into(),
            current: Some(0),
            next_end: Some(2),
            position: 2.0,
            volume: 0.2,
            flags: 5,
        };
        lib.save_session(&session, true).unwrap();
        super::super::listening::drop_migration(&lib.db);
        lib.db.execute_batch("ALTER TABLE tracks DROP COLUMN embedded_art; ALTER TABLE tracks DROP COLUMN art_checked; ALTER TABLE folders DROP COLUMN artwork; ALTER TABLE folders DROP COLUMN art_source; PRAGMA user_version=7;").unwrap();
        drop(lib);
        let lib = Library::open(&db).unwrap();
        let restored = lib.load_session().unwrap();
        assert_eq!(restored.tracks.len(), 2);
        assert_eq!(restored.current, Some(0));
        assert_eq!(restored.next_end, Some(2));
        assert_eq!(restored.position, 2.0);
        assert_eq!(restored.flags, 5);
        assert_eq!(restored.volume, 0.2);
        drop(lib);
        fs::remove_dir_all(dir).unwrap();
    }
}
