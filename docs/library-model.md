# Library model

The collection is the filesystem. SQLite is a rebuildable read cache in `%LOCALAPPDATA%/Spiewak/library.db`. Music is never copied, renamed, retagged or deleted by this version.

Schema version 1 has roots, parent-linked folders and tracks. Every real directory is represented, including folders with no directly contained audio. Tags enrich rows; they never create/reparent directories or artists. Overlapping roots are rejected to keep folder ownership unambiguous. Symlinks and Windows reparse points below a root are skipped to prevent cycles and unexpected traversal.

IDs persist across rescans and restarts. Schema 4 recognizes Windows file/directory identities during renames or moves within one root and volume. Unsupported filesystems fall back to paths; cross-root/volume moves are not certified. Unavailable roots retain cached data. Any read error suppresses deletion of stale entries for the affected root.

A background reconciliation has its own WAL connection and savepoint. Readers see the previous complete index until commit. Unchanged size/mtime skips tag parsing. Failed SQL rolls the scan back. Schema versions newer than the application are refused.

Folder queries load children lazily. Track queries are ordered by track number, title, then ID, with 500-row pages in the UI. The queue uses the full selected folder, including tracks not yet paged into the UI. Million-folder trees and 100,000-track performance are not yet certified.

Schema 2 migrates existing caches atomically to external-content SQLite FTS5. Triggers maintain search data on insert/delete/tag/path changes, without reindexing each unchanged scan epoch. Queries are parameterized, tokenized to quoted prefixes, and bounded. See https://sqlite.org/fts5.html for external-content trigger/rebuild semantics.

Schema 3 adds playlists and ordered playlist_items. Items refer to library IDs with ON DELETE SET NULL and retain path/title/artist/album snapshots, so unavailable files do not silently vanish. Reordering uses item occurrence positions and a transaction. Playlist deletion is a tombstone; restore never modifies music files.

Schema 4 uses read-only GetFileInformationByHandleEx/FileIdInfo (volume + 128-bit ID), combined with creation time to reduce file-ID reuse collisions. Only one vanished path is eligible for relocation; existing hard links and occupied destination rows are preserved separately. A BEFORE DELETE trigger saves the current track labels/path into playlist items before their reference becomes NULL.

Monitor owns a separate WAL connection and native notify watcher. It batches events after 750 ms quiet time, caps bursts at 8 s, refreshes root watches every 2 s, retries failed watches every 30 s and reconciles every 60 s as a safety net. Manual and automatic scans share one atomic scan lock. Own database and AppleDouble notifications are ignored; overflow flags/errors reconcile. This version walks roots after a batch, so large/network libraries need dedicated measurements and future incremental optimization.

Sources: [Windows identity API](https://learn.microsoft.com/en-us/windows/win32/api/winbase/nf-winbase-getfileinformationbyhandleex), [FILE_ID_INFO](https://learn.microsoft.com/en-us/windows/win32/api/winbase/ns-winbase-file_id_info), [notify limitations](https://docs.rs/notify/8.2.0/notify/), [event overflow](https://docs.rs/notify/8.2.0/notify/struct.Event.html).

Schema 5 adds monotonic entity ID allocation inside scan transactions. SQLite default row-ID reuse after deletion must not make a runtime queue point to a different song. Existing IDs remain unchanged during migration.

Schema 6 persists a folder's requested presentation separately from metadata. Auto derives Album/LooseTracks/MixedCollection/UserCollection conservatively, using shared album/owner and unique positive track numbers. Manual Artist/Compilation/Album/Loose/Mixed/Collection overrides only affect app presentation; rescan and file-ID relocation retain them. There are no virtual album folders created from tags. A one-track tagged folder defaults to LooseTracks; album completeness cannot be inferred solely from this heuristic.
