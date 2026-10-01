# Audio fingerprint identification

Schema 13 / typed ABI 10 adds local audio fingerprint calculation and reviewed AcoustID → MusicBrainz candidates to **Spiewak**. It does not automatically edit files or configure Inbox folders. Those remain separate R12 work.

## User behavior

Select one available file and choose **Find tags and cover…**. Without an application key, **Calculate local audio fingerprint** decodes locally and caches an odcisk audio in app data. No provider request is sent. Title/artist searches remain available independently. With an application key and explicitly enabled online lookup, **Identify from audio** sends the encoded fingerprint and whole-file duration to AcoustID; audio bytes and the local path are not uploaded.

Results carry AcoustID similarity evidence, duration/version checks and MusicBrainz recording/edition IDs. All candidates need review, including a score of 1.0. An audio match cannot determine one unique album edition. Returned recording IDs are deduplicated using the strongest valid score; invalid IDs/scores are ignored. Live/Remix/Acoustic and other known version conflicts cannot be previewed as a different version. Meaningful annotations and existing tags/covers retain the existing discovery rules.

Preview, Apply and Undo use the existing reversible editor. The original folder is authoritative; discovery does not rename, move or create album folders. Fingerprint/source identity, SHA-256 and subsequent proposal checks reject changed files. Provider provenance survives Apply and Undo. Missing key, missing match, provider error and version ambiguity leave the file unchanged and playable.

## Application key

The owner confirmed on 1 October 2026 that no application key is available yet. Live AcoustID identification therefore has not passed acceptance. Register an **application client key**, not a user submission key, at [AcoustID application registration](https://acoustid.org/new-application). Account login is required. This app performs lookup only; it does not submit fingerprints. [Web-service documentation](https://acoustid.org/webservice).

Secrets are read only from `SPIEWAK_ACOUSTID_CLIENT_KEY` in the Windows process environment. No key is stored in SQLite, written to a file, returned through a snapshot, or placed in the request URL. HTTP/provider error text is not echoed. `.env.example` documents the variable; the player does not load `.env` files.

A temporary Windows launch can read the key without echoing it or putting it into shell history:

```powershell
$key = Read-Host 'AcoustID application key' -AsSecureString
$buffer = [Runtime.InteropServices.Marshal]::SecureStringToBSTR($key)
try {
    $env:SPIEWAK_ACOUSTID_CLIENT_KEY = [Runtime.InteropServices.Marshal]::PtrToStringBSTR($buffer)
    & .\tools\run.ps1
} finally {
    [Runtime.InteropServices.Marshal]::ZeroFreeBSTR($buffer)
    Remove-Item Env:SPIEWAK_ACOUSTID_CLIENT_KEY -ErrorAction SilentlyContinue
}
```

Do not paste the key into chat, source code or a command argument. The child player inherits it for that run. Online lookup still defaults off.

## Implementation and limits

Official, unmodified [Chromaprint 1.6.1](https://github.com/acoustid/chromaprint/releases/tag/v1.6.1) library sources are statically compiled through the existing MSVC toolchain, using bundled KissFFT/internal avresample and default TEST2. The source archive hash, upstream notices and LGPL resampler license are included in `core/crates/player-library/native/chromaprint`. No runtime fpcalc process, FFmpeg DLL, IPC or additional audio device is used. Rust decodes with pinned Symphonia 0.5.5, already used by playback, with container padding trimming enabled.

Calculation streams bounded decoded packets into Chromaprint and feeds only the first 120 seconds. It measures the whole decoded duration; supported regular files are limited to 512 MiB, three hours, 10+ seconds of decodable audio, 1–8 channels and rates 1001–192000 Hz. Corrupt/unsupported audio fails without sending identification. Read locks deny concurrent writes on Windows. Successful fingerprints are cached by track, source SHA-256/identity and pinned decoder/algorithm version, capped at 5,000 entries. Indexing remains read-only and does not start identification.

AcoustID lookup uses HTTPS POST, verified TLS, no redirects, 15-second deadline and a 2 MiB body limit. Starts are durably reserved at least 1.1 seconds apart per service host, below the published three-per-second maximum. Error/busy responses back off. Network and decoding work do not own the playback or UI library locks. AcoustID response bodies/keys are not stored in the HTTP cache; reviewed candidate evidence is stored separately. MusicBrainz/CAA retain their existing bounded cache and rate rules.

## Verification

The comparison example and `tools/verify-fingerprint.ps1` are test tools, outside the product path. The script generates varied 135-second audio, compares raw vectors against official fpcalc 1.6.1 for WAV16/WAV24/FLAC/MP3/AAC/ALAC/Ogg, checks complete duration, the 120-second item count and exact source hashes. Lossless formats require identical vectors. Lossy decoder differences have an explicit 2% bit tolerance in this fixture; that is not an AcoustID identity threshold or proof of successful live recognition.

Build the example in the same Visual Studio/LLVM environment as the normal build, then compare:

```powershell
# From core/, in the configured native compiler environment:
cargo build -p player-library --release --example fingerprint --target aarch64-pc-windows-msvc
# From the repo root:
powershell.exe -NoProfile -ExecutionPolicy Bypass -File .\tools\verify-fingerprint.ps1 -Ffmpeg "$env:LOCALAPPDATA\Microsoft\WinGet\Links\ffmpeg.exe" -Fpcalc "$env:LOCALAPPDATA\Spiewak\test-tools\chromaprint-1.6.1\fpcalc.exe"
dotnet run --project tests/Core.Smoke/Core.Smoke.csproj -c Release -- "$PWD\core\target\aarch64-pc-windows-msvc\release\spiewak_core.dll" --fingerprint-only --audio --ffmpeg "$env:LOCALAPPDATA\Microsoft\WinGet\Links\ffmpeg.exe"
```

Live recognition requires the owner's registered key and real recordings. Actual WinUI dialog gestures, household acceptance, Inbox configuration and automatic application remain open. x64 validation is not inferred from the ARM64 build.

Windows 11 ARM64 verification completed: 58 Rust tests, workspace fmt, clippy with warnings denied, release DLL and WinUI build. Native C# regression passed six-codec decode/seek, metadata decoded-PCM preservation/exact Undo, real MusicBrainz/CAA transport/cover review and local fingerprint calculation while WASAPI kept playing and Now commands remained responsive. All source SHA-256 checks passed. Missing-key fixture sends no requests even with online enabled; tests also cover content-bound cache invalidation, malformed/invalid/duplicate/empty matches, highest-score deduplication, static credential-safe errors, backoff, version conflicts and durable provenance/Undo.

Official fpcalc comparison on varied 135-second fixtures returned 948 items for every format (first 120 seconds). WAV16, WAV24, FLAC, MP3, ALAC and Ogg raw vectors were identical. AAC differed by 1.6284% of aligned bits with the two decoders; whole duration stayed within 0.1 seconds and source bytes were preserved. No live AcoustID identity success is claimed from these generated fixtures.

Logs in the VM owner profile: `spiewak-fingerprint-final-build.log`, `spiewak-fingerprint-build.log`, `spiewak-fingerprint-tests.log`, `spiewak-fingerprint-clippy.log`, `spiewak-fingerprint-reference.log`, `spiewak-fingerprint-native-audio.log`, `spiewak-fingerprint-final-audio.log`.

The rebuilt Spiewak demo shell was launched and visibly rendered in Parallels. Guest navigation/text-input attempts did not verify the discovery dialog workflow; concurrent VM windows changed the foreground. Interactive acceptance remains open.
