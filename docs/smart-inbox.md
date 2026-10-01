# Smart Inbox — confirmed household requirement

Automation remains disabled. Explicit manual metadata handling and reversible operation history now have native Windows validation (METADATA.md); actual dialog acceptance is still open. Reviewed MusicBrainz/CAA discovery and downloads now have native validation (DISCOVERY.md). Local fingerprint identification is implemented (FINGERPRINT.md); live AcoustID still requires an application key. Configured read-only Inbox intake is implemented below. Automatic provider selection/application remains unfinished.

Pipeline: configured folders now collect new stable files for review. Existing manual cleanup, provider search, fingerprints, preview/Apply and Undo are available from that view. Automatic scheduling of discovery, a defensible confidence policy and automatic application remain future work. Existing albums must never be silently moved or reorganized. No metadata may be invented for unresolved files.

Provider credentials must be configured securely outside the repository. Low confidence belongs in Needs Review. Each automatic rename/tag/artwork mutation needs a recoverable original state and a tested Undo path before enabling automatic apply.

## Confirmed priority and provider boundaries

The owner confirmed automatic artwork, tag discovery and cleanup of newly downloaded YouTube MP3s as household requirements. See household-requirements.md. Tags/artwork must preserve directory ownership and must not create prominent derived albums from single tracks. Filename noise cleanup is the current interpretation; audio editing is not confirmed.

Integrations and remaining work:

- Implemented: reviewed MusicBrainz recording/release metadata lookup. A descriptive application User-Agent and at most one request per second are required; use a queue/cache and backoff rather than firing one network call per UI row. [API](https://musicbrainz.org/doc/MusicBrainz_API), [rate limiting](https://musicbrainz.org/doc/MusicBrainz_API/Rate_Limiting).
- Implemented for explicit review: local Chromaprint and AcoustID fingerprint lookup when names/tags are insufficient. Live service acceptance remains open without the application key. Requires an application API key, duration and an audio fingerprint; do not assume a filename is a fingerprint. No music-file upload is needed for the lookup API. [Official service](https://acoustid.org/webservice).
- Implemented: bounded Cover Art Archive front-cover download for a reviewed MusicBrainz release/release group. Artwork can be absent (404); support redirects, bounded downloads, cache and retry/backoff. A successful metadata match does not guarantee artwork. [API](https://musicbrainz.org/doc/Cover_Art_Archive/API).

Keep provider provenance and match evidence with suggestions. Do not use a provider search score as a calibrated probability or hard-code a promise that a given percentage is safe. Artist/title/duration/version must be consistent; unresolved candidates remain Needs Review. Cache art under app data first; embedding artwork/tagging or renaming requires reversible mutation history. Existing local artwork and manual folder presentation remain authoritative.


## Read-only Inbox intake (schema 14 / ABI 11)

Browse an existing real folder, choose **Folder options → Use as Inbox**, and confirm the stated scope. **Inbox folders…** lists/removes configured folders and shows waiting/unreadable counts plus up to five diagnostic paths. The sidebar **Inbox · Needs review** opens a normal paged track view with playback, Find tags and cover, Edit tags and Mark reviewed. Routine review list navigation is not a modal dialog. Configure or remove changes only app data.

Enabling scans the owning library root before recording a stable-ID baseline. Only subsequently indexed tracks directly in that configured folder enter intake; pre-existing tracks, previously indexed tracks moved in, and subfolders are excluded. Configure a subfolder separately when wanted. Repeated enable preserves the baseline. Disable cascades only the intake records; enabling again establishes a fresh baseline. Folder configuration follows supported same-root/volume folder renames, but deletion/recreation creates a new folder requiring explicit configuration.

The existing native watcher discovers files; its worker checks intake about every two seconds while no scan/file operation is active. Size, modification timestamp and Windows file identity must remain unchanged for at least ten seconds across observations. A tag/header read without embedded artwork must return a positive duration. Windows validation allows readers but refuses concurrent writers/renames; unreadable/locked files retry after thirty seconds, and changed files start waiting again. This is a conservative heuristic, **not proof a downloader has finished forever or every audio frame decodes**. No fingerprint, HTTP lookup or file mutation occurs during intake.

Ready files remain in their original folders, with original tags, covers, presentation and IDs. Mark reviewed takes the displayed ID/observation timestamp and rejects a changed or missing file. Review acknowledgement persists after restart and temporary file unavailability. A later observed byte-size/time/identity change brings the item back to waiting, including an explicit tag edit; after editing, allow the file to settle before marking it reviewed. Filename-only renames with retained identity do not reset that state. Disabling Inbox, marking reviewed and moving a file out never delete music. Current rows are removed from intake after scanner reconciliation confirms deletion or changed folder ownership.

All observations are in the per-app SQLite database. Existing schema 13 libraries migrate with no Inbox enabled and preserve their playlists/session/history. ABI 11 adds folder status/configuration and single-item acknowledgement using the existing owned Snapshot; listening view 3 supplies up to 500 ready rows per page. The library revision refreshes the visible view. State values in SQLite: 0 waiting, 1 needs review, 2 reviewed, 3 unavailable/unreadable.

## Evidence and remaining gates

Six Rust Inbox tests cover migration, baseline, repeated enable, partial/growing files, active Windows writer, stale review, unavailable/moved files, restart and unchanged source bytes. Native C# tests exercise the real Windows watcher, Unicode paths, writer exclusion, delayed readiness, NTFS rename, persistence and disable through ABI 11. See STATUS.md for the final run results.

Current limits: direct folders only; ready-list plus aggregate waiting/error status (not a complete per-item progress/retry UI); no automatic discovery/confidence/application. Large/network Inbox performance is unmeasured; intake currently visits tracked Inbox entries and shares the existing scan worker. Ten seconds and thirty seconds are minimum/nominal intervals, not deadlines during scans, suspension or storage failures. Cache-only ready entries can briefly remain visible at startup until background checking; acknowledgements and actual metadata writes perform their own source checks. A same-size edit with a deliberately preserved timestamp cannot be detected by the stability heuristic, although actual Apply/Undo retain their content-hash checks. Multiple simultaneous application instances are not certified. Actual configuration/review dialog gestures and household acceptance remain open.
