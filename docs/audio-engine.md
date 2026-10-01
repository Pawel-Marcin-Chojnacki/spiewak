# Audio engine

Rodio 0.22.2 was selected as the maintained streaming integration of Symphonia and CPAL/WASAPI. This avoids implementing a second decoder/output glue layer for the first playable slice. Its pinned dependency graph currently uses Symphonia 0.5.5 and CPAL 0.17.3; direct upgrades are constrained by Rodio compatibility. Cargo.lock is committed.

A dedicated Rust worker owns the output device and player. The device is opened on first playback, so a machine without speakers can still browse music. Decoding streams from an open file rather than loading complete songs into RAM. Commands travel on an in-process channel; state is sampled by WinUI four times per second. Queue storage is shared with Arc, so seek/volume commands do not clone a large queue.

Play builds a queue from the chosen folder. Pause/resume, stop, bounded seek, next/previous and volume are supported. Previous restarts the current track after three seconds, otherwise selects its predecessor. Stop disables automatic advancement. Last-track completion does not wrap. A failed explicit selection keeps the prior queue/audio and returns an actionable error.

Compiled decoders cover WAV, MP3, FLAC, Vorbis, AAC/M4A and ALAC. **Opus decoding is not provided by the pinned Symphonia version.** Opus files can be indexed but playback reports a decoding error. Generated MP3, FLAC, AAC/M4A, ALAC, Ogg Vorbis and WAV fixtures have passed index/decode/play/seek integration on Windows ARM64; see STATUS.md.

Automatic next has a short transition; gapless playback, preloading, ReplayGain, output-device selection and mute UI are not yet implemented. Do not claim gapless correctness or add crossfade before measuring sample continuity.

Mixed selection queues support append/play-next, duplicate entries, upcoming removal/reorder/clear, and jump without rebuilding a folder queue. Shuffle reorders the upcoming suffix; current playback/history stay intact. Repeat-one applies to natural completion; explicit Next advances. Mute preserves remembered volume. Queue state is still session-only.
