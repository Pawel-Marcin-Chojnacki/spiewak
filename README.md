# Śpiewak

A native Windows music player that respects your folders. WinUI 3 / .NET 10 + Rust, in one process. GPL-3.0.

The first playable slice indexes music into a rebuildable SQLite cache, presents the real folder hierarchy, reads tags/local covers, and plays folder queues through a Rust Symphonia/CPAL audio worker. See [verified status and current limitations](docs/STATUS.md).

## Build and run

Windows 11, Visual Studio 2026 (C++ desktop tools / Windows SDK), .NET 10 and Rust stable MSVC are required. Builds are self-contained unpackaged apps; no additional service or web runtime is started.

```powershell
.\tools\build.ps1                 # ARM64: tests, fmt, clippy, Rust DLL, WinUI, FFI smoke
.\tools\run.ps1
.\tools\build.ps1 x64
.\tools\run.ps1 x64
```

When cross-building x64 on ARM64, the x64 .NET 10 SDK must also be installed for native smoke tests. GitHub CI uses Windows x64.

Click **Add music folder**, browse your real directory tree, and double-click a track. Space toggles playback; Enter plays the selection; Alt+Left goes back. Indexing never changes your music. Explicit **Edit tags…** offers a preview before writing the selected file; **File change history** can Undo those changes using retained originals. Automatic metadata writes remain disabled. Its database lives in `%LOCALAPPDATA%/Spiewak`. Keep it: besides rebuildable metadata it contains your playlists, favorites, history and playback state. Keep the adjacent `library.operations` directory for file-change recovery. See [manual metadata and recovery limits](docs/METADATA.md).

## Windows audio integration test

The build runs a generated-file smoke test without requiring a sound device. To test actual audio commands on a Windows desktop:

```powershell
dotnet run --project tests/Core.Smoke/Core.Smoke.csproj -c Release -- "$PWD\core\target\aarch64-pc-windows-msvc\release\spiewak_core.dll" --audio
```

Add `--ffmpeg <absolute path to ffmpeg.exe>` to the audio smoke command to generate and test MP3/FLAC/AAC/ALAC/Vorbis/WAV fixtures plus invalid-audio recovery. FFmpeg is test tooling, not an application dependency.

For six legal/generated quiet WAV samples and local test artwork (Python 3):

```powershell
python tools/generate-demo.py "$env:USERPROFILE\Music\Spiewak Demo"
```

The generator refuses to overwrite an existing folder. It downloads no music. Audio tests use low-amplitude sine tones, create a temporary library, and remove it afterwards.

Import UTF-8 M3U8 beside Playlists; export from a playlist to a new filename. Drop files, folders or M3U8 on the browser/queue to append without autoplay. Drop folders on the folder tree to add library roots. See [playlist files and limits](docs/PLAYLIST-FILES.md).

See [architecture](docs/architecture.md), [library model](docs/library-model.md), [audio](docs/audio-engine.md), [FFI](docs/ffi.md), [UX](docs/ux.md), [Smart Inbox policy](docs/smart-inbox.md), and the [complete product plan](docs/master-plan.md). This delivery covers foundation and the first useful playback path; it is not the complete roadmap.

## Product research

[Competitor features, user feedback and implementation priorities (2026-10-01)](docs/research/music-players-2026-10-01.md). Confirmed household needs prioritize real folders, quiet singles, automatic artwork/tag discovery and safe cleanup of downloaded MP3s.

Confirmed household product requirements: [docs/household-requirements.md](docs/household-requirements.md).
