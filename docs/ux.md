# First folder-first UX

A three-region desktop shell keeps a lazy native folder tree, dense virtualized track rows, and queue/file inspector visible together. Transport stays at the bottom across navigation. Real filenames are secondary row text; the inspector shows the full selectable path. Local folder artwork appears in the folder heading and current-track thumbnail.

Add music folder opens a native Windows folder picker. Open folders through either the tree or the center subfolder list. Double-click/Enter plays a track and replaces the queue with that entire folder. Play folder starts its first track. The sidebar lists upcoming tracks; double-clicking one plays from that track's source folder. This first queue behavior is intentionally basic and will need dedicated queue-jump/edit commands before mixed queues are introduced.

Space toggles playback; Alt+Left goes back; native extended selection supports Ctrl/Shift. Track context actions include Play and Show in Explorer. Compact changes row padding, and Queue toggles the right panel. Labels/focus use native WinUI automation properties and theme resources. Slider seeks are debounced 180 ms; timer updates do not fight pending user seeks.

No navigation entries advertise unfinished Albums, Artists, Playlists or Inbox views. Those are later features. The first slice has no search, forward history, persistent layout/tree expansion, resizable left pane, playlist actions or global media-key support. Selected-track Play presently starts its folder queue; batch selection operations are later work.

Actual screenshots must be inspected for each completed iteration. UI automation access through Parallels may need separate input tooling; record the testing method and limits in STATUS.md.
