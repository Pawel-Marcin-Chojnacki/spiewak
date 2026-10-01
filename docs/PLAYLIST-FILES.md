# Local playlist files and Explorer drops

Schema 10 / ABI 7. The DTO layouts remain Folder 64, Track 104 and Snapshot 112 bytes. Rust owns parsing, filesystem traversal, references, SQLite transactions and export writes; WinUI passes local paths from native pickers/drag-and-drop.

## User behavior

- Import beside Playlists opens UTF-8 `.m3u8` or `.m3u` and creates a persistent playlist. Import does not start audio or replace the queue.
- Export in a playlist view/context menu writes UTF-8 `.m3u8` with relative paths where the destination and track share a Windows drive/UNC prefix; otherwise full paths. Select a **new filename**: existing files are never overwritten, including after a picker overwrite confirmation.
- Drop local audio files, folders or M3U8 on the central browser or queue to append occurrences. Playback stays stopped/paused/playing as it was. Folder drops recurse in case-insensitive lexical path order; input containers, playlist order and duplicates remain ordered. This is not natural track-number sorting.
- Drop folders on the folder tree to explicitly add library roots. Files/playlists there report that the browser/queue is the correct target. This does not move files into the selected tree node.
- Save queue creates a playlist from the complete queue, including unavailable snapshots and repeated occurrences.

An explicitly referenced audio file outside library roots gets a stable positive track ID with no folder membership. It remains searchable, favoritable and playable without scanning its neighbors or creating a synthetic album/folder. Adding its actual folder later promotes the same ID into the real hierarchy; existing favorites/playlist/session references stay valid.

## Format and storage

Strict UTF-8, optional BOM, LF/CRLF, optional EXTM3U header, PLAYLIST name and EXTINF labels/durations are supported. Relative paths resolve against the playlist's own directory. Absolute drive and UNC paths work. URLs/streams, Windows device paths, drive-relative/root-relative paths and unsupported audio extensions are rejected. Empty lists can be imported. No external service is contacted.

Parsing/expansion precedes atomic database mutation. Invalid input does not leave a partial playlist or change the queue. A missing referenced file remains an unavailable occurrence with its path/label/duration; it is not silently removed. An original deleted/replaced file is never automatically reattached to a different file at that path. Explicit reimport can create fresh references. Export sanitizes metadata line breaks and preserves path text; an unrepresentable path fails the whole export.

Exports use a newly created sibling temporary file, flush it, and publish without replacement (MoveFileExW on Windows). Failure removes only the temporary file owned by that operation. Music source files remain read-only. Artwork extraction uses the existing bounded application cache.

Migration 9→10 preserves stable IDs, track indexes/FTS and foreign-key/snapshot triggers while making folder membership nullable. Playlist snapshots also retain duration/codec. Failure rolls back the migration; retain the database for recovery. It contains user playlists, favorites, history, presentation and playback state.

## Bounds and supported limits

- Playlist input/output: 8 MiB; each input line/local path: 64 KiB UTF-8; at most 100,000 occurrences per import/drop/save.
- Folder drop: at most 1,000,000 visited/pending entries; links and Windows reparse points are skipped. Unsupported nested non-audio files are ignored; inaccessible paths abort the drop.
- External references are checked by explicit rescan and the periodic reconciliation safety net. Their parent directory is not added to the watcher. Offline parents preserve cached entries. An external rename outside registered roots is not tracked by searching neighboring directories; explicit reimport is required. Root-based rename continuity remains the documented same-root/volume NTFS behavior.
- Raw `.m3u` in legacy encodings requires conversion to UTF-8. Network shares/offline reference recovery, large/drop responsiveness and simultaneous app instances are not certified.
- Explorer associations/context menus and dragging items out of the player are future work. No file-association registry changes are made by this milestone.

## Evidence and open acceptance

The Rust suite has 37 tests, including Unicode/BOM/CRLF/duplicates/missing files, external references without neighbors, promotion preserving favorites, replacement identity, relative Windows paths, import rollback, export without overwrite, source bytes and migration/snapshot/FTS preservation. Native C# checks exercise UTF-8 path-array ownership across ABI 7, complete queue snapshots and import/export round trips. WinUI ARM64 Release and WASAPI playback checks pass, including unregistered external files, pause preservation and visible missing-source errors; six-codec, session and artwork regression checks also pass. Actual UI gates remain open as recorded in STATUS.md.

The WinUI uses [Windows App SDK path-based pickers](https://learn.microsoft.com/en-us/windows/apps/develop/files/pickers-save-file) and [native storage-item drag/drop](https://learn.microsoft.com/en-us/windows/apps/develop/data/drag-and-drop). Passing native APIs does not prove actual picker selection, Explorer gestures, focus or high-DPI layout. Those end-user gates remain open while Computer Use guest input cannot produce a verified state change.
