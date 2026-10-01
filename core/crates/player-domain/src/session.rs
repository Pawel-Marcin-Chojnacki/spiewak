use crate::Track;
use std::sync::Arc;
#[derive(Clone)]
pub struct Session {
    pub tracks: Arc<[Track]>,
    pub current: Option<usize>,
    pub next_end: Option<usize>,
    pub position: f64,
    pub volume: f32,
    pub flags: u32,
}
impl Default for Session {
    fn default() -> Self {
        Self {
            tracks: Arc::from([]),
            current: None,
            next_end: None,
            position: 0.0,
            volume: 0.75,
            flags: 0,
        }
    }
}
impl Session {
    pub fn validate(&self) -> Result<(), String> {
        if !self.position.is_finite()
            || self.position < 0.0
            || !self.volume.is_finite()
            || !(0.0..=1.0).contains(&self.volume)
            || self.flags & !7 != 0
            || self.flags >> 1 > 2
            || self.current.is_some_and(|i| i >= self.tracks.len())
            || self.next_end.is_some_and(|end| {
                end <= self.current.map(|i| i + 1).unwrap_or(0) || end > self.tracks.len()
            })
            || self.current.is_none() && self.position != 0.0
        {
            return Err("Saved playback session is invalid. Keep the database for recovery; do not delete your music.".into());
        }
        Ok(())
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn session_rejects_invalid_cursor_and_nonfinite_settings() {
        let mut s = Session {
            tracks: vec![Track::default()].into(),
            current: Some(1),
            ..Default::default()
        };
        assert!(s.validate().is_err());
        s.current = Some(0);
        s.position = f64::NAN;
        assert!(s.validate().is_err());
        s.position = 1.0;
        s.flags = 6;
        assert!(s.validate().is_err());
        s.flags = 5;
        assert!(s.validate().is_ok());
    }
}
