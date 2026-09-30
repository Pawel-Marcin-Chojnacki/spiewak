//! Filesystem identities and playback queue rules, independent of UI and storage.
#[derive(Clone, Debug, Default)]
pub struct Track {
    pub id: i64,
    pub folder_id: i64,
    pub path: String,
    pub title: String,
    pub artist: String,
    pub album: String,
    pub album_artist: String,
    pub artwork: String,
    pub codec: String,
    pub duration: f64,
    pub number: u32,
    pub year: u32,
}

#[derive(Clone, Debug)]
pub struct Folder {
    pub id: i64,
    pub parent_id: i64,
    pub name: String,
    pub path: String,
    pub has_children: bool,
}

#[derive(Clone, Default)]
pub struct Queue {
    pub tracks: std::sync::Arc<[Track]>,
    pub current: Option<usize>,
}
impl Queue {
    pub fn replace(&mut self, tracks: Vec<Track>, start: usize) -> Result<(), String> {
        if tracks.is_empty() || start >= tracks.len() {
            return Err("Choose a playable track first.".into());
        }
        self.tracks = tracks.into();
        self.current = Some(start);
        Ok(())
    }
    pub fn advance(&mut self) -> Option<Track> {
        let next = self.current? + 1;
        if next >= self.tracks.len() {
            return None;
        }
        self.current = Some(next);
        self.tracks.get(next).cloned()
    }
    pub fn previous(&mut self) -> Option<Track> {
        let previous = self.current?.saturating_sub(1);
        self.current = Some(previous);
        self.tracks.get(previous).cloned()
    }
    pub fn selected(&self) -> Option<&Track> {
        self.tracks.get(self.current?)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn track(id: i64) -> Track {
        Track {
            id,
            ..Default::default()
        }
    }
    #[test]
    fn queue_navigation_never_wraps_or_loses_current() {
        let mut q = Queue::default();
        q.replace(vec![track(1), track(2)], 0).unwrap();
        assert_eq!(q.previous().unwrap().id, 1);
        assert_eq!(q.advance().unwrap().id, 2);
        assert!(q.advance().is_none());
        assert_eq!(q.selected().unwrap().id, 2);
    }
    #[test]
    fn invalid_replacement_preserves_queue() {
        let mut q = Queue::default();
        q.replace(vec![track(7)], 0).unwrap();
        assert!(q.replace(vec![], 0).is_err());
        assert_eq!(q.selected().unwrap().id, 7);
    }
}
