#pragma once
#include <stdint.h>
#include <stddef.h>
#ifdef __cplusplus
extern "C" {
#endif
typedef struct SpiewakCore SpiewakCore;
typedef struct { int64_t id, parent; const char *name, *path; uint32_t has_children, kind; uint64_t track_count; uint32_t override_kind; } SpiewakFolder;
typedef struct { int64_t id, folder; const char *title, *artist, *album, *path, *artwork, *codec; double duration; uint32_t number, year; } SpiewakTrack;
typedef struct { int64_t id; const char *name; uint64_t count; } SpiewakPlaylist;
/* Read-only header of a larger owned allocation. Never allocate this yourself. */
typedef struct SpiewakSnapshot {
 const SpiewakFolder *folders; size_t folder_count;
 const SpiewakTrack *tracks; size_t track_count;
 double position; int64_t current_id, current_index; uint64_t revision;
 uint32_t playing; float volume; uint32_t flags; const char *error, *message;
 const SpiewakPlaylist *playlists; size_t playlist_count;
} SpiewakSnapshot;
uint32_t spiewak_abi_version(void);
SpiewakCore *spiewak_open(const char *database_path);
void spiewak_close(SpiewakCore *);
void spiewak_snapshot_free(SpiewakSnapshot *);
SpiewakSnapshot *spiewak_folders(SpiewakCore *, int64_t parent);
SpiewakSnapshot *spiewak_tracks(SpiewakCore *, int64_t folder, uint32_t offset, uint32_t limit);
SpiewakSnapshot *spiewak_add_root(SpiewakCore *, const char *path);
SpiewakSnapshot *spiewak_rescan(SpiewakCore *);
SpiewakSnapshot *spiewak_play_folder(SpiewakCore *, int64_t folder, int64_t track);
SpiewakSnapshot *spiewak_audio_command(SpiewakCore *, uint32_t command, double value);
SpiewakSnapshot *spiewak_now(SpiewakCore *);
SpiewakSnapshot *spiewak_queue(SpiewakCore *);


// Selection mode: 0 play, 1 append, 2 play next. Queue command indexes are zero-based.
SpiewakSnapshot* spiewak_selection(SpiewakCore*, const int64_t* ids, size_t count, uint32_t mode);

SpiewakSnapshot *spiewak_search(SpiewakCore *, const char *query);
SpiewakSnapshot *spiewak_tracks_sorted(SpiewakCore *, int64_t folder, uint32_t offset, uint32_t limit, uint32_t sort);
SpiewakSnapshot *spiewak_play_view(SpiewakCore *, int64_t folder, int64_t track, uint32_t sort);
SpiewakSnapshot *spiewak_playlists(SpiewakCore *);
SpiewakSnapshot *spiewak_playlist_tracks(SpiewakCore *, int64_t id, uint32_t offset);
SpiewakSnapshot *spiewak_playlist_command(SpiewakCore *, int64_t id, uint32_t command, const char *name, const int64_t *ids, size_t count, uint32_t index);
SpiewakSnapshot *spiewak_play_playlist(SpiewakCore *, int64_t id, uint32_t index);

/* Notification revision/status; flags bit 0 means all registered roots are watched. */
SpiewakSnapshot *spiewak_library_state(SpiewakCore *core);
SpiewakSnapshot *spiewak_folder(SpiewakCore *core, int64_t id);
SpiewakSnapshot *spiewak_refresh_queue(SpiewakCore *core);
/* Folder kinds: 0 auto, 1 album, 2 artist, 3 loose, 4 mixed, 5 compilation, 6 collection. */
SpiewakSnapshot *spiewak_folder_kind(SpiewakCore *, int64_t id, uint32_t kind);
#ifdef __cplusplus
}
#endif
