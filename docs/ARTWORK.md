# Local artwork without reorganizing music

Implemented on Windows 11 ARM64, 2026-10-01. Indexing reads source files and writes thumbnails only beside the application database. This milestone does not download artwork or edit tags.

## Selection

- Album and Compilation folders use an explicit folder cover when available. Each track retains its separate embedded-cover reference so changing the presentation back to Loose tracks restores individual images.
- Without a folder cover, tracks keep their own embedded images. The Album/Compilation header uses the first available image by track number, title and ID.
- Loose tracks, Mixed folder, Artist folder and My collection have no shared album header. Individual tracks use their own embedded image or the neutral placeholder. A picture or album tag on one file never creates a new folder or album.
- Embedded pictures are inspected across tags: front covers first, then other decodable pictures. A malformed front picture can fall back to a valid other picture.
- Folder names are matched case-insensitively, in priority order: cover.jpg, folder.jpg, front.jpg, cover.png, folder.png, front.png, cover.jpeg, folder.jpeg, front.jpeg. Symlinks and Windows reparse-point files are excluded. Source path, byte size and modification time identify an unchanged cover.

## Cache and limits

For library.db the directory is library.artwork, in the same application-data directory. Originals stay untouched. JPEG/PNG inputs up to 8 MiB and dimensions up to 4096 × 4096 are supported. Other image formats use the placeholder. Thumbnails are PNG, with longest edge at most 512 pixels and preserved aspect ratio; small images are not enlarged.

The image decoder receives a 64 MiB allocation guard, which is best-effort, not a hard process-memory ceiling. Lofty receives an 8 MiB single-item allocation limit and does not retain format-specific duplicate tag data. These limits do not guarantee an aggregate process memory bound. Primary dependency references: [image limits](https://docs.rs/image/latest/image/struct.Limits.html), [image source](https://github.com/image-rs/image), [SHA-256 API](https://docs.rs/sha2/0.10.9/sha2/).

Original image bytes are hashed with SHA-256. Identical bytes share a v1-<hash>.png thumbnail; visually identical images with different encodings can have separate entries. A temporary file is flushed and published by a same-directory rename. Cache writers are serialized by the single application's scan lock; simultaneous app instances are not certified.

The cache permits at most 256 MiB of new thumbnail storage, counting existing regular files. Existing matching thumbnails remain usable when the limit is reached. New images then remain unavailable until space is freed, while indexing and playback continue. I/O failures produce scan warnings and are retried by reconciliation. A missing referenced thumbnail is rebuilt from its source. Corrupt nonempty cache files are not yet fully validated or repaired automatically.

Reconciliation removes only generated PNG/TMP orphans at least 24 hours old. Library and saved-session image references are protected. Unknown filenames, including Unicode filenames, are preserved. The cache subtree is excluded from music indexing/watch events.

The image directory is disposable; close the app before manually removing it. **Do not delete the database as a cache cleanup step:** it contains playlists, folder choices and the saved playback session. A user-facing cache-management action remains future work.

## Verification

Twenty-seven Rust tests pass with fmt and clippy warnings denied. WinUI ARM64 Release builds and the native C# integration suite passes. Artwork checks cover JPEG/PNG bounds, aspect ratio, no enlargement, content reuse, malformed pictures, front-cover fallback, quota and cache-write failures, missing-cache rebuild, folder override, unchanged rescans, same-volume rename, source-file byte preservation, old-orphan cleanup and schema 7→8 session preservation. Unicode filenames cannot cause a cleanup panic.

Native Windows tests read embedded ID3 pictures from WAV and MP3 fixtures. MP3 remains a single loose track and opens the WASAPI output successfully. Folder-cover refresh while playing preserves the running audio. Existing six-codec decode/seek, queue, playlist and restart/crash-checkpoint regressions also pass. Test fixture writers are confined to temporary test music.

The actual custom shell launches and is visible in Parallels. Guest Computer Use clicks/keys have not produced a verified state change, so bitmap appearance, album/loose navigation and end-user input remain acceptance gates. Embedded artwork in all other supported audio containers and 1k/10k/100k cache performance are not yet certified. Online providers, metadata matching, Home presentation and reversible tag/name edits remain separate requirements.
