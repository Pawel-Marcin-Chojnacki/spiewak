# Manual metadata and reversible file changes

Schema 11 / ABI 8. Windows ARM64 implementation. Smart Inbox and online providers remain separate, unfinished R12 requirements.

## User workflow

Select exactly one available track, then **Edit tags…** in its context menu. Basic title/artist/album/filename fields and front cover appear first; recording date, album artist, track/disc totals, genre, composer, comment and existing ReplayGain values are under **More tags**. ReplayGain editing does not analyze or change audio loudness. Choosing a cover only replaces the front image, preserving other picture types.

**Suggest cleanup of download name** fills a proposal. The current conservative whitelist removes trailing Official Video/Official Audio/Lyrics/320kbps annotations, including the known bracketed forms. It keeps Live/Remix/Acoustic and other meaningful version text. Artist/title inference from `Artist - Title` fills only truly empty tags. Existing tags are not overwritten by the suggestion. This is a local filename heuristic, not identification of the recording.

**Preview changes** prepares a copy under app data, then presents before/after values. **Apply changes** explicitly changes the original file. It stays in the same physical folder, keeps its audio extension and stable Windows identity, and keeps playlist/favorite references. It never creates derived album directories. A filename-only edit does not rewrite tags. Cancel leaves source music unchanged.

**File change history** lists the newest 100 operations. Undo restores the entire original bytes and filename, including metadata not represented in the editor. Explicit recovery for an interrupted/conflicting write preserves the current unrecognized bytes in a separate recovery copy before restoring the original. Existing unrelated files at either filename are never replaced. No background recovery writes music.

Editing the selected audio source unloads its decoder, retains queue/cursor/position and leaves it paused. Resume is explicit and decodes its refreshed path. Other playing tracks continue. A short audio hold prevents automatic advancement into a file while it is being changed; user playback commands are serialized with file mutation. Selection IDs are re-resolved after waiting behind a write, so a pending selection uses the new filename.

## Storage and failures

Indexing remains read-only. Originals and prepared copies live beside the database in `library.operations/<operation ID>/`, not in the music folder. Scanning skips the operation/cache directories even when a containing directory is registered. Originals are retained, not silently pruned. The current quota is 4 GiB, with a conservative allowance for added metadata/artwork; individual editable files are limited to 512 MiB. Storage management/export for older backups remains later work. Keep the database and operations directory together for recovery; deleting app data deletes this recovery history.

Preparation requires one regular identifiable Windows file. Reparse points and hard links are rejected. A read-sharing lock protects originals/staged inputs from concurrent writes/deletes, and SHA-256 plus Windows file identity is checked before mutation. Tag editing uses pinned Lofty 0.25.4 with preservation of format-specific items. Unsupported/lossy requested field conversions fail during preparation; no original is then written. Date/number syntax and filename/extension/Windows reserved names are checked. JPEG/PNG input is bounded and decoded through the existing artwork validation.

Applying/undoing journals its state before rename/write. Rename uses the owned source handle without replacing another file. Source writes retain its NTFS identity; a failed file/index update attempts rollback from the verified retained copy. If rollback cannot finish, the journal keeps Conflict and both original/recovery evidence. After an abrupt termination, startup reconciles verified completed bytes into the cache/journal. Torn or externally changed content is retained for explicit recovery instead of automatic overwrite. Undo refuses any content changed after application by another program.

A whole-file copy into the owned handle is not power-loss atomic. This is why originals, durable journal states, startup detection and explicit recovery exist. Local NTFS on the Windows 11 VM is the verified filesystem; network/cloud placeholders and other volumes are not certified. Preservation of unknown tags is exercised with a custom ID3 TXXX field; universal preservation of every proprietary format item is not claimed.

## Verification

Windows Rust tests cover preview/apply/undo, exact original restoration, PCM chunk preservation, custom TXXX/genre preservation, front/back artwork handling, backup exclusion from indexing, filename-only crash recovery, concurrent writer/hard-link rejection, invalid names, collision, external edits, damaged backups, SQLite failure rollback, interrupted writes and schema migration preserving favorites/playlists/IDs.

Native C# ↔ Rust checks exercise typed UTF-8 metadata ownership, all 18 fields, before/after proposals, cancellation, queue path/identity refresh, paused source editing, explicit resume and history/Undo after restarting the native core. Generated WAV/MP3/FLAC/AAC-M4A/ALAC-M4A/Ogg fixtures compare decoded PCM SHA-256 before/after tag changes and verify byte-exact Undo. These tests run on disposable files, not the household collection.

The WinUI editor/history connections compile. Actual context-menu, cover picker, dialog focus/scroll and history gestures remain an acceptance gate: Mac Computer Use can observe the Parallels guest, but the new File change history button displayed correctly but its Computer Use click only changed hover, without opening a verified dialog. A screenshot of the shell does not certify the full editor workflow. Online MusicBrainz/AcoustID/Cover Art Archive discovery, automatic cover downloads, configured Inbox and automatic application remain unimplemented; their household priority is unchanged.
