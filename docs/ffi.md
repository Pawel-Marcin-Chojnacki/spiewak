# Native boundary (ABI 1)

There is no localhost service, IPC or JSON boundary. C# SafeHandle pins the core during P/Invoke and defers close while calls are active. Rust panic guards turn internal failures into error snapshots. Invalid/dangling foreign pointers remain a caller contract, not something Rust can safely validate.

Coarse calls: open/close, folder children, paged folder tracks, add root/reconcile, play a folder starting at an ID, transport command, now playing, queue. A result is an owned snapshot containing contiguous sequential DTOs and UTF-8 string pointers. C# copies the rows/strings before releasing it in a finally block. Call snapshot_free exactly once; never free individual strings or rows. Numeric IDs, not paths, identify application entities.

64-bit layout: Folder DTO 40 bytes; Track DTO 80 bytes; Snapshot public header 88 bytes. Rust unit tests and the C# smoke check row sizes and Unicode strings. The private native allocation contains vectors after that header; foreign callers must never allocate or modify a snapshot. Header declarations are in player_ffi.h.

Playback is independent of navigation/scanning. FFI library reads use one locked read connection; reconciliation writes through a separate WAL connection. The audio handle runs on a separate worker. Concurrent scans are rejected. All expensive calls are dispatched off the UI thread.

Interoptopus was evaluated for the foundation. Manual DTO declarations keep this small API inspectable and avoid generator machinery; revisit generated bindings as the public API grows.
