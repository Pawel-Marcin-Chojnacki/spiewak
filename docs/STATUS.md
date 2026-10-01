# Verified status — player milestones

As of 2026-10-01 on Windows 11 ARM64 (Parallels). This is a working foundation/first playback milestone, not the complete product roadmap.

## Implemented

- Native WinUI 3 / .NET 10 process loading Rust through typed C ABI.
- Add library roots; background read-only scanning; SQLite WAL library index and persistent app state with schema version check.
- Real folder hierarchy, lazy tree children, center subfolder navigation, paged/virtualized track list.
- Lofty title/artist/album/album-artist/number/year/duration; filename fallback; embedded JPEG/PNG plus folder covers, bounded content-addressed thumbnails and per-track isolation for loose folders.
- Streaming Rust audio on a dedicated worker; folder queue, play/pause/resume/stop/seek/next/previous/volume.
- Persistent transport, upcoming queue sidebar, path/album inspector, compact rows, hideable queue, Back, Enter/Space/Alt+Left, basic context menu.
- Local UTF-8 M3U8 import/export and file/folder/playlist drops to queue; external file references do not create synthetic folders or scan neighboring music.
- Favorites, first-indexed/recently-added view and successful-playback-start history; persistent app state and unavailable snapshots.
- Cached startup before background reconciliation; rescan external additions/renames/deletions without changing music.

## Verification

- Rust: thirty-seven unit/integration tests; fmt and clippy with warnings denied.
- Release WinUI build on ARM64; native shell visually inspected in actual VM screenshots.
- C# ↔ Rust smoke: Unicode paths, actual WAV metadata/tags, folder fidelity, IDs across rescan/restart, external rename/delete, SHA-256 music preservation.
- Codec integration: generated MP3, FLAC, AAC/M4A, ALAC, Ogg Vorbis and WAV fixtures all index, decode/play and seek successfully on Windows ARM64. Corrupt audio returns an error while previous audio/queue continue. FFmpeg 9.0.2 installed via winget for fixture generation only.
- Windows audio smoke: default WASAPI device opens; position advances; pause freezes it; resume/seek/volume/next/previous/stop/end-of-queue pass. Quiet generated signals only. This verifies decoding/output command flow, not subjective listening quality.
- Foundation GitHub Windows x64 CI passed: https://github.com/Pawel-Marcin-Chojnacki/spiewak/actions/runs/36770607755 . Current-feature CI and additional codec/UI checks are recorded below after completion.

## Current limits / next work

- No Opus decoder in pinned Rodio/Symphonia graph. Files are indexed but playback reports an error.
- Native watcher and stable IDs within one root/volume are implemented. Reconciliation still walks roots in batches; per-event incremental updates, cross-root/cross-volume identity and network performance are open. Conservative folder presentation is implemented; a derived album/artist catalog remains open.
- Embedded/folder artwork cache is implemented. Tag editing, Smart Inbox, provider integrations and Undo remain open. Source-music mutation is absent/disabled.
- No gapless guarantee, ReplayGain, output-device selector or global media keys. Mute is implemented.
- Mixed queues, selection append/play-next, upcoming edits/clear, queue jump, shuffle upcoming, repeat off/queue/track and mute are implemented. Restart restores queue occurrences, cursor/position, volume and repeat/mute paused.
- Tree expansion/layout/density are not persisted; no resizable left panel. Forward navigation is implemented.
- Native extended selection exists; batch Play/Play next/Add to queue are implemented; playlist actions are implemented; tag editing remains open. Full accessibility/high contrast/high-DPI/multi-monitor review and 1k/10k/100k benchmarks remain open.
- Unpackaged self-contained developer build; proper installer/update/diagnostic export is later work.

## Reproduction

Single checkout: `C:\Users\pawel\source\repos\spiewak`. Do not create a parallel macOS checkout.

```powershell
.\tools\build.ps1
.\tools\run.ps1
dotnet run --project tests/Core.Smoke/Core.Smoke.csproj -c Release -- "$PWD\core\target\aarch64-pc-windows-msvc\release\spiewak_core.dll" --audio
```

The supplied plan is preserved verbatim in master-plan.md. Start the next feature from the pushed current feature branch, not master. Never merge automatically.

## UI test access

Computer Use can inspect the actual Parallels screenshot and native macOS menus, but guest clicks/keys were not delivered in this session. Full picker/tree/double-click testing remains open: Computer Use guest clicks and keyboard attempts have not produced a verifiable state change. No guest UI workflow is claimed verified solely from passing core tests.

## Player controls milestone

- Eight Rust tests plus C# FFI queue edits, multi-selection and Windows audio command checks.
- Interim semantic theme in windows/Player/Design/Theme.xaml. Native controls retain focus/accessibility behavior.
- `tools/run.ps1 -Demo` opens generated samples from Music/Spiewak Demo using a separate SpiewakDemo cache. Sample files are generated by tools/generate-demo.py; they are not bundled copyrighted music.

Actual dark sample-library shell was visually inspected in Parallels on 2026-10-01. Guest coordinate clicks still did not yield a verifiable UI state change; full end-user input workflows remain open. Sample inspection exposed AppleDouble sidecars being treated as audio; scanner now ignores these metadata files, with a regression fixture.

## Search milestone

SQLite schema 2 adds Unicode FTS5 indexes for title/artist/album/path and folder name/path. Safe prefix queries return at most 200 tracks and 30 folders. Search-as-you-type uses a 220 ms debounce and discards obsolete UI results. Ctrl+F, Escape and Down support keyboard search. Six native folder sort modes keep paging and folder playback order consistent. Forward history uses Alt+Right. Nine Rust tests and C# ABI search/sort/rename checks pass. Artists/albums grouped result sections and advanced filters remain later work.

## Playlist milestone

SQLite schema 3 stores ordered playlists, including duplicate occurrences. Create/save queue is atomic; append resolves all selected IDs before mutation. Rename, remove/reorder items, soft-delete and restore playlist are connected to native UI. Missing files retain snapshot labels/paths as unavailable entries. Playlist playback uses occurrence index, not track ID. Ten Rust tests plus C# create/append/reorder/delete/restore/restart/missing-file checks pass. UI list rows have reference identity so duplicate occurrences are selected independently. M3U import/export is implemented in the later playlist-files milestone; smart playlists remain open.

## Product research

Source-linked research of ten player references, praise and friction from public user discussions, implementation priorities and acceptance scenarios is in research/music-players-2026-10-01.md. Household requirements are confirmed in household-requirements.md; automatic artwork, metadata and download cleanup are P1 priorities. Final UI/UX work should preserve the semantic theme boundary and validate these actual workflows.

## Library synchronization milestone

SQLite schema 4 stores Windows volume/file ID plus creation time for tracks and directories. Rename/move within a single root on the same volume keeps IDs, playlist references and folder ownership. Schema 5 prevents reuse of deleted track/folder IDs, so an old runtime queue cannot silently resolve to an unrelated new song. Hard links remain separate entries; ambiguous matches and occupied destinations are not merged. Unsupported identity APIs use path matching. Root renames, moves across roots/volumes and delete/recreate identity are not certified.

Rust owns a debounced notify 8.2.0 native watcher (750 ms quiet period, 8 s burst cap), maintenance of root watches, a 60 s reconciliation safety net and 30 s retry for failed watches. Notification overflow requests a scan. Missing roots preserve the cached index; restored roots are watched again. The watcher batches root walks and skips unchanged tag parsing; it is not the plan's final fully incremental scanner. Unchanged scans do not refresh the view. WinUI polls a revision and refreshes the active folder/search/playlist plus queue metadata; the playing audio source is kept open. Tree expansion and loaded pages can reset during a refresh; scroll-position persistence remains open.

Twelve Rust tests, fmt/clippy with warnings denied, ARM64 WinUI Release and C# ABI tests pass. Real NTFS creation/deletion/file rename/album rename, hard-link separation, case-only rename, unavailable/restored roots and playlist last-known paths are checked. Windows audio verifies that refreshing a renamed upcoming file preserves playback position and that Next opens its new path. These are native filesystem/core checks; guest UI input workflows remain unverified.

Latest demo build launched successfully in the Windows console session. Additional visual inspection was unavailable because macOS was locked; unlocking it is required for further native UI input checks.

## Folder presentation / household milestone

Confirmed household requirements are recorded in household-requirements.md and linked from AGENTS.md. The owner wants a very simple player respecting directories, quiet presentation of unrelated singles, automatic artwork/tag discovery and cleanup of downloaded MP3s. Audio editing remains unconfirmed. Provider integrations and reversible file mutation are later implementation, not claimed ready.

Schema 6 persists folder presentation overrides. Automatic modes are conservative: Album only for at least two tracks with shared nonempty album/album artist (artist fallback), unique positive track numbers and no child directories; one tagged file remains LooseTracks. Tracks plus subfolders are MixedCollection; empty/container folders are UserCollection. Manual choices additionally include Artist and Compilation; they describe the same existing folder and never create or reparent folders. Override survives rescans, restart and same-root/volume directory rename. Actual album completeness is not certified by this heuristic. Ignore and automatic artist/compilation inference remain open.

ABI 4 extends Folder DTO to 56 bytes (kind, direct track count, override). Snapshot remains 112 bytes. Folder/search results are read in SQLite transactions so metadata decoration uses a coherent snapshot. WinUI displays folder type/count, adds a small presentation menu and hides the large album-art header for loose/mixed/container folders and search/playlists. A separate Home and its side section for singles remain later UI work.

Fifteen Rust tests, fmt/clippy with warnings denied, WinUI Release ARM64 and C# ABI tests verify coherent albums, a single tagged song, persistent overrides, folder rename and unchanged music bytes. The new visual/input state has not yet been inspected because the Mac was locked. Keep this distinct from successful compilation/core testing.

## Active research implementation goal

The complete research implementation is tracked in RESEARCH-IMPLEMENTATION.md. P0/P1/P2 decisions, household requirements, nine acceptance scenarios and UI/performance evidence remain in scope; existing narrow checks are not a completion claim.

Queue semantics milestone: successive Play next requests form a FIFO prefix before the original upcoming suffix. Natural/manual advancement consumes that prefix, repeat-one retains it, removing an occurrence adjusts its boundary. Explicit queue jump/Previous/manual reorder/shuffle starts a new request context; Clear and replacement clear the prefix. Duplicate occurrences remain distinct. Double-click/Enter plays only that selected track; explicit folder/playlist/results buttons build their indicated full queues. Sixteen Rust tests and Windows C# ABI/audio checks pass, including FIFO requests while playing. WinUI Release builds; full UI input verification remains pending. Session persistence is implemented in the milestone below.

## Playback session milestone

Schema 7 atomically stores the ordered queue, duplicate occurrences, current occurrence, Play next FIFO boundary, seek position, volume, mute and repeat. Library joins resolve the latest metadata/path by stable ID; deletion retains a last-known snapshot and an unavailable label. Separate missing items keep separate paths during refresh. Restoration is paused and lazy: neither a music file nor an output device is opened until explicit playback. Play on an upcoming-only queue starts its first occurrence. Stop resets position. A missing next item stops visibly at that occurrence; it is not silently skipped or resolved to an unrelated replacement file.

Rust publishes the complete audio state before acknowledging a command. Successful commands checkpoint app state; WinUI's background playback polling checkpoints elapsed position every five seconds, and normal close makes a final attempt. Queue rows are rewritten only when their Arc identity changes, not on every position update. Checkpoint failure is reported without cutting playback; startup reports the actual SQLite/session error. Keep the database for recovery: it contains playlists, presentation choices and session state, not just disposable metadata. Automatic recovery from a corrupt saved session and simultaneous multiple app instances remain open.

Twenty-one Rust tests, fmt, clippy with warnings denied, WinUI Release ARM64 and C# native tests pass. Tests cover schema 6→7 preserving playlist state, rollback after a simulated mid-queue SQL failure, mixed sources/duplicates, paused settings/cursor restart, preserved FIFO prefix, renamed paths, distinct unavailable entries, real WASAPI resume at the saved position and termination of a child process followed by recovery of the periodic checkpoint. Timing is nominal while playback polling is active and storage is writable; this is not a guarantee against a storage failure or OS suspension.

The latest demo app was launched in the Windows console session and visually inspected through Parallels. Compact folder presentation and the empty transport are visible. The Mac is currently accessible, but guest click and Ctrl+F/type attempts still produced no UI state change. Actual user input and restored transport/volume appearance remain unverified; core tests do not substitute for those gates.

## Local artwork milestone

Schema 8 stores separate embedded-cover references and effective track covers. ABI 5 extends Folder DTO to 64 bytes with its selected header artwork; Track remains 80 bytes and Snapshot 112 bytes. Album/Compilation can share a folder cover; loose/mixed/container folders keep per-track embedded images and a compact header. Manual presentation changes recompute this choice atomically. No music files are written.

The per-database cache accepts bounded JPEG/PNG sources, creates PNG thumbnails up to 512 pixels without enlarging smaller images, deduplicates identical source bytes and limits new storage to 256 MiB. Old generated orphans can be removed after 24 hours while library/session references and unknown files are protected. Cache write/quota/image errors do not mark the filesystem scan incomplete or prevent deleted music from being reconciled. Missing cache files regenerate. See ARTWORK.md for exact limits, retry and unsupported/corrupt-image behavior.

Twenty-seven Rust tests, fmt/clippy, WinUI ARM64 Release and C# native integration pass. Tests include ID3 artwork in WAV and MP3, folder versus single-track isolation through ABI 5, aspect ratio/no enlargement, cover refresh without cutting WASAPI playback, source SHA-256 preservation, storage failure recovery and migration 7→8 retaining session state. Online artwork/tag matching remains unimplemented. Actual bitmap/UI input and large-library performance remain open; the visible shell alone does not pass them.

## Listening library milestone

Schema 9 adds first-indexed times, favorites and history of successful new-source starts. Existing tracks receive a migration-time baseline; rescans/rename do not reset added time. Favorite and history references follow supported stable IDs and retain unavailable snapshots after deletion. Replacement files do not inherit those favorites. Favorite changes and history writes stay in the app database.

ABI 6 extends Track DTO to 104 bytes with favorite state, original reference ID and view timestamp; Folder remains 64 and Snapshot 112 bytes. WinUI has sidebar shortcuts, transport/selection hearts, Ctrl+D and 500-row pages. Explicit Play uses the full indicated view; double-click/Enter remains a single-track action. History refresh uses a coherent rows/revision snapshot.

The audio worker buffers successful starts independently of UI polling; native commands/polls commit and acknowledge them atomically. Pause/resume, seek, append and failed playback are excluded. Uncommitted starts can be lost on abrupt exit; the bounded buffer reports overflow, and database failures retain pending entries for retry while audio continues. See LISTENING.md for semantics and limits.

Thirty Rust tests, fmt/clippy, WinUI ARM64 Release and C# Windows native/audio checks pass, including natural advance, repeat/duplicate occurrences, unavailable favorite removal, rename/restart and music-byte preservation. The actual demo window shows the three sidebar shortcuts and empty-transport heart; sidebar/heart/keyboard interactions remain unverified through Parallels; larger-scale performance, history privacy controls and navigation/scroll polish remain open.

## Playlist files / Explorer integration milestone

Schema 10 allows explicit audio references outside library roots without creating folders or scanning neighboring files. UTF-8 M3U8/M3U import preserves order, repeated occurrences and unavailable labels/durations. Export writes new M3U8 files atomically, uses relative paths where possible and never overwrites existing destinations. Adding a real folder promotes the same external track ID; a replacement NTFS file does not inherit an old reference or favorite. Complete-queue saving retains unavailable snapshots.

ABI 7 adds import/export, bounded UTF-8 path arrays and save-queue calls while retaining Folder/Track/Snapshot sizes 64/104/112. Native WinUI path pickers, Import/Export controls, browser/queue storage-item drops and folder-tree root drops are connected. See PLAYLIST-FILES.md for traversal/format bounds and external-rename limits.

Thirty-seven Rust tests, fmt, clippy with warnings denied, WinUI ARM64 Release and C# native integration pass. Windows WASAPI checks cover unregistered files, import/drop without autoplay, paused-current preservation, unavailable occurrence errors, relative Unicode round trips, queue snapshots, promotion preserving favorites, restart, unchanged music bytes, six codecs and existing session/artwork behavior.

The current demo window launches and shows Import beside Playlists, compact real-folder navigation and a stopped transport. The Import button hover is visible, but its Computer Use click still did not open a picker. Actual picker selection, Explorer drag gestures, export dialogs, high-DPI/input and household acceptance remain open. Native tests are not substituted for these gates. No file associations/context-menu registry entries are installed.

Git identity diagnostic: playlist-files commit 015d64d was pushed with author name `unknown` and the existing owner email after inheriting an overlong corrupted name from earlier published commits failed. This affects author metadata, not source/test results. Subsequent commits use process-local ASCII `Pawel` with the same email; AGENTS.md documents the Windows encoding trap. Published history is retained without force-push or rewrite.
