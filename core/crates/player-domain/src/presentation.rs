//! Presentation describes a real directory; metadata never creates a directory.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[repr(u32)]
pub enum FolderKind {
    #[default]
    Auto = 0,
    Album = 1,
    Artist = 2,
    LooseTracks = 3,
    MixedCollection = 4,
    Compilation = 5,
    UserCollection = 6,
}
pub struct FolderEvidence {
    pub track_count: u64,
    pub has_children: bool,
    pub shared_album: bool,
    pub shared_artist: bool,
    pub unique_numbers: bool,
}
impl FolderKind {
    pub fn from_value(value: u32) -> Result<Self, String> {
        match value {
            0 => Ok(Self::Auto),
            1 => Ok(Self::Album),
            2 => Ok(Self::Artist),
            3 => Ok(Self::LooseTracks),
            4 => Ok(Self::MixedCollection),
            5 => Ok(Self::Compilation),
            6 => Ok(Self::UserCollection),
            _ => Err("Unknown folder presentation.".into()),
        }
    }
    pub fn infer(e: &FolderEvidence) -> Self {
        if e.has_children || e.track_count == 0 {
            if e.track_count > 0 {
                Self::MixedCollection
            } else {
                Self::UserCollection
            }
        } else if e.track_count >= 2 && e.shared_album && e.shared_artist && e.unique_numbers {
            Self::Album
        } else {
            Self::LooseTracks
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn single_tagged_song_and_uncertain_metadata_do_not_become_albums() {
        let mut e = FolderEvidence {
            track_count: 1,
            has_children: false,
            shared_album: true,
            shared_artist: true,
            unique_numbers: true,
        };
        assert_eq!(FolderKind::infer(&e), FolderKind::LooseTracks);
        e.track_count = 2;
        assert_eq!(FolderKind::infer(&e), FolderKind::Album);
        e.shared_artist = false;
        assert_eq!(FolderKind::infer(&e), FolderKind::LooseTracks);
        e.shared_artist = true;
        e.unique_numbers = false;
        assert_eq!(FolderKind::infer(&e), FolderKind::LooseTracks);
        e.unique_numbers = true;
        e.shared_album = false;
        assert_eq!(FolderKind::infer(&e), FolderKind::LooseTracks);
    }
    #[test]
    fn directory_mixed_with_subfolders_keeps_its_folder_presentation() {
        let e = FolderEvidence {
            track_count: 12,
            has_children: true,
            shared_album: true,
            shared_artist: true,
            unique_numbers: true,
        };
        assert_eq!(FolderKind::infer(&e), FolderKind::MixedCollection);
        assert!(FolderKind::from_value(7).is_err());
    }
}
