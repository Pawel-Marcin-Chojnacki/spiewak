# Reviewed tag and cover discovery

MusicBrainz recording/release discovery and automatic download of a selected release's front cover are implemented. Search results always need review. Audio fingerprint identification was added in the following milestone (FINGERPRINT.md). Configured Inbox folders and automatic file changes remain open.

## Household behavior

Select one available track and choose **Find tags and cover…**. Online lookup starts disabled. The dialog explains which artist/title terms are sent to MusicBrainz and which services deliver images; audio files are not uploaded. Enabling requests is saved in app data. Fresh cached searches remain usable after disabling online requests.

The user can edit the search terms and choose among recordings/editions. Rows include title, artist, release/date/country/disambiguation, duration and match evidence. A search score of 100/100 is not a probability of identity. The generated tone in the public-service smoke deliberately has a different duration from the song being queried; a successful search must still remain Needs Review.

**Preview this candidate** obtains the selected release's metadata and downloads its cover when local artwork is absent. Existing tags and artwork win by default. Replacement requires the explicit replace checkboxes. The shared metadata review shows before/after values, retained provenance and the downloaded cover before explicit Apply. Cancel leaves the music unchanged. Apply/Undo use the existing verified original-file journal (METADATA.md).

The file remains in its real directory, with its filename unchanged by provider discovery. Album tags cannot create synthetic folder navigation or make a singleton an album folder. Known version markers must agree with the original file/title as well as the search terms. Editing a search box cannot hide a Live/Remix/Acoustic conflict. Meaningful parentheses/brackets are retained in proposed titles; only the existing known download-suffix whitelist is removed. Conservative name cleanup remains a separate manual-editor proposal.

The selected recording must occur on the fetched release. Multiple occurrences do not invent track/disc numbering. File identity and SHA-256 are checked before discovery, after search and again before staging. External file changes invalidate the proposal. MusicBrainz IDs and match evidence are stored in a separate journal provenance column and survive Apply, Undo and recovery status messages.

## Provider implementation

Rust owns HTTPS, external REST JSON parsing, SQLite caching and proposal rules. WinUI uses owned typed ABI 10 snapshots, with explicit frees; JSON never crosses the C#–Rust boundary. Network work runs in background calls without owning the UI-library, audio-command or scan locks. Cache image publication and editor staging acquire the same scan gate as existing mutations.

- MusicBrainz searches recordings, then looks up the chosen release. Requests identify Spiewak and reserve starts at least 1.1 seconds apart per service host, including between processes sharing the database. Rate limits/errors are surfaced and backed off. This follows the service's published request and identification guidance. [Rate limiting](https://musicbrainz.org/doc/MusicBrainz_API/Rate_Limiting).
- Cover Art Archive front-500 requests support bounded redirects to the archive services, upgrading allowed legacy HTTP destinations to HTTPS. Missing covers (404) are normal; metadata review can continue without one. [Cover Art Archive API](https://musicbrainz.org/doc/Cover_Art_Archive/API).
- TLS verifies certificates. Redirects are restricted to MusicBrainz, Cover Art Archive and Internet Archive domains with no user credentials or nonstandard port. Five redirects, 15-second request deadlines, 2 MiB JSON and 8 MiB artwork download limits. The artwork decoder retains its existing input/dimension/memory bounds and 512-pixel thumbnails.
- Cached metadata expires after seven days, successful cover downloads after one day and missing responses after one hour. HTTP body storage is capped at 128 MiB; candidate rows at 5,000 total and 40 per track. Referenced candidate artwork is protected from ordinary cache pruning. Originals in the operation store keep the existing 4 GiB quota and are not silently deleted.
- Schema 12 (subsequently 13) preserves existing folders, favorites, playlists, paused sessions and manual operation history. Online setting defaults off during upgrade.

Necessary dependencies are pinned ureq 3.4.2 (blocking HTTPS/rustls) and serde_json 1.0.151 for the external providers. Windows ARM64 requires LLVM/clang for ring; `winget install --id LLVM.LLVM --exact` installs it, and tools/build.ps1 checks/adds the installed compiler path. Windows x64 retains the Visual Studio build environment. No provider API key is needed for the implemented MusicBrainz/CAA read APIs. AcoustID lookup now reads its application key from the Windows environment; local calculation works without a key. See FINGERPRINT.md for the separate validation limits.

## Validation and remaining gates

Windows 11 ARM64 checks include 54 Rust tests, fmt, clippy with warnings denied, release Rust DLL and WinUI build. Provider fixtures cover original preservation/Undo, singleton folder fidelity, durable provenance, local tag/artwork precedence, Live conflict and edited search terms, unknown annotation retention, ambiguous numbering, stale files, disabled/offline cache, expiration, backoff, malformed/oversized responses, unsupported redirects and schema-11 upgrades.

Native C# integration covers ABI 10 layouts, local search terms, opt-in persistence and disabled requests. A real public-service query for Haddaway / What Is Love obtains reviewed MusicBrainz candidates, chosen-release metadata and a bounded PNG through Cover Art Archive redirects/TLS. Generated audio keeps playing and Now commands respond while the lookup runs; preview does not write the source, Apply preserves its folder and provenance, and Undo restores exact bytes. This verifies provider transport and review mechanics, not identification of the generated tone. Six-codec WAV/MP3/FLAC/AAC/ALAC/Ogg decoding, seeking and metadata PCM/restoration regression also passed.

Reproduce offline/core and optional public network checks:

```powershell
powershell.exe -NoProfile -ExecutionPolicy Bypass -File .\tools\build.ps1 -Architecture ARM64
dotnet run --project tests/Core.Smoke/Core.Smoke.csproj -c Release -- "$PWD\core\target\aarch64-pc-windows-msvc\release\spiewak_core.dll" --audio --providers --ffmpeg "$env:LOCALAPPDATA\Microsoft\WinGet\Links\ffmpeg.exe"
```

The last checks are recorded in the VM owner profile: spiewak-discovery-final-build.log, spiewak-discovery-live-audio.log, spiewak-discovery-live.log and spiewak-discovery-final-live.log. The targeted `--discovery-only --audio --providers` smoke repeats provider checks without rerunning unrelated library fixtures.

This document records the initial discovery checks; the latest acoustic milestone uses schema 13 / ABI 10 (FINGERPRINT.md). Actual dialog, selection, checkbox, keyboard and image-preview gestures remain unverified. The native demo shell starts and renders; Computer Use still moves/hovers the guest pointer without a verified double-click navigation change. No interactive UI acceptance is inferred from native tests. Automatic file application remains disabled; live fingerprint matching with the registered key, Inbox configuration, safe confidence rules and household acceptance are still required by the overall research objective. No current-feature x64 CI result is claimed before it runs.
