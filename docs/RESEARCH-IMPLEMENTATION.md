# Research implementation and completion evidence

Active objective: implement docs/research/music-players-2026-10-01.md at source revision a64f477 (feature/folder-presentation), with the confirmed household requirements. This ledger does not replace or narrow that objective. Competitor descriptions are evidence, not a request to copy every competing feature. The research's P0/P1/P2 decisions, acceptance scenarios and applicable household invariants remain the requirements.

## Requirements and gates

| ID | Requirement | Current evidence / missing gate |
|---|---|---|
| R01 | Real folders canonical; tags never change hierarchy; offline cache retained | Core/ABI filesystem tests pass. End-user folder-picker/input workflow and household review still required. |
| R02 | Predictable queue, explicit single/folder/playlist scope, duplicates, shuffle, repeated Play next | FIFO prefix and explicit UI scope implemented. Sixteen Rust tests plus native Windows audio FIFO checks pass; UI verification required. |
| R03 | Unicode search, selection, responsive results, no playback interruption | FTS/debounce/obsolete-result discard implemented. UI acceptance, responsiveness and larger datasets remain unmeasured. |
| R04 | Persistent playlists, visible missing entries, rename/move references | Core/ABI checks pass for same-root/volume NTFS. M3U interoperability is R11. Broader supported filesystem limits must remain explicit. |
| R05 | File synchronization, debounce, stable IDs | Native watcher plus periodic reconciliation implemented; IDs verified on NTFS, overflow covered. Fully incremental performance, large/network libraries and UI state preservation remain open. |
| R06 | Restore queue/order/volume/position without surprising autoplay | Core implemented: schema 7, duplicate/mixed queue occurrences, FIFO boundary, settings/cursor/position, paused/lazy restart and unavailable snapshots. Twenty-one Rust tests and native Windows restart/WASAPI/abrupt-termination checks pass. Actual restored UI/input gate remains open. |
| R07 | Favorites, recently added, history, one simple action | Not implemented. Store app state without rewriting tags or directories; verify rename/restart and navigation. |
| R08 | ReplayGain track/album, verified gapless, opt-in crossfade | Volume/mute exist. Gain handling, source pipeline and measured continuous output are missing. Decode/output safety and album order need regression. |
| R09 | Global media keys, output-device selection/loss | Not implemented. Verify outside the app and physical/default-device changes, with actionable errors. |
| R10 | Embedded artwork cache, bounded sizes, neutral placeholder, folder versus loose-track choice | Embedded/front-cover and folder-cover extraction, bounded SHA-256 thumbnail cache and isolated per-track choice implemented (schema 8, ABI 5). Twenty-seven Rust tests and native Windows WAV/MP3 artwork/audio checks pass. Provider downloads, actual bitmap/UI workflow and large-library performance remain open. |
| R11 | Explorer drag/drop, M3U8 import/export, relative paths, visible missing files | Not implemented. Unicode round-trip and user-facing import/drop workflow required; Explorer associations/context menu are later items in the research. |
| R12 | Manual tags, metadata discovery, safe Inbox, preview/apply/undo, configured providers | Not implemented. Confirmed household priority; preserve originals and significant version names. Test provider errors/matching, collision/concurrent edits, undo and actual files. Existing-library restructuring stays excluded. |
| R13 | Simple smart playlists with understandable rules and preview | Not implemented (P2). Verify live result changes, missing files and independence from physical collection. |
| R14 | Conservative album/singles presentation and override; household simplicity | Native domain/ABI tests pass. Home with quiet side section for loose tracks and actual household workflows remain open. |
| R15 | Native custom theme, keyboard/focus/screen reader/high contrast, large text/high-DPI | Semantic theme and native controls exist. Actual varied-scale/input/accessibility review missing; screenshots of empty shell alone cannot pass this gate. |
| R16 | Performance at 1k/10k/100k; no UI blocking during scan/error | Not benchmarked. Need reproducible seeded library, scan/search/cache/audio measurements and UI evidence. |
| R17 | Lyrics/sleep timer/A-B and multiple independent queues | Conditional research candidates, not confirmed household requirements. Keep visible in this audit; do not silently claim implemented or assume their necessity. |
| R18 | Preserve non-goals and maintain reviewable Git history | Native Rust/WinUI typed C ABI; no server, streaming, sonic AI, skins/plugins or Electron replacement. Separate stacked feature branches, checks and pushes; no default-branch merge. |

## Nine acceptance scenarios from the research

1. Add real folder, find Polish-named track and play without configuring tags: ABI covered; native input workflow pending.
2. Browse/search while playing without interruption: architecture separated; end-user workflow pending.
3. A then B Play next, shuffle and duplicates: FIFO native and Windows audio checks pass; UI gates remain open.
4. Save queue/restart/playlist, no autoplay: playlist and session restart plus abrupt termination covered natively; actual UI workflow remains open.
5. Explorer rename album/file updates library/playlist; offline root preserved: native filesystem/ABI covered; UI refresh/scroll acceptance pending.
6. Connected album tracks produce no unintended gap: missing source pipeline and measured evidence.
7. Corrupt file/device loss/scan do not freeze UI: corrupt file preserves audio in ABI tests; device and UI load evidence missing.
8. Normal scaling/large text keeps transport readable and keyboard accessible: manual/native UI verification missing.
9. Run confirmed household tasks and record problems before final UI polish: user supplied requirements; actual household review pending.

## Continuation

R02 queue semantics and R06 session continuity are implemented with native checks; their UI gates remain open. R10 local artwork is implemented with bounded cache and native checks; online providers, rendering/input and scale gates remain open. Continue R07/R11/R12 and audio/device/performance/UX gates. This order is execution planning, not a change to final scope. Keep the goal active until each required feature and its acceptance evidence is complete. Latest shell inspection was possible, but Computer Use guest clicks/keys still did not yield verified state changes. Continue independent core implementation while keeping actual UI acceptance open.
