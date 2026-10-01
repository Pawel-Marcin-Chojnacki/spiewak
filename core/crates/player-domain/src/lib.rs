//! Filesystem identities and playback queue rules, independent of UI and storage.
mod presentation;
mod session;
pub use presentation::{FolderEvidence, FolderKind};
pub use session::Session;
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
    pub kind: FolderKind,
    pub override_kind: FolderKind,
    pub track_count: u64,
}

#[derive(Clone, Default)]
pub struct Queue {
    pub tracks: std::sync::Arc<[Track]>,
    pub current: Option<usize>,
    next_end: Option<usize>,
}
impl Queue {
    pub fn replace(&mut self, tracks: Vec<Track>, start: usize) -> Result<(), String> {
        if tracks.is_empty() || start >= tracks.len() {
            return Err("Choose a playable track first.".into());
        }
        self.tracks = tracks.into();
        self.current = Some(start);
        self.next_end = None;
        Ok(())
    }
    pub fn restore(&mut self, session: &Session) -> Result<(), String> {
        session.validate()?;
        self.tracks = session.tracks.clone();
        self.current = session.current;
        self.next_end = session.next_end;
        Ok(())
    }
    /// Manual Next ignores repeat-one; automatic completion honors it.
    pub fn advance(&mut self, natural: bool, repeat: u32) -> Option<Track> {
        let current = self.current?;
        let next = if natural && repeat == 2 {
            current
        } else if current + 1 < self.tracks.len() {
            current + 1
        } else if repeat == 1 {
            0
        } else {
            return None;
        };
        self.current = Some(next);
        if self.next_end.is_some_and(|end| next + 1 >= end) {
            self.next_end = None;
        }
        self.tracks.get(next).cloned()
    }
    pub fn jump(&mut self, index: usize) -> Result<(), String> {
        if index >= self.tracks.len() {
            return Err("This queue item no longer exists.".into());
        }
        self.current = Some(index);
        self.next_end = None;
        Ok(())
    }
    /// Successive Play next requests form a FIFO prefix ahead of the original suffix.
    pub fn append(&mut self, tracks: Vec<Track>, next: bool) {
        if tracks.is_empty() {
            return;
        }
        let count = tracks.len();
        let start = self.current.map(|i| i + 1).unwrap_or(0);
        let index = if next {
            self.next_end
                .unwrap_or(start)
                .max(start)
                .min(self.tracks.len())
        } else {
            self.tracks.len()
        };
        let mut rows = self.tracks.to_vec();
        rows.splice(index..index, tracks);
        self.tracks = rows.into();
        if next {
            self.next_end = Some(index + count);
        }
    }
    pub fn priority_end(&self) -> Option<usize> {
        self.next_end
    }
    fn editable(&self, index: usize) -> Result<(), String> {
        if index >= self.tracks.len() || self.current.is_some_and(|i| index <= i) {
            return Err("Only upcoming queue items can be edited.".into());
        }
        Ok(())
    }
    pub fn remove(&mut self, index: usize) -> Result<(), String> {
        self.editable(index)?;
        let mut rows = self.tracks.to_vec();
        rows.remove(index);
        self.tracks = rows.into();
        if let Some(end) = self.next_end
            && index < end
        {
            let end = end - 1;
            self.next_end = (end > self.current.map(|i| i + 1).unwrap_or(0)).then_some(end);
        }
        Ok(())
    }
    pub fn move_item(&mut self, index: usize, up: bool) -> Result<(), String> {
        self.editable(index)?;
        let target = if up {
            index.checked_sub(1)
        } else {
            index.checked_add(1)
        }
        .ok_or("Cannot move this item further.")?;
        self.editable(target)?;
        let mut rows = self.tracks.to_vec();
        rows.swap(index, target);
        self.tracks = rows.into();
        self.next_end = None;
        Ok(())
    }
    pub fn clear_upcoming(&mut self) {
        self.next_end = None;
        self.tracks = self.tracks[..self.current.map(|i| i + 1).unwrap_or(0)]
            .to_vec()
            .into();
    }
    /// Shuffle only the unplayed suffix, preserving the current source and history.
    pub fn shuffle(&mut self, mut seed: u64) {
        self.next_end = None;
        seed = seed.max(1);
        let start = self.current.map(|i| i + 1).unwrap_or(0);
        let rows = std::sync::Arc::make_mut(&mut self.tracks);
        for i in (start + 1..rows.len()).rev() {
            seed ^= seed << 13;
            seed ^= seed >> 7;
            seed ^= seed << 17;
            let j = start + seed as usize % (i - start + 1);
            rows.swap(i, j);
        }
    }
    pub fn previous(&mut self) -> Option<Track> {
        let previous = self.current?.saturating_sub(1);
        self.current = Some(previous);
        self.next_end = None;
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
        assert_eq!(q.advance(false, 0).unwrap().id, 2);
        assert!(q.advance(false, 0).is_none());
        assert_eq!(q.selected().unwrap().id, 2);
    }
    #[test]
    fn invalid_replacement_preserves_queue() {
        let mut q = Queue::default();
        q.replace(vec![track(7)], 0).unwrap();
        assert!(q.replace(vec![], 0).is_err());
        assert_eq!(q.selected().unwrap().id, 7);
    }
    #[test]
    fn mixed_queue_edits_preserve_current_and_duplicates() {
        let mut q = Queue::default();
        q.replace(vec![track(1), track(2), track(3)], 0).unwrap();
        q.append(vec![track(9), track(9)], true);
        assert_eq!(
            q.tracks.iter().map(|t| t.id).collect::<Vec<_>>(),
            vec![1, 9, 9, 2, 3]
        );
        q.move_item(4, true).unwrap();
        q.remove(1).unwrap();
        assert_eq!(
            q.tracks.iter().map(|t| t.id).collect::<Vec<_>>(),
            vec![1, 9, 3, 2]
        );
        assert!(q.remove(0).is_err());
        q.shuffle(42);
        assert_eq!(q.selected().unwrap().id, 1);
        let mut ids = q.tracks.iter().map(|t| t.id).collect::<Vec<_>>();
        ids.sort();
        assert_eq!(ids, vec![1, 2, 3, 9]);
        q.clear_upcoming();
        assert_eq!(q.tracks.len(), 1);
        assert_eq!(q.selected().unwrap().id, 1);
    }
    #[test]
    fn successive_play_next_requests_keep_fifo_order_across_advance_and_removal() {
        let mut q = Queue::default();
        q.replace(vec![track(1), track(2)], 0).unwrap();
        q.append(vec![track(10), track(10)], true);
        q.append(vec![track(20)], true);
        assert_eq!(
            q.tracks.iter().map(|t| t.id).collect::<Vec<_>>(),
            vec![1, 10, 10, 20, 2]
        );
        q.advance(false, 0);
        q.remove(2).unwrap();
        q.append(vec![track(30)], true);
        assert_eq!(
            q.tracks.iter().map(|t| t.id).collect::<Vec<_>>(),
            vec![1, 10, 20, 30, 2]
        );
        assert_eq!(q.advance(true, 2).unwrap().id, 10); // Repeat one does not consume pending requests.
        assert_eq!(q.advance(false, 0).unwrap().id, 20);
        assert_eq!(q.advance(false, 0).unwrap().id, 30);
        q.append(vec![track(40)], true);
        assert_eq!(q.advance(false, 0).unwrap().id, 40);
        assert_eq!(q.advance(false, 0).unwrap().id, 2);
        q.append(vec![track(50)], true);
        q.shuffle(42);
        q.append(vec![track(60)], true);
        assert_eq!(q.tracks[q.current.unwrap() + 1].id, 60);
    }
    #[test]
    fn repeat_modes_distinguish_natural_end_from_next() {
        let mut q = Queue::default();
        q.replace(vec![track(1), track(2)], 0).unwrap();
        assert_eq!(q.advance(true, 2).unwrap().id, 1);
        assert_eq!(q.advance(false, 2).unwrap().id, 2);
        assert!(q.advance(false, 2).is_none());
        assert_eq!(q.advance(true, 1).unwrap().id, 1);
    }
}

#[derive(Clone, Debug)]
pub struct Playlist {
    pub id: i64,
    pub name: String,
    pub count: u64,
}
