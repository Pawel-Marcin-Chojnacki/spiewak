# Master Implementation Plan: Modern Folder-First Music Player for Windows

## 1. Mission

Design and implement a production-quality native Windows music player that combines:

- the filesystem-first control and information density of foobar2000,
- the library-management capabilities of MusicBee and MediaMonkey,
- the visual polish of modern applications such as Plexamp, Tauon and modern WinUI applications,
- automatic metadata cleanup similar to MusicBrainz Picard,
- an exceptionally fast, native desktop experience.

The application is primarily intended for users who maintain carefully organized music directories and do **not** want metadata to redefine their collection.

The most important product differentiator is:

> The filesystem is the user's music collection. Metadata enriches it but never replaces its structure.

UI/UX is a first-class product requirement, not a presentation layer to implement after the backend.

The application should feel like a hypothetical foobar2000 designed from scratch in 2026.

---

# 2. Product Principles

These principles are non-negotiable.

## 2.1 Filesystem is the source of truth

Given:

```text
Music/
├── Pink Floyd/
│   ├── The Wall/
│   └── Wish You Were Here/
│
├── AC-DC/
│   └── Back in Black/
│
└── Random Songs/
    ├── Haddaway - What Is Love.mp3
    ├── Billie Eilish - Birds of a Feather.mp3
    └── random-song.mp3
```

the application must preserve and present that hierarchy.

A random Haddaway track must not become visually equivalent to an artist for whom the user owns ten albums.

The application may generate alternative views such as:

- Albums
- Album Artists
- Artists
- Tracks
- Genres
- Years

but these are **derived views**.

They must never replace Folder view as the canonical organization.

---

## 2.2 Never reorganize music automatically without explicit permission

Automatic operations may safely:

- read metadata,
- identify a recording,
- fetch metadata,
- fetch artwork,
- suggest corrections,
- rename a newly downloaded loose track,
- update tags when enabled.

They must NOT silently:

- move existing albums,
- split folders,
- merge folders,
- move tracks between artist directories,
- delete files,
- restructure an existing library.

Any destructive or structural operation needs explicit user authorization.

---

## 2.3 Exception-driven automation

Automation should handle obvious cases silently and bring only uncertain cases to the user's attention.

Example:

```text
SMART INBOX                                      4

✓ Billie Eilish — Birds of a Feather      99%
  Identified · renamed · tagged · artwork added

✓ Haddaway — What Is Love                  98%
  Identified · renamed · tagged · artwork added

? Unknown Song                             71%
  Possible match available

! audio_928193.mp3                          —
  Could not identify
```

The ideal interaction is:

> The user only handles exceptions.

---

## 2.4 Native desktop application

Do NOT build:

- Electron,
- React Native,
- Flutter,
- a local web application,
- a WebView-based UI,
- localhost HTTP communication between frontend and backend.

The application should be one native process.

Architecture:

```text
MusicPlayer.exe
│
├── .NET / C#
│   └── WinUI 3
│
└── Rust native library
    └── application core
```

Communication must happen through native FFI.

---

# 3. Technology Stack

## Windows UI

Use:

- C#
- WinUI 3
- XAML
- latest stable Windows App SDK
- latest supported stable .NET version

Do not use experimental Windows App SDK features unless there is a compelling requirement.

As of project initialization, verify the latest stable releases before pinning dependencies.

Use MVVM pragmatically.

CommunityToolkit.Mvvm may be used to reduce boilerplate, but avoid creating an unnecessarily framework-heavy application.

The UI layer should contain almost no domain logic.

---

# 4. Rust Core

Rust owns:

```text
audio engine
library
filesystem model
scanner
watcher
metadata
database
search
queue
playlists
smart inbox
identification
artwork management
playback state
business rules
```

Suggested crates:

```text
Audio decoding        Symphonia
Audio device          CPAL / WASAPI
Metadata              Lofty
Resampling            Rubato
Database              rusqlite / SQLite
Filesystem watching   notify
Async work             Tokio
HTTP                   reqwest
Serialization          serde
Fingerprinting         Chromaprint bindings
```

Evaluate dependencies before introducing them.

Prefer mature, actively maintained libraries.

Avoid deprecated APIs and abandoned crates.

---

# 5. Repository Architecture

Use a Cargo workspace.

Suggested structure:

```text
music-player/
│
├── README.md
├── docs/
│   ├── architecture.md
│   ├── ux.md
│   ├── library-model.md
│   └── ffi.md
│
├── core/
│   ├── Cargo.toml
│   │
│   └── crates/
│       ├── player-domain/
│       ├── player-audio/
│       ├── player-library/
│       ├── player-metadata/
│       ├── player-storage/
│       ├── player-scanner/
│       ├── player-search/
│       ├── player-identification/
│       ├── player-playlists/
│       ├── player-inbox/
│       └── player-ffi/
│
├── windows/
│   ├── Player.sln
│   │
│   └── Player/
│       ├── App.xaml
│       ├── Views/
│       ├── ViewModels/
│       ├── Controls/
│       ├── Converters/
│       ├── Services/
│       ├── Native/
│       ├── Assets/
│       └── Themes/
│
└── tests/
```

Dependencies should point inward.

Example:

```text
player-domain
     ↑
     ├── player-library
     ├── player-audio
     ├── player-playlists
     └── player-inbox
              ↑
        player-ffi
```

The core must not depend on WinUI, .NET or Windows UI concepts.

---

# 6. Rust ↔ C# Boundary

Treat the FFI boundary as an application API.

Do NOT expose hundreds of tiny getter functions such as:

```text
GetTrackArtist()
GetTrackTitle()
GetTrackAlbum()
GetTrackDuration()
GetTrackPath()
```

Prefer coarse operations:

```text
GetFolderContents(folderId)
GetAlbum(albumId)
Search(query)
GetQueue()
GetNowPlaying()
GetInboxItems()
```

Return compact DTOs.

Commands should look conceptually like:

```text
Play(trackId)
Pause()
Resume()
Seek(position)
PlayNext()
PlayPrevious()

QueueAdd(trackIds)
QueueRemove(queueItemId)
QueueMove(queueItemId, targetPosition)

ScanLibrary()
AddLibraryRoot(path)
RemoveLibraryRoot(path)

Search(query)

ApplyInboxSuggestion(itemId)
RejectInboxSuggestion(itemId)
Undo(operationId)
```

Core-to-UI events:

```text
PlaybackStateChanged
PlaybackPositionChanged
NowPlayingChanged
QueueChanged

FolderChanged
LibraryChanged
TrackChanged

ScanStarted
ScanProgress
ScanCompleted

InboxItemAdded
InboxItemUpdated

ArtworkAvailable
```

Playback position updates should not saturate FFI or the UI thread.

Use a reasonable update frequency and allow UI interpolation where appropriate.

Investigate Interoptopus as the preferred FFI solution.

If it introduces unnecessary complexity or limitations, use a stable manually designed C ABI with generated C# bindings.

Do not introduce IPC or JSON serialization between the UI and Rust core.

---

# 7. Library Model

The central concept is:

```text
Filesystem
    ↓
Indexer
    ↓
SQLite cache
    ↓
Application views
```

SQLite is an index.

It is NOT the canonical music collection.

Deleting the database should allow the application to rebuild the collection from the filesystem.

Store stable internal identities wherever practical so renames and moves can be distinguished from delete/create events.

---

# 8. Folder Types

Introduce semantic folder types.

Possible values:

```text
Auto
Album
Artist
LooseTracks
MixedCollection
Compilation
UserCollection
Ignore
```

`Auto` should be the default.

The application may infer:

### Album

Signals:

- common album tag,
- common album artist,
- track numbers,
- similar artwork,
- mostly audio files,
- little directory nesting.

### LooseTracks

Signals:

- many unrelated artists,
- missing album relationships,
- independent tracks.

### Artist

Signals:

- child directories that appear to be albums.

Users must be able to override automatic classification.

The override must be persistent.

---

# 9. Artist Semantics

Do NOT let arbitrary track artist tags pollute the main artist catalog.

Primary Artist view should default to:

```text
Album Artists
```

Secondary modes:

```text
Album Artists
All Artists
Track Artists
Featured Artists
```

Thus one random song from an artist does not create the same visual presence as an artist with multiple complete albums.

---

# 10. Audio Engine

Build the playback pipeline inside Rust.

Target conceptual pipeline:

```text
File
 ↓
Symphonia demux
 ↓
Symphonia decode
 ↓
PCM
 ↓
ReplayGain
 ↓
DSP pipeline
 ↓
optional resampling
 ↓
CPAL
 ↓
WASAPI
 ↓
Audio device
```

Initial playback requirements:

- MP3
- FLAC
- AAC/M4A where supported
- ALAC where supported
- Ogg Vorbis
- Opus
- WAV

Support:

- play
- pause
- resume
- stop
- seek
- previous
- next
- queue
- volume
- mute
- output device selection

Design the engine from the beginning to support:

- gapless playback,
- ReplayGain,
- preloading next track,
- crossfade later,
- DSP later.

Do not implement crossfade before gapless playback is correct.

---

# 11. Queue

Queue is independent from library structure.

It needs:

- append
- insert next
- remove
- reorder
- clear
- shuffle
- repeat track
- repeat queue

The current queue survives UI navigation.

Optionally persist it across application restarts.

Right sidebar should be capable of presenting the queue without navigating away from the current library view.

---

# 12. Metadata

Use a dedicated metadata abstraction backed initially by Lofty.

Read:

```text
Title
Artist
Album
Album Artist
Track Number
Track Total
Disc Number
Disc Total
Year
Genre
Composer
Comment
ReplayGain
Embedded Artwork
```

Preserve unknown/custom metadata where possible when updating files.

Never destroy unrelated tags merely because the application updates one field.

---

# 13. Smart Inbox

Smart Inbox is a core feature.

Users can configure one or more watched inbox directories, for example:

```text
Downloads/Music
Music/Incoming
```

When a new file appears:

```text
File created
    ↓
wait until writing has completed
    ↓
basic validation
    ↓
read existing metadata
    ↓
clean obvious filename noise
    ↓
attempt metadata matching
    ↓
fingerprint if necessary
    ↓
obtain candidate metadata
    ↓
calculate confidence
    ↓
apply automatically OR request confirmation
```

---

# 14. Filename Cleanup

Recognize common download noise such as:

```text
(Official Video)
(Official Music Video)
(Official Audio)
(Lyrics)
(Lyric Video)
[HD]
[4K]
HQ
1080p
320kbps
youtube
yt1s
y2mate
```

Do NOT blindly remove arbitrary bracketed text.

A cleaned loose-track filename should normally follow:

```text
Artist - Title.ext
```

Users must later be able to configure naming templates.

Examples:

```text
{artist} - {title}
{track:02} - {title}
{album_artist}/{album}/{track:02} - {title}
```

For the initial version, only implement the first two if necessary.

---

# 15. Track Identification

Identification pipeline should support:

```text
existing tags
filename parsing
duration
audio fingerprint
AcoustID
MusicBrainz
```

Do not treat filename parsing as authoritative.

Possible confidence system:

```text
>= 95%     automatically apply if automation enabled
80–94%     suggest strongly
60–79%     require confirmation
< 60%      leave unresolved
```

Do not hard-code these exact thresholds deep into the system.

They should be configurable internally and potentially user-configurable later.

Confidence should consider:

- fingerprint match,
- duration similarity,
- artist similarity,
- title similarity,
- release information,
- existing tags.

Never invent metadata when no trustworthy match exists.

---

# 16. Artwork

Support:

```text
embedded artwork
cover.jpg
folder.jpg
front.jpg
```

Album folders:

Prefer shared folder artwork where appropriate.

Loose-track collections:

Prefer embedded artwork because multiple unrelated tracks may live in the same directory.

Artwork resolution order should be deterministic and documented.

Cache decoded/scaled UI artwork separately from the original artwork.

Never repeatedly decode a multi-megabyte album image merely to render a 48x48 row thumbnail.

---

# 17. Automatic Operation History

Every automatic file mutation must create a reversible operation record.

Examples:

```text
rename
tag update
artwork insertion
```

Provide:

```text
Undo
```

for recent automatic actions.

Whenever possible, undo must restore:

- original filename,
- original tags,
- original artwork state.

This is essential.

Automation that cannot be trusted will eventually be disabled by the user.

---

# 18. Filesystem Monitoring

Use filesystem events for responsiveness, but do not assume they are perfectly reliable.

Architecture:

```text
filesystem events
       +
startup reconciliation
       +
periodic reconciliation
```

Support:

- create
- delete
- rename
- move
- modify

Debounce bursts of filesystem events.

Avoid rescanning the entire library when one file changes.

On application startup:

1. Load cached library immediately.
2. Render UI immediately.
3. Reconcile filesystem state in the background.
4. Incrementally update UI as differences are discovered.

Never block application startup on a full filesystem scan.

---

# 19. Search

Search must understand both filesystem and metadata.

Search:

```text
pink floyd
```

should find artists, albums and tracks.

Search:

```text
80s party
```

should also find:

```text
D:\Music\Parties\80s Party
```

Search results should be grouped:

```text
FOLDERS

ALBUMS

ARTISTS

TRACKS

PLAYLISTS
```

Initial implementation may use SQLite FTS5.

Do not introduce a dedicated search engine until measurements justify it.

---

# 20. UI/UX Is the Primary Product Feature

Do not treat UI as a collection of default WinUI controls.

The interface needs deliberate product design.

The target feeling is:

> modern, calm, extremely responsive, information-rich, native and predictable.

Avoid:

- Spotify clone layouts,
- huge unused empty areas,
- excessive rounded cards,
- giant artwork everywhere,
- excessive animation,
- hamburger-menu-heavy navigation,
- hiding common actions behind multiple menus,
- mobile UI stretched onto desktop.

This is a desktop power application.

Use Windows interaction conventions.

---

# 21. Main Window Layout

Use three primary regions:

```text
┌───────────────────────────────────────────────────────────────────────────┐
│ Search                                                   window controls │
├────────────────┬───────────────────────────────────┬──────────────────────┤
│ NAVIGATION     │ CONTENT                           │ CONTEXT / QUEUE      │
│                │                                   │                      │
│ Folders        │                                   │                      │
│ Albums         │                                   │                      │
│ Artists        │                                   │                      │
│ Tracks         │                                   │                      │
│ Playlists      │                                   │                      │
│                │                                   │                      │
│ Inbox       3  │                                   │                      │
│                │                                   │                      │
│ folder tree    │                                   │                      │
├────────────────┴───────────────────────────────────┴──────────────────────┤
│ NOW PLAYING / TRANSPORT BAR                                               │
└───────────────────────────────────────────────────────────────────────────┘
```

Allow the right panel to be hidden.

Allow the left panel width to be resized.

Remember layout preferences.

---

# 22. Navigation

Primary navigation:

```text
Folders
Albums
Artists
Tracks
Playlists

Inbox
```

Settings should be separated visually from library navigation.

The user should be able to navigate naturally through folders without losing playback state.

Back/forward navigation should behave predictably.

Use native mouse buttons / keyboard navigation where practical.

---

# 23. Folder View

This is the most important screen.

Left panel contains a real filesystem tree.

Example:

```text
▾ Music
  ▾ Pink Floyd
    ▸ The Wall
    ▸ Wish You Were Here

  ▸ AC-DC

  ▾ Random Songs
```

Expansion state should persist.

Do not load an entire million-node tree eagerly.

Use virtualization and lazy child loading.

---

# 24. Folder Content Presentation

Presentation depends on folder type.

## Artist folder

Show:

- artist heading,
- albums as compact cover tiles or list,
- optional loose tracks beneath.

## Album folder

Show:

```text
[ARTWORK]

The Wall
Pink Floyd
1979 · 26 tracks · 1h 21m

[Play] [Shuffle] [...]

01  In the Flesh?                  3:20
02  The Thin Ice                   2:26
03  Another Brick in the Wall...  3:11
```

## Loose Tracks

Use a dense track list.

Example:

```text
TITLE                         ARTIST          YEAR      TIME
What Is Love                  Haddaway        1993      4:31
Birds of a Feather            Billie Eilish  2024      3:30
...
```

Do not pretend this folder is an album.

---

# 25. Density Modes

Provide at least two UI density modes:

```text
Comfortable
Compact
```

Comfortable:

- larger rows,
- artwork thumbnails,
- modern visual spacing.

Compact:

- high information density,
- minimal row padding,
- suitable for thousands of files,
- inspired by foobar2000.

Switching density should not alter information architecture.

---

# 26. Track Rows

Track rows should support configurable columns eventually.

Initial columns:

```text
Track #
Title
Artist
Album
Year
Duration
Codec
Bitrate
```

Do not show every column by default.

Folder view may optionally show the real filename as secondary text.

Example:

```text
Haddaway — What Is Love
Haddaway - What Is Love.mp3
```

This is particularly useful for users who care about their actual filesystem.

---

# 27. Album Grid

Albums should support:

```text
Grid
List
```

Grid cards should be restrained:

```text
[cover]
The Wall
Pink Floyd · 1979
```

Avoid Spotify-scale cards.

Desktop users should see many albums at once.

Allow resizing the grid via:

```text
Ctrl + mouse wheel
```

or another intuitive density mechanism.

---

# 28. Artists View

Default:

```text
Album Artists
```

Use either:

- compact artwork tiles,
- dense list.

Do not clutter artist tiles with one-off loose-track artists by default.

---

# 29. Now Playing Bar

Persistent bottom transport:

```text
[artwork]  Another Brick in the Wall
           Pink Floyd

           ⏮   ▶/❚❚   ⏭

──────●────────────────────
1:43 / 3:11

                     🔊 ─────●
```

It should remain visible throughout navigation.

Support:

- click artwork/title → open Now Playing,
- context menu,
- favorite/rating later,
- queue shortcut,
- device selector.

The transport controls must have stable positions.

Never move Play/Pause controls based on context.

---

# 30. Full Now Playing View

This can be visually richer than the main library.

Potential layout:

```text
                 [large artwork]

                 Track title
                 Artist
                 Album

              ⏮    ▶    ⏭

        ─────────●────────────

              queue / lyrics
```

Use artwork-derived visual accents carefully.

Do not reduce text contrast.

Do not make the screen visually noisy.

Animations should be subtle and GPU-friendly.

---

# 31. Queue Panel

Right panel:

```text
UP NEXT

Playing
[cover] Another Brick...
        Pink Floyd

Next

1  Hey You
2  Comfortably Numb
3  Run Like Hell
```

Interactions:

- drag reorder,
- Delete removes,
- context menu,
- Clear,
- Save as Playlist.

Do not navigate away from the library when manipulating the queue.

---

# 32. Inspector Panel

Right panel may switch from:

```text
Queue
```

to:

```text
Track Info
```

Inspector should show:

```text
Artwork

Title
Artist
Album
Album Artist
Year
Track
Disc
Genre

FILE
D:\Music\Pink Floyd\The Wall\01 - ....flac

AUDIO
FLAC
24-bit
96 kHz
2 channels

[Edit metadata]
[Show in Explorer]
```

This is where advanced information belongs.

Do not overload the default track table with it.

---

# 33. Smart Inbox UX

Smart Inbox needs a polished dedicated screen.

Example:

```text
INBOX                                           3 items

AUTO-CLEANED

✓ Birds of a Feather
  Billie Eilish
  Confidence 99%

  Renamed:
  youtube_BillieEilish_birds_official.mp3
  →
  Billie Eilish - Birds of a Feather.mp3

  [Undo]


NEEDS REVIEW

? Unknown Song

  Current:
  unknown_23982.mp3

  Suggested:
  Artist — Song
  Album · 2004

  Match confidence 73%

  [Apply] [Search alternatives] [Ignore]
```

The application should communicate clearly what it changed.

Avoid modal dialogs for routine Inbox processing.

---

# 34. Drag and Drop

Support native drag/drop extensively.

Examples:

Drag track:

```text
→ queue
→ playlist
```

Drag folder:

```text
→ application
```

to add it as a library root or temporarily play it.

Dragging tracks outside the application should expose actual files where Windows APIs permit it.

---

# 35. Context Menus

Context menus should expose advanced functionality without cluttering primary UI.

Track:

```text
Play
Play Next
Add to Queue

Add to Playlist >

Show in Folder
Open Properties

Edit Metadata
Identify Track

Copy >
```

Folder:

```text
Play
Shuffle

Rescan
Folder Type >

Show in Explorer
Properties
```

Use standard Windows context menu behavior.

---

# 36. Keyboard UX

Power-user keyboard navigation is essential.

At minimum:

```text
Space            Play/Pause
Ctrl+F           Search
Ctrl+L           Focus folder/library navigation if appropriate
Enter            Play/open selection depending on context
Delete           Remove queue item where applicable
Alt+Left         Back
Alt+Right        Forward
Ctrl+A           Select all
Ctrl+Z           Undo recent supported operation
```

Media keys must work globally while the application runs.

Later allow configurable shortcuts.

---

# 37. Multi-Selection

Desktop lists must support:

```text
Ctrl click
Shift click
Ctrl+A
```

Operations should work on selected tracks:

```text
Play
Queue
Playlist
Edit metadata
Remove from playlist
```

Do not design interactions assuming only one selected item.

---

# 38. Responsiveness

Hard requirement:

UI must never visibly freeze while:

- scanning directories,
- reading tags,
- decoding artwork,
- querying MusicBrainz,
- generating fingerprints,
- updating the database.

All expensive work occurs outside the UI thread.

Target experience:

```text
launch
→ shell visible nearly immediately
→ cached library visible immediately
→ reconciliation continues in background
```

Scrolling must remain smooth while scanning.

---

# 39. Virtualization

Assume users may have:

```text
100,000+
```

tracks.

Do not render thousands of WinUI controls eagerly.

Use appropriate virtualization for:

- track tables,
- album grids,
- folder trees,
- search results,
- queue.

Benchmark large synthetic libraries.

---

# 40. Artwork Performance

Implement:

```text
original artwork
       ↓
artwork cache
       ↓
size-specific decoded image
       ↓
UI
```

Do not pass huge raw artwork blobs repeatedly through FFI.

Design an efficient mechanism such as:

- cached local artwork file path,
- stable artwork ID,
- requested thumbnail size.

Rust should own canonical artwork/cache state.

---

# 41. Search Interaction

Global search should be fast enough for search-as-you-type.

Example:

```text
pink flo
```

Results overlay/page:

```text
Folders
  Pink Floyd

Artists
  Pink Floyd

Albums
  The Wall
  Wish You Were Here

Tracks
  Pink Floyd — Time
```

Debounce user input slightly.

Cancel obsolete searches.

Keyboard navigation through results must work.

---

# 42. Settings

Initial settings sections:

```text
Library
Playback
Appearance
Metadata
Smart Inbox
Advanced
```

## Library

- library roots,
- excluded folders,
- scan/reconcile.

## Playback

- output device,
- ReplayGain,
- gapless,
- volume behavior.

## Appearance

- system/light/dark,
- density,
- artwork size.

## Metadata

- preferred metadata source,
- artwork behavior,
- automatic tag updates.

## Smart Inbox

- watched locations,
- filename cleanup,
- automatic apply threshold,
- naming rule.

---

# 43. Theme

Support:

```text
System
Light
Dark
```

Use WinUI/Fluent conventions.

Use Mica or other native materials only where appropriate.

Do not layer translucent surfaces excessively.

Content hierarchy should come from:

- spacing,
- typography,
- separators,
- contrast,

rather than a large number of nested cards.

---

# 44. Visual Design Direction

Use a restrained modern desktop aesthetic.

Suggested visual properties:

```text
corner radius        moderate, not excessive
spacing              consistent 4/8px system
animations           120–250 ms for routine transitions
icons                Fluent/System icons
typography            native Windows typography
accent                Windows accent where appropriate
```

Artwork should provide personality.

The surrounding UI should stay neutral.

---

# 45. Accessibility

Do not postpone accessibility.

Support:

- keyboard navigation,
- correct focus order,
- screen-reader labels,
- high contrast,
- scalable text,
- visible focus indicators,
- reduced-motion preferences where relevant.

Use native WinUI capabilities rather than rebuilding basic controls unnecessarily.

---

# 46. Database

SQLite schema should model at least:

```text
library_roots
folders
tracks

albums
artists
track_artists

artwork

playlists
playlist_items

queue_state

inbox_items
metadata_candidates

operations
operation_changes

scan_state
settings
```

Avoid premature denormalization.

Measure before optimizing.

Use migrations from the beginning.

---

# 47. IDs

Do not expose raw filesystem paths as application IDs.

Use internal stable IDs.

Paths may change.

The application should attempt to recognize moves/renames rather than treating every rename as an entirely new track.

---

# 48. Error Handling

Errors should be actionable.

Bad:

```text
Error 0x80070005
```

Better:

```text
Cannot update this file.

The file is read-only:
D:\Music\...\song.flac

[Show file] [Dismiss]
```

Technical details may be expandable.

Do not interrupt playback for unrelated metadata/scanner errors.

---

# 49. Logging

Implement structured logging.

Separate areas:

```text
audio
library
scanner
filesystem
metadata
network
database
ffi
ui
```

Normal users should not see logs.

Provide an easy way later to export diagnostics.

Never log sensitive credentials/API secrets.

---

# 50. Testing Strategy

Rust unit tests should cover:

- folder classification,
- filename cleanup,
- metadata merge logic,
- confidence calculations,
- queue operations,
- playlist rules,
- reconciliation,
- path handling.

Integration tests:

```text
temporary library folder
→ initial scan
→ add track
→ rename track
→ move album
→ delete track
→ restart
→ reconcile
```

Metadata fixture tests should use small legal/generated audio fixtures.

UI tests should cover critical workflows.

---

# 51. Performance Benchmarks

Create synthetic libraries for:

```text
1,000 tracks
10,000 tracks
100,000 tracks
```

Measure:

- startup,
- scan,
- search,
- memory,
- scroll behavior,
- metadata indexing,
- artwork cache performance.

Do not optimize based on intuition alone.

---

# 52. Phase 0 — Foundation

Before implementing features:

1. Create repository structure.
2. Create Cargo workspace.
3. Create WinUI application.
4. Build Rust DLL.
5. Establish FFI.
6. Send a trivial Rust value to WinUI.
7. Establish CI builds.
8. Write architecture documentation.

Acceptance:

```text
WinUI executable loads the Rust core successfully.
```

---

# 53. Phase 1 — Vertical Slice

Build the smallest complete useful application.

Implement:

- choose music folder,
- scan folders,
- index tracks,
- display folder tree,
- display tracks,
- play selected track,
- play/pause,
- seek,
- next/previous,
- basic artwork,
- persistent database.

This phase is successful when:

> The user can point the application at a real music folder, browse exactly the same directory hierarchy and listen to music.

Do not spend weeks building infrastructure before this vertical slice works.

---

# 54. Phase 2 — Excellent Folder UX

Focus heavily on UI.

Implement:

- folder classifications,
- album presentation,
- loose-track presentation,
- breadcrumbs,
- right inspector,
- queue sidebar,
- comfortable/compact density,
- context menus,
- keyboard navigation,
- drag/drop,
- sorting,
- column layouts.

At this point the application should already feel more pleasant than foobar2000 for ordinary library navigation.

---

# 55. Phase 3 — Reliable Library Synchronization

Implement:

- filesystem watcher,
- event debouncing,
- incremental changes,
- startup reconciliation,
- background scanning,
- move/rename handling.

Test external changes made using Windows Explorer while the application is running.

Acceptance:

```text
Explorer filesystem changes appear in the application without requiring a manual library rebuild.
```

---

# 56. Phase 4 — Metadata and Artwork

Implement:

- robust tag reading,
- metadata inspector,
- tag editing,
- artwork detection,
- artwork cache,
- album artist logic.

Do not implement automatic file mutation yet until manual metadata handling is reliable.

---

# 57. Phase 5 — Smart Inbox

Implement:

1. watched inbox locations,
2. stable-file detection after download,
3. filename parsing,
4. cleanup rules,
5. fingerprint generation,
6. AcoustID integration,
7. MusicBrainz integration,
8. artwork retrieval,
9. confidence model,
10. suggestions,
11. automatic application,
12. Undo.

This is a major product milestone.

---

# 58. Phase 6 — Search and Power Features

Implement:

- global search,
- FTS index,
- playlists,
- smart playlists,
- ReplayGain,
- advanced queue,
- customizable columns,
- keyboard shortcuts.

---

# 59. Phase 7 — Polish

Focus almost entirely on:

- interaction latency,
- scroll smoothness,
- animation,
- layout consistency,
- typography,
- empty states,
- loading states,
- error states,
- keyboard flow,
- accessibility,
- high-DPI behavior,
- multi-monitor behavior.

Do not equate polish with adding features.

---

# 60. Phase 8 — Packaging

Create a proper Windows distribution.

Evaluate current recommended Windows App SDK packaging options.

Prefer:

- simple installation,
- clean updates,
- no administrator privileges when unnecessary,
- no unnecessary background services.

Application data should live in standard Windows application-data locations.

Music files remain where the user put them.

---

# 61. Explicit Non-Goals for Initial Releases

Do NOT initially implement:

- streaming services,
- Spotify integration,
- Apple Music integration,
- YouTube integration,
- cloud accounts,
- user accounts,
- social features,
- AI recommendations,
- online recommendations,
- visualizers,
- advanced EQ,
- plugin system,
- mobile client,
- server architecture.

Design core abstractions so some of these remain possible, but do not implement speculative architecture for hypothetical features.

---

# 62. Engineering Rules

Follow these throughout development.

### Rule 1

Never use deprecated APIs when a supported replacement exists.

### Rule 2

Prefer latest stable dependencies over preview/experimental dependencies.

### Rule 3

Do not add an abstraction until it has a concrete purpose.

### Rule 4

Do not duplicate domain logic in C# and Rust.

Rust is authoritative.

### Rule 5

Do not block the UI thread with file, database, image or network work.

### Rule 6

Do not silently modify existing user files.

### Rule 7

Do not sacrifice filesystem fidelity for metadata convenience.

### Rule 8

Measure performance rather than guessing.

### Rule 9

Keep commits small and coherent.

### Rule 10

A feature is not finished until its UX is finished.

---

# 63. Product-Level Acceptance Scenarios

The final architecture should support all of these naturally.

## Scenario A — Carefully organized albums

```text
Music/
└── Pink Floyd/
    ├── The Wall/
    └── Wish You Were Here/
```

The user sees exactly this hierarchy.

Both album folders receive rich album presentation.

---

## Scenario B — Random songs

```text
Random Songs/
├── Haddaway - What Is Love.mp3
├── Daft Punk - One More Time.mp3
└── Billie Eilish - Birds of a Feather.mp3
```

The folder appears as one loose-track collection.

These tracks do not pollute the primary Album Artists catalog.

---

## Scenario C — External rename

User changes:

```text
Random
```

to:

```text
Loose Songs
```

in Windows Explorer.

The application recognizes the change and updates without rebuilding the entire library.

---

## Scenario D — New downloaded track

A downloader creates:

```text
y2mate.com_Billie_Eilish_BIRDS_OF_A_FEATHER_Official.mp3
```

The application waits until the file is complete.

It identifies the recording.

It proposes or automatically produces:

```text
Billie Eilish - Birds of a Feather.mp3
```

with proper tags and artwork.

The user receives a subtle notification with:

```text
Undo
```

---

## Scenario E — Unknown track

Identification confidence is low.

The application does nothing destructive.

It places the track into:

```text
Inbox → Needs Review
```

and displays candidates.

---

## Scenario F — Huge collection

The user has 100,000 tracks.

The application starts using its cached index rather than rescanning everything.

Library browsing remains responsive while reconciliation happens in the background.

---

# 64. UX Definition of Success

The UI succeeds if all of the following are true:

1. A foobar2000 power user immediately understands how to browse the library.
2. A normal Windows user can use the application without configuration.
3. Folder structure is always visible and understandable.
4. Important information is available without overwhelming the default view.
5. Advanced information is one click away.
6. Frequent actions rarely require menus.
7. Routine operations do not trigger modal dialogs.
8. Playback is never interrupted by library-management tasks.
9. A large library does not make the application feel heavy.
10. The UI feels unmistakably like a modern native Windows desktop application.

---

# 65. First Implementation Target

Do not attempt the entire roadmap at once.

Start by delivering this exact vertical slice:

```text
Launch application
      ↓
Add D:\Music as library
      ↓
Rust scans filesystem
      ↓
metadata indexed into SQLite
      ↓
WinUI renders filesystem tree
      ↓
select folder
      ↓
tracks appear instantly
      ↓
double-click track
      ↓
Rust audio engine plays it
      ↓
persistent bottom Now Playing bar updates
      ↓
queue shows upcoming tracks
```

The folder tree, track list and Now Playing experience should already receive deliberate UI design.

Do not use placeholder-quality UI merely because this is the first milestone.

---

# 66. Initial Main-Screen Target

Use this as the initial design direction:

```text
┌──────────────────────────────────────────────────────────────────────────────────┐
│  ◀  ▶        Search your library...                              ─  □  ×        │
├───────────────┬──────────────────────────────────────────────┬────────────────────┤
│ LIBRARY       │ Music / Pink Floyd / The Wall               │ UP NEXT            │
│               │                                              │                    │
│ ▣ Folders     │  ┌────────┐                                  │ NOW PLAYING        │
│   Albums      │  │        │  THE WALL                       │                    │
│   Artists     │  │ cover  │  Pink Floyd                     │ Another Brick...   │
│   Tracks      │  │        │  1979 · 26 tracks               │ Pink Floyd         │
│   Playlists   │  └────────┘                                  │                    │
│               │                                              │ NEXT               │
│ ⚡ Inbox   3  │  ▶ Play    Shuffle    •••                   │                    │
│               │                                              │ 1  Mother          │
│ ───────────── │  #  TITLE                       TIME         │ 2  Goodbye Blue... │
│               │  01 In the Flesh?               3:20         │ 3  Empty Spaces    │
│ ▾ Music       │  02 The Thin Ice                2:26         │                    │
│   ▾ Pink Floyd│  03 Another Brick in the Wall   3:11         │                    │
│     ▸ The Wall│  04 The Happiest Days...        1:50         │                    │
│   ▸ AC-DC     │                                              │                    │
│   ▸ Random    │                                              │                    │
│               │                                              │                    │
├───────────────┴──────────────────────────────────────────────┴────────────────────┤
│ [art]  Another Brick in the Wall              ───────●────────────  2:13 / 3:11 │
│        Pink Floyd                              ⏮      ▶      ⏭          🔊 ──●  │
└──────────────────────────────────────────────────────────────────────────────────┘
```

This is a design direction, not a requirement to reproduce ASCII literally.

Iterate using screenshots of the actual application.

Evaluate:

- hierarchy,
- alignment,
- density,
- typography,
- wasted space,
- discoverability,
- native behavior.

---

# 67. Development Method

Work iteratively.

For each meaningful feature:

1. understand the user workflow,
2. define domain behavior,
3. define UI interaction,
4. implement the smallest vertical path,
5. test it,
6. run the application,
7. inspect the actual UI,
8. improve UX,
9. commit.

Do not implement twenty backend services and only then connect the UI.

Maintain a working application throughout development.

---

# 68. Documentation

Keep these documents current:

```text
docs/architecture.md
docs/library-model.md
docs/audio-engine.md
docs/ffi.md
docs/ux.md
docs/smart-inbox.md
```

Document important decisions and their rationale.

Do not create documentation merely to describe obvious code.

Focus on architectural decisions that future agents/developers need to preserve.

---

# 69. Decision Authority

When requirements are ambiguous:

1. preserve user files,
2. preserve filesystem organization,
3. prefer native Windows UX,
4. prefer simpler architecture,
5. prefer stable technology,
6. prefer measurable performance,
7. prefer reversible operations.

If an implementation choice harms UX merely to make the code easier, reconsider the implementation.

If a fancy abstraction makes the application harder to reason about, remove the abstraction.

---

# 70. Ultimate Product Goal

The finished product should make a user say:

> “This gives me the control of foobar2000, but I no longer feel like I am using software designed twenty years ago.”

The application should respect users who organize their music themselves instead of forcing them into somebody else's metadata model.

Its defining combination should be:

```text
FILESYSTEM FIDELITY
        +
MODERN NATIVE UI
        +
FAST LOCAL PLAYBACK
        +
AUTOMATIC CLEANUP
        +
REVERSIBLE AUTOMATION
```

Build toward that product, not toward a generic music player.