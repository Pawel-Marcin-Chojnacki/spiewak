# Library model

The collection is the filesystem. SQLite is a rebuildable read cache in `%LOCALAPPDATA%/Spiewak/library.db`. Music is never copied, renamed, retagged or deleted by this version.

Schema version 1 has roots, parent-linked folders and tracks. Every real directory is represented, including folders with no directly contained audio. Tags enrich rows; they never create/reparent directories or artists. Overlapping roots are rejected to keep folder ownership unambiguous. Symlinks and Windows reparse points below a root are skipped to prevent cycles and unexpected traversal.

IDs persist across rescans and restarts while paths remain unchanged. Recognizing file identity across moves/renames is a later synchronization milestone; today a rescan replaces moved entries. Unavailable roots retain cached data. Any read error suppresses deletion of stale entries for the affected root.

A background reconciliation has its own WAL connection and savepoint. Readers see the previous complete index until commit. Unchanged size/mtime skips tag parsing. Failed SQL rolls the scan back. Schema versions newer than the application are refused.

Folder queries load children lazily. Track queries are ordered by track number, title, then ID, with 500-row pages in the UI. The queue uses the full selected folder, including tracks not yet paged into the UI. Million-folder trees and 100,000-track performance are not yet certified.

Schema 2 migrates existing caches atomically to external-content SQLite FTS5. Triggers maintain search data on insert/delete/tag/path changes, without reindexing each unchanged scan epoch. Queries are parameterized, tokenized to quoted prefixes, and bounded. See https://sqlite.org/fts5.html for external-content trigger/rebuild semantics.
