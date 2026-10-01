# Native boundary (ABI 4)

There is no localhost service, IPC or JSON boundary. C# SafeHandle pins the core during P/Invoke and defers close while calls are active. Rust panic guards turn internal failures into error snapshots. Invalid/dangling foreign pointers remain a caller contract, not something Rust can safely validate.

Coarse calls: open/close, folder children, paged folder tracks, add root/reconcile, play a folder starting at an ID, transport command, now playing, queue. A result is an owned snapshot containing contiguous sequential DTOs and UTF-8 string pointers. C# copies the rows/strings before releasing it in a finally block. Call snapshot_free exactly once; never free individual strings or rows. Numeric IDs, not paths, identify application entities.

64-bit layout: Folder DTO 56 bytes; Track DTO 80 bytes; Snapshot public header 112 bytes (ABI 4). Rust unit tests and the C# smoke check row sizes and Unicode strings. The private native allocation contains vectors after that header; foreign callers must never allocate or modify a snapshot. Header declarations are in player_ffi.h.

Playback is independent of navigation/scanning. FFI library reads use one locked read connection; reconciliation writes through a separate WAL connection. The audio handle runs on a separate worker. Concurrent scans are rejected. All expensive calls are dispatched off the UI thread.

Interoptopus was evaluated for the foundation. Manual DTO declarations keep this small API inspectable and avoid generator machinery; revisit generated bindings as the public API grows.

Playback flags: bit 0 mute; bits 1–2 repeat (0 off, 1 queue, 2 track). Selection accepts a bounded i64 ID array; all IDs resolve before audio/queue mutation. Commands 7 mute, 8 cycle repeat, 9 shuffle upcoming, 10 remove, 11/12 move, 13 clear upcoming, 14 jump within existing queue. Queue item indexes distinguish duplicate tracks.

Search and sorted folder APIs return existing owned snapshots. Sort is a validated enum; play_view constructs the whole folder queue in that same order. The UI never sends SQL.

ABI 3 appends a playlist pointer/count to the snapshot prefix (112 bytes total). Playlist DTO is 24 bytes (i64 ID, UTF-8 name, u64 count). Playlist commands: 0 create with optional selection, 1 rename, 2 soft-delete, 3 append, 4 remove occurrence, 5/6 move up/down, 7 restore.

Library notification calls retain ABI 3 layout: library_state returns revision/message and flags bit 0 for all configured roots watched; folder resolves an ID to its current path; refresh_queue merges current library metadata/paths by ID on the audio worker without replacing its source or position. Queue occurrence/order changes concurrent with refresh remain intact.

ABI 4 adds folder kind (u32 at offset 36), direct track count (u64 at offset 40) and requested override (u32 at offset 48), extending Folder DTO to 56 bytes. Kinds: 0 Automatic request, 1 Album, 2 Artist, 3 LooseTracks, 4 MixedCollection, 5 Compilation, 6 UserCollection. folder_kind updates only the SQLite presentation override and returns the decorated folder. Unknown values or absent IDs return an owned error snapshot. Cache schema 6 and ABI 4 are separate version numbers.
