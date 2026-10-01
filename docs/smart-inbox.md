# Smart Inbox — confirmed household requirement

Not implemented in the first playback slice. Keep automation disabled until manual metadata handling and reversible operation history are reliable.

Future pipeline: watch configured inboxes, wait for stable files, validate, read tags, remove only known filename noise, match/fingerprint against trustworthy providers, compute configurable confidence, propose/apply changes, record reversible history. Existing albums must never be silently moved or reorganized. No metadata may be invented for unresolved files.

Provider credentials must be configured securely outside the repository. Low confidence belongs in Needs Review. Each automatic rename/tag/artwork mutation needs a recoverable original state and a tested Undo path before enabling automatic apply.

## Confirmed priority and provider boundaries

The owner confirmed automatic artwork, tag discovery and cleanup of newly downloaded YouTube MP3s as household requirements. See household-requirements.md. Tags/artwork must preserve directory ownership and must not create prominent derived albums from single tracks. Filename noise cleanup is the current interpretation; audio editing is not confirmed.

Proposed integrations, not implemented:

- MusicBrainz recording/release metadata lookup. A descriptive application User-Agent and at most one request per second are required; use a queue/cache and backoff rather than firing one network call per UI row. [API](https://musicbrainz.org/doc/MusicBrainz_API), [rate limiting](https://musicbrainz.org/doc/MusicBrainz_API/Rate_Limiting).
- AcoustID fingerprint lookup when names/tags are insufficient. Requires an application API key, duration and an audio fingerprint; do not assume a filename is a fingerprint. No music-file upload is needed for the lookup API. [Official service](https://acoustid.org/webservice).
- Cover Art Archive for a matched MusicBrainz release/release group. Artwork can be absent (404); support redirects, bounded downloads, cache and retry/backoff. A successful metadata match does not guarantee artwork. [API](https://musicbrainz.org/doc/Cover_Art_Archive/API).

Keep provider provenance and match evidence with suggestions. Do not use a provider search score as a calibrated probability or hard-code a promise that a given percentage is safe. Artist/title/duration/version must be consistent; unresolved candidates remain Needs Review. Cache art under app data first; embedding artwork/tagging or renaming requires reversible mutation history. Existing local artwork and manual folder presentation remain authoritative.
