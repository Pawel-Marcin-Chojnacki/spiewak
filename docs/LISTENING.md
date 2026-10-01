# Favorites, recently added and listening history

Implemented on Windows 11 ARM64, 2026-10-01. These are views over real library files. They never create albums, move music, or write tags.

## Everyday actions

The sidebar offers Favorites, Recently added and Listening history. The transport has a heart for its current track. Selected rows have an inspector button and context action; Ctrl+D toggles the selection. If all selected rows are favorite, the action removes them; otherwise it adds the whole selection. Favoriting an already-favorite file is idempotent and does not change its favorite date.

All three views show newest entries first and load 500 rows per page. Double-click/Enter plays only the selected track; the explicit Play view button queues the full current view (up to 100,000 entries). History duplicates represent separate starts and remain separate row objects. The inspector shows the local date/time. Searching from a listening view temporarily shows library-wide results; clearing search restores the source view. Search playback uses only its displayed results.

## Data and semantics

Schema 9 adds a first-indexed timestamp, favorites and listening-history snapshots. Existing schema-8 tracks receive a migration-time baseline because their original import time was not recorded. Newly indexed files use their first scan time, not filesystem creation/modification time. Rescans, tag changes and supported renames preserve that timestamp. Ties use stable ID order.

Favorites and history retain stable original IDs and last-known metadata after deletion, displaying Unavailable rather than resolving to a different replacement file. A missing favorite can be removed. Same-root/volume NTFS rename keeps references; cross-volume/root and other filesystem limits are unchanged. Unavailable items fail safely if played; there is no silent skip. Ordinary folder/search/playlist rows and the current transport expose favorite state. Application startup restores paused listening state without adding a history entry.

The audio worker emits an event when a new decoded source is successfully started, including manual selection, Next/jump, natural advancement, repeat and explicit restored playback. Pause/resume of an existing source, seek/Previous seeking to the beginning, volume, queue append, failed source opens and passive restoration do not create entries. This is a history of successful source starts, not proof of a completed listen or subjective audibility.

Events buffer outside the SQLite/audio output path. Commands, playback polling, listening-view reads and normal close attempt an atomic database batch. Only a successful commit acknowledges its event prefix; starts arriving during the write remain pending. Database errors retain the batch for retry and do not cut the audio. The buffer holds 1,000 starts; overflow drops additional starts and publishes a diagnostic with the lost count for the rest of that process lifetime, even after another command or recovery. Abrupt termination can lose uncommitted starts. Polling normally runs every 250 ms; storage failure, suspension and simultaneous app instances are not certified delivery conditions. History is stored locally and has no completion threshold, automatic retention cutoff or cloud submission. A history-clear/privacy setting remains future product work.

History revision and rows are read in one SQLite transaction so UI polling cannot pair older rows with a newer revision. The open history view refreshes when committed starts change that revision; this can reset loaded pages/selection. Navigation/scroll refinement and full keyboard, screen-reader and varied-scale acceptance remain open. Artwork pruning protects favorites/history snapshots as well as library/session references.

ABI 6 extends Track DTO from 80 to 104 bytes with favorite state, original reference ID and the view timestamp. Folder remains 64 bytes and Snapshot 112 bytes. Old binaries must be rebuilt together.

## Verification

Thirty Rust tests pass, including schema 8→9 preserving playlist/session state, atomic failure/retry behavior, event-prefix acknowledgement, bounded buffer with persistent overflow warning, reference continuity, deletion/replacement isolation and unchanged source bytes. fmt/clippy with warnings denied and WinUI ARM64 Release pass.

Native C# tests cover favorite idempotence and folder/search/playlist/transport state, pagination, first-indexed timestamps, NTFS rename/restart, unavailable snapshots/removal and SHA-256 source preservation. Windows WASAPI tests verify successful starts, duplicate occurrences, natural advancement between UI polls, repeat-one, failed-source exclusion and no duplicate from pause/resume/seek/volume. Existing six-codec, queue, session/crash-checkpoint and artwork regressions also pass.

The actual demo shell launches in Parallels. The three sidebar shortcuts and the disabled heart in the empty transport were visually inspected in the actual window. However, Computer Use guest clicks/keys have not produced verified state changes. The complete new UI workflow is therefore not yet accepted. No performance claim for 1k/10k/100k libraries is made.
