# First folder-first UX

A three-region desktop shell keeps a lazy native folder tree, dense virtualized track rows, and queue/file inspector visible together. Transport stays at the bottom across navigation. Real filenames are secondary row text; the inspector shows the full selectable path. Local folder artwork appears in the folder heading and current-track thumbnail.

Add music folder opens a native Windows folder picker. Open folders through either the tree or the center subfolder list. Double-click/Enter plays a track and replaces the queue with that entire folder. Play folder starts its first track. The sidebar lists upcoming tracks; double-clicking one jumps to its occurrence in the existing mixed queue. Queue editing preserves the playing source.

Space toggles playback; Alt+Left goes back; native extended selection supports Ctrl/Shift. Track context actions include Play and Show in Explorer. Compact changes row padding, and Queue toggles the right panel. Labels/focus use native WinUI automation properties and theme resources. Slider seeks are debounced 180 ms; timer updates do not fight pending user seeks.

Folders remain the canonical navigation. Search, forward history, persistent playlists and batch queue operations are implemented. Album/artist catalogs, Inbox, persistent layout/tree expansion, resizable left pane and global media keys remain later features. No navigation entries advertise unfinished views.

Actual screenshots must be inspected for each completed iteration. UI automation access through Parallels may need separate input tooling; record the testing method and limits in STATUS.md.

The interim visual direction is a quiet dark music workspace, off-white text and lavender transport accent. Design/Theme.xaml owns semantic colors and shared control styles (light/high contrast also defined). Retain native focus/keyboard/screen-reader behavior when refining visuals later. This is a functional baseline, not final UI/UX approval.

Confirmed household rules are in household-requirements.md. Existing folders get conservative semantic presentation plus a manual override in the folder menu. Album/Compilation folders have a 96 px artwork header; LooseTracks/MixedCollection/Artist/UserCollection have a compact text header and real-file list. Search/playlists also use compact headers. The child folder list shows a quiet kind/count label. No album entity is synthesized from a stray file's Album tag. A dedicated Home with a side section for singles is still open; final UI should validate it with household workflows rather than assume this shell is approved.
