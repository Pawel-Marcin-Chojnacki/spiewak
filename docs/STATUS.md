# Verified status — first vertical slice

As of 2026-09-30 on Windows 11 ARM64 (Parallels). This is a working foundation/first playback milestone, not the complete product roadmap.

## Implemented

- Native WinUI 3 / .NET 10 process loading Rust through typed C ABI.
- Add library roots; background read-only scanning; rebuildable SQLite WAL cache with schema version check.
- Real folder hierarchy, lazy tree children, center subfolder navigation, paged/virtualized track list.
- Lofty title/artist/album/album-artist/number/year/duration; filename fallback; cover.jpg/folder.jpg/front.jpg/PNG folder covers.
- Streaming Rust audio on a dedicated worker; folder queue, play/pause/resume/stop/seek/next/previous/volume.
- Persistent transport, upcoming queue sidebar, path/album inspector, compact rows, hideable queue, Back, Enter/Space/Alt+Left, basic context menu.
- Cached startup before background reconciliation; rescan external additions/renames/deletions without changing music.

## Verification

- Rust: six unit/integration tests; fmt and clippy with warnings denied.
- Release WinUI build on ARM64; native shell visually inspected in actual VM screenshots.
- C# ↔ Rust smoke: Unicode paths, actual WAV metadata/tags, folder fidelity, IDs across rescan/restart, external rename/delete, SHA-256 music preservation.
- Codec integration: generated MP3, FLAC, AAC/M4A, ALAC, Ogg Vorbis and WAV fixtures all index, decode/play and seek successfully on Windows ARM64. Corrupt audio returns an error while previous audio/queue continue. FFmpeg 9.0.2 installed via winget for fixture generation only.
- Windows audio smoke: default WASAPI device opens; position advances; pause freezes it; resume/seek/volume/next/previous/stop/end-of-queue pass. Quiet generated signals only. This verifies decoding/output command flow, not subjective listening quality.
- Foundation GitHub Windows x64 CI passed: https://github.com/Pawel-Marcin-Chojnacki/spiewak/actions/runs/36770607755 . Current-feature CI and additional codec/UI checks are recorded below after completion.

## Current limits / next work

- No Opus decoder in pinned Rodio/Symphonia graph. Files are indexed but playback reports an error.
- No watcher, incremental event synchronization or stable file IDs across rename/move yet; use Rescan. No album classification or derived album/artist catalog.
- No embedded-artwork extraction/cache, tag editing, search, playlists, Smart Inbox, provider integrations or Undo. All file mutation is absent/disabled.
- No gapless guarantee, ReplayGain, output-device selector, mute button or global media keys.
- Queue is a folder queue; queue editing/shuffle/repeat/mixed queues and restart persistence are not implemented.
- Tree expansion/layout/density are not persisted; no resizable left panel or forward navigation.
- Native extended selection exists; batch actions are not implemented. Full accessibility/high contrast/high-DPI/multi-monitor review and 1k/10k/100k benchmarks remain open.
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

Computer Use can inspect the actual Parallels screenshot and native macOS menus, but guest clicks/keys were not delivered in this session. Full picker/tree/double-click testing is pending authorization for PowerShell UI Automation as an alternative input method. No guest UI workflow is claimed verified solely from passing core tests.
