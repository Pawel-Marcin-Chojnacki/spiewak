using Spiewak.Native;
using System.Runtime.InteropServices;
using System.Text;
using System.Security.Cryptography;

if (args.Length < 1) throw new ArgumentException("Usage: Core.Smoke <absolute Rust DLL path> [--audio]");
var dll = Path.GetFullPath(args[0]);
NativeLibrary.SetDllImportResolver(typeof(Core).Assembly, (name, _, _) => name == "spiewak_core" ? NativeLibrary.Load(dll) : 0);
if (args.Length > 1 && args[1] == "--session-child")
{
    using var child = new Core(args[2]);
    child.Play(long.Parse(args[3]), long.Parse(args[4])); child.Command(6, 0.19); child.Command(7);
    child.Command(5, 0.5);
    while (true)
    {
        var state = child.Now(); if (state.Error.Length > 0) throw new Exception(state.Error);
        if (state.Position > 6) File.WriteAllText(args[5], "ready");
        await Task.Delay(100);
    }
}
var sandbox = Path.Combine(Path.GetTempPath(), "spiewak-smoke-" + Guid.NewGuid());
Directory.CreateDirectory(sandbox);
var music = Path.Combine(sandbox, "Muzyka Śpiewak"); var album = Path.Combine(music, "Sessions", "Field Notes"); Directory.CreateDirectory(album);
var one = Path.Combine(album, "01 - Morning.wav"); var two = Path.Combine(album, "02 - Evening.wav");
Wave(one, 330); Wave(two, 440);
var before = SHA256.HashData(File.ReadAllBytes(one));
var db = Path.Combine(sandbox, "cache", "library.db");
try
{
    long folderId, trackId, playlistId;
    using (var core = new Core(db))
    {
        core.AddRoot(music);
        var roots = core.Folders().Folders; Assert(roots.Count == 1 && roots[0].Name == "Muzyka Śpiewak", "Unicode library root");
        var artist = core.Folders(roots[0].Id).Folders.Single(); Assert(artist.Name == "Sessions", "Artist folder preserved");
        var folder = core.Folders(artist.Id).Folders.Single(); folderId = folder.Id;
        var tracks = core.Tracks(folderId).Tracks; Assert(tracks.Count == 2, "Two tracks indexed");
        Assert(folder.Kind == FolderKind.Album && folder.TrackCount == 2, "Coherent numbered folder gets album presentation through typed ABI");
        core.SetFolderKind(folderId, FolderKind.LooseTracks);
        Assert(core.Folder(folderId).Folders.Single().Kind == FolderKind.LooseTracks, "User can keep tagged album tracks as a loose collection");
        core.SetFolderKind(folderId, FolderKind.Auto);
        Assert(tracks[0].Duration > 7.9 && tracks[0].Duration < 8.1, "Actual WAV metadata duration");
        Console.WriteLine($"Metadata: title={tracks[0].Title}; artist={tracks[0].Artist}; album={tracks[0].Album}");
        Assert(tracks[0].Title == "Morning" && tracks[0].Artist == "Spiewak Sessions" && tracks[0].Album == "Field Notes", "Metadata tags enrich, never reparent folders");
        Assert(core.Search("morn").Tracks.Single().Id == tracks[0].Id, "Prefix search through typed ABI");
        Assert(core.Search("ŚPIE").Tracks.Count == 2, "Unicode path search");
        Assert(core.Tracks(folderId, 0, 1).Tracks[0].Title == "Evening", "Native title sort");
        playlistId = core.PlaylistCommand(0, 0, "Evening Śpiewak").CurrentId;
        core.PlaylistCommand(playlistId, 3, rows: new[] { tracks[0], tracks[1], tracks[0] });
        Assert(core.PlaylistTracks(playlistId).Tracks.Count == 3, "Persistent playlist preserves duplicates");
        core.PlaylistCommand(playlistId, 4, index: 2); core.PlaylistCommand(playlistId, 6, index: 0);
        Assert(core.PlaylistTracks(playlistId).Tracks[0].Id == tracks[1].Id, "Playlist reorder/remove by occurrence");
        core.PlaylistCommand(playlistId, 1, "Renamed playlist"); Assert(core.Playlists().Playlists.Single().Name == "Renamed playlist", "Rename playlist");
        core.PlaylistCommand(playlistId, 2); Assert(core.Playlists().Playlists.Count == 0, "Playlist trash");
        core.PlaylistCommand(playlistId, 7); Assert(core.Playlists().Playlists.Count == 1, "Restore playlist without modifying music");
        trackId = tracks[0].Id;
        core.Selection(tracks, 1); await Wait(() => core.Queue().Tracks.Count == 2, "Append selection without auto-playing");
        Assert(!core.Now().Playing, "Queue additions do not start audio");
        core.Command(12, 0); await Wait(() => core.Queue().Tracks[0].Id == tracks[1].Id, "Queue move down");
        core.Command(10, 0); await Wait(() => core.Queue().Tracks.Count == 1, "Remove queue item");
        core.Command(13); await Wait(() => core.Queue().Tracks.Count == 0, "Clear upcoming");
        if (args.Contains("--audio"))
        {
            core.PlayPlaylist(playlistId, 1); await Wait(() => core.Now().CurrentId == trackId && core.Now().CurrentIndex == 1, "Playlist playback starts correct occurrence");
            core.Play(folderId, trackId); await Wait(() => core.Now().Playing && core.Now().Position > 0.15, "Audio starts through WASAPI");
            Assert(core.Queue().Tracks.Count == 2, "Queue contains the whole folder");
            core.Selection(new[] { tracks[1], tracks[0] }, 2); await Wait(() => core.Queue().Tracks.Count == 4, "Insert selection next preserving duplicates");
            Assert(core.Queue().Tracks[2].Id == trackId, "Inserted selection order");
            core.Selection(new[] { tracks[0] }, 2); await Wait(() => core.Queue().Tracks.Count == 5, "Successive Play next request appended to pending prefix");
            Assert(core.Queue().Tracks[1].Id == tracks[1].Id && core.Queue().Tracks[3].Id == trackId, "Successive Play next requests remain FIFO");
            core.Command(10, 3);
            core.Command(10, 2); core.Command(10, 1); await Wait(() => core.Queue().Tracks.Count == 2, "Edit upcoming while audio continues");
            core.Command(7); await Wait(() => (core.Now().Flags & 1) != 0 && core.Now().Playing, "Mute preserves playback"); core.Command(7);
            core.Command(8); await Wait(() => ((core.Now().Flags >> 1) & 3) == 1, "Repeat queue state");
            core.Command(8); await Wait(() => ((core.Now().Flags >> 1) & 3) == 2, "Repeat track state");
            core.Command(5, 7.8); await Wait(() => core.Now().Position < 1 && core.Now().Playing, "Natural end repeats same track");
            core.Command(8); await Wait(() => (core.Now().Flags & 6) == 0, "Repeat off");
            core.Command(1); await Task.Delay(100); var paused = core.Now(); await Task.Delay(250); Assert(!core.Now().Playing && Math.Abs(core.Now().Position - paused.Position) < 0.08, "Pause freezes playback position");
            core.Command(1); await Wait(() => core.Now().Playing, "Resume");
            core.Command(5, 3); await Wait(() => core.Now().Position > 2.8, "Seek");
            core.Command(6, 0.25); await Wait(() => Math.Abs(core.Now().Volume - 0.25) < 0.01, "Volume");
            core.Command(3); await Wait(() => core.Now().CurrentId == tracks[1].Id, "Next track");
            core.Command(4); await Wait(() => core.Now().CurrentId == trackId, "Previous track");
            core.Command(2); await Task.Delay(200); Assert(!core.Now().Playing && core.Now().CurrentId == trackId, "Stop does not advance queue");
            core.Play(folderId, tracks[1].Id); core.Command(5, 7.8); await Wait(() => !core.Now().Playing, "Last track ends without repeating");
        }
        var ffmpegIndex = Array.IndexOf(args, "--ffmpeg");
        if (ffmpegIndex >= 0)
        {
            var formatFolder = Path.Combine(sandbox, "Formats"); Directory.CreateDirectory(formatFolder);
            foreach (var (filename, codec) in new[] { ("sample.mp3", "libmp3lame"), ("sample.flac", "flac"), ("aac.m4a", "aac"), ("alac.m4a", "alac"), ("sample.ogg", "libvorbis"), ("sample.wav", "pcm_s16le") })
                ConvertAudio(args[ffmpegIndex + 1], one, Path.Combine(formatFolder, filename), codec);
            core.AddRoot(formatFolder);
            var formatId = core.Folders().Folders.Single(f => f.Name == "Formats").Id;
            var formats = core.Tracks(formatId).Tracks;
            Assert(formats.Count == 6, "Six generated codec fixtures indexed");
            foreach (var format in formats)
            {
                Assert(format.Duration > 7.8, $"Metadata: {Path.GetFileName(format.Path)}");
                if (args.Contains("--audio"))
                {
                    core.Play(formatId, format.Id); await Wait(() => core.Now().CurrentId == format.Id && core.Now().Playing && core.Now().Position > 0.1, $"Decode/play: {Path.GetFileName(format.Path)}");
                    core.Command(5, 2); await Wait(() => core.Now().Position > 1.8, $"Seek: {Path.GetFileName(format.Path)}"); core.Command(2);
                }
            }
            var corrupt = Path.Combine(formatFolder, "broken.mp3"); File.WriteAllText(corrupt, "Not audio"); core.Rescan();
            if (args.Contains("--audio"))
            {
                core.Play(formatId, formats[0].Id); await Wait(() => core.Now().Playing, "Playback before invalid selection");
                var bad = core.Tracks(formatId).Tracks.Single(t => t.Path == corrupt);
                bool rejected = false; try { core.Play(formatId, bad.Id); } catch (InvalidOperationException) { rejected = true; }
                Assert(rejected, "Invalid audio returns an actionable error"); await Task.Delay(150); Assert(core.Now().CurrentId == formats[0].Id && core.Now().Playing, "Invalid selection preserves prior audio and queue"); core.Command(2);
            }
        }
        core.Rescan(); Assert(core.Tracks(folderId).Tracks[0].Id == trackId, "IDs survive unchanged reconciliation");
        Assert(before.SequenceEqual(SHA256.HashData(File.ReadAllBytes(one))), "Scanner does not change music bytes");
        await Wait(() => (core.LibraryState().Flags & 1) != 0, "Native watcher attached to music roots");
        var eveningId = tracks[1].Id;
        var moved = Path.Combine(album, "03 - Renamed.wav"); File.Move(two, moved);
        await Wait(() => core.Search("renam").Tracks.Count == 1, "External rename automatically updates FTS without Rescan");
        Assert(core.Search("renam").Tracks.Single().Id == eveningId, "Windows file identity survives rename");
        Assert(core.PlaylistTracks(playlistId).Tracks.Single(t => t.Id == eveningId).Path == moved, "Playlist follows renamed file");
        var renamedAlbum = Path.Combine(Path.GetDirectoryName(album)!, "Renamed Album"); Directory.Move(album, renamedAlbum);
        moved = Path.Combine(renamedAlbum, Path.GetFileName(moved)); one = Path.Combine(renamedAlbum, Path.GetFileName(one));
        await Wait(() => core.Folder(folderId).Folders.Single().Path == renamedAlbum, "Folder rename preserves ID and automatically updates hierarchy");
        Assert(core.Tracks(folderId).Tracks.Single(t => t.Id == eveningId).Path == moved, "Tracks retain IDs after album move");
        core.SetFolderKind(folderId, FolderKind.UserCollection);
        Assert(core.Folder(folderId).Folders.Single().OverrideKind == FolderKind.UserCollection, "Presentation override stored independently of tags");
        var added = Path.Combine(renamedAlbum, "04 - Added.wav"); Wave(added, 550);
        await Wait(() => core.Tracks(folderId).Tracks.Count == 3, "External file creation synchronized without Rescan");
        var addedId = core.Tracks(folderId).Tracks.Single(t => t.Path == added).Id;
        File.Delete(added); await Wait(() => core.Tracks(folderId).Tracks.Count == 2, "External deletion synchronized");
        Wave(added, 550); await Wait(() => core.Tracks(folderId).Tracks.Count == 3, "New file at deleted path synchronized");
        Assert(core.Tracks(folderId).Tracks.Single(t => t.Path == added).Id != addedId, "Deleted IDs are never reused for new tracks");
        File.Delete(added); await Wait(() => core.Tracks(folderId).Tracks.Count == 2, "Replacement fixture removed");
        core.RefreshQueue();
        if (args.Contains("--audio"))
        {
            core.Play(folderId, trackId); await Wait(() => core.Now().Playing && core.Now().Position > 0.2, "Playback before queued file rename");
            var renamedAgain = Path.Combine(renamedAlbum, "05 - Another.wav"); File.Move(moved, renamedAgain); moved = renamedAgain;
            await Wait(() => core.Tracks(folderId).Tracks.Single(t => t.Id == eveningId).Path == renamedAgain, "Upcoming file rename synchronized");
            var position = core.Now().Position; core.RefreshQueue(); await Task.Delay(150);
            Assert(core.Now().Playing && core.Now().Position >= position, "Queue refresh preserves playing source and position");
            core.Command(3); await Wait(() => core.Now().Playing && core.Now().CurrentId == eveningId, "Next uses refreshed queue path"); core.Command(2);
        }
        File.Delete(moved);
        await Wait(() => core.PlaylistTracks(playlistId).Tracks.Any(t => t.Id == 0 && t.Path == moved), "Deleted playlist occurrence retains last known path");
        Assert(before.SequenceEqual(SHA256.HashData(File.ReadAllBytes(one))), "Synchronization preserves music bytes after moves");
        var offline = music + " offline"; Directory.Move(music, offline);
        await Wait(() => (core.LibraryState().Flags & 1) == 0, "Unavailable root detected by watcher maintenance");
        Assert(core.Tracks(folderId).Tracks.Single().Id == trackId, "Unavailable root retains cached tracks and playlist links");
        Directory.Move(offline, music);
        await Wait(() => (core.LibraryState().Flags & 1) != 0, "Restored root automatically watched again");
    }
    using (var core = new Core(db)) { Assert(core.Folder(folderId).Folders.Single().Kind == FolderKind.UserCollection, "Folder presentation survives restart"); core.SetFolderKind(folderId, FolderKind.Auto); Assert(core.Folder(folderId).Folders.Single().Kind == FolderKind.LooseTracks, "One remaining tagged song stays a loose track in automatic mode"); Assert(core.Playlists().Playlists.Single().Count == 2, "Playlist survives restart"); core.Rescan(); Assert(core.PlaylistTracks(playlistId).Tracks.Any(t => t.Id == 0), "Unavailable playlist entry remains visible"); Assert(core.Tracks(folderId).Tracks.Count == 1 && core.Tracks(folderId).Tracks[0].Id == trackId, "Cache persists and external delete reconciles after restart"); }
    await SessionChecks(sandbox, dll, args.Contains("--audio"));
    var artworkFfmpeg = Array.IndexOf(args, "--ffmpeg");
    await ArtworkChecks(sandbox, args.Contains("--audio"), artworkFfmpeg >= 0 ? args[artworkFfmpeg + 1] : null);
    Console.WriteLine("PASS: native Rust ↔ C# vertical slice" + (args.Contains("--audio") ? " including Windows audio" : " (audio skipped)"));
}
finally { Directory.Delete(sandbox, true); }

static void Assert(bool condition, string label) { if (!condition) throw new Exception("FAIL: " + label); Console.WriteLine("PASS: " + label); }
static async Task Wait(Func<bool> condition, string label) { for (var i = 0; i < 50; i++) { if (condition()) { Console.WriteLine("PASS: " + label); return; } await Task.Delay(100); } throw new Exception("FAIL: timeout: " + label); }
static void Wave(string path, double frequency)
{
    using var stream = new MemoryStream(); using var writer = new BinaryWriter(stream, Encoding.ASCII, true);
    writer.Write(Encoding.ASCII.GetBytes("RIFF")); writer.Write(0); writer.Write(Encoding.ASCII.GetBytes("WAVEfmt ")); writer.Write(16); writer.Write((ushort)1); writer.Write((ushort)1); writer.Write(44100); writer.Write(88200); writer.Write((ushort)2); writer.Write((ushort)16);
    using var info = new MemoryStream(); using (var tags = new BinaryWriter(info, Encoding.ASCII, true))
    {
        tags.Write(Encoding.ASCII.GetBytes("INFO"));
        foreach (var (key, value) in new[] { ("INAM", Path.GetFileName(path).StartsWith("01") ? "Morning" : "Evening"), ("IART", "Spiewak Sessions"), ("IPRD", "Field Notes"), ("IPRT", Path.GetFileName(path).StartsWith("01") ? "1" : "2") })
        {
            var bytes = Encoding.ASCII.GetBytes(value + "\0"); tags.Write(Encoding.ASCII.GetBytes(key)); tags.Write(bytes.Length); tags.Write(bytes); if (bytes.Length % 2 != 0) tags.Write((byte)0);
        }
    }
    writer.Write(Encoding.ASCII.GetBytes("LIST")); writer.Write((int)info.Length); writer.Write(info.ToArray());
    writer.Write(Encoding.ASCII.GetBytes("data")); writer.Write(44100 * 8 * 2);
    for (int i = 0; i < 44100 * 8; i++) writer.Write((short)(Math.Sin(i * 2 * Math.PI * frequency / 44100) * 500));
    stream.Position = 4; writer.Write((int)stream.Length - 8); writer.Flush(); File.WriteAllBytes(path, stream.ToArray());
}

static void ConvertAudio(string ffmpeg, string source, string target, string codec)
{
    var start = new System.Diagnostics.ProcessStartInfo(ffmpeg) { UseShellExecute = false, RedirectStandardError = true, RedirectStandardOutput = true };
    foreach (var argument in new[] { "-hide_banner", "-loglevel", "error", "-i", source, "-map", "0:a:0", "-c:a", codec, target }) start.ArgumentList.Add(argument);
    using var process = System.Diagnostics.Process.Start(start) ?? throw new Exception("Cannot start ffmpeg");
    var error = process.StandardError.ReadToEnd(); process.WaitForExit(); if (process.ExitCode != 0) throw new Exception("Fixture conversion failed: " + error);
}

static async Task SessionChecks(string sandbox, string dll, bool audio)
{
    var root = Path.Combine(sandbox, "Session files"); Directory.CreateDirectory(root);
    var other = Path.Combine(root, "Loose tracks"); Directory.CreateDirectory(other);
    var aPath = Path.Combine(root, "01 - Session.wav"); var bPath = Path.Combine(other, "02 - Other.wav");
    Wave(aPath, 220); Wave(bPath, 250);
    var db = Path.Combine(sandbox, "session.db"); long rootId; Track a, b;
    using (var core = new Core(db))
    {
        core.AddRoot(root); rootId = core.Folders().Folders.Single().Id;
        a = core.Tracks(rootId).Tracks.Single(); b = core.Tracks(core.Folders(rootId).Folders.Single().Id).Tracks.Single();
        core.Selection(new[] { a, b, a }, 1); core.Command(6, 0.23); core.Command(7); core.Command(8);
    }
    using (var core = new Core(db))
    {
        Assert(core.Queue().Tracks.Select(t => t.Id).SequenceEqual(new[] { a.Id, b.Id, a.Id }), "Restart preserves mixed queue sources, order and duplicates");
        var state = core.Now(); Assert(!state.Playing && state.CurrentIndex == -1 && state.Position == 0, "Upcoming-only queue restores without starting audio");
        Assert(Math.Abs(state.Volume - 0.23) < 0.001 && state.Flags == 3, "Volume, mute and repeat restore");
        if (audio)
        {
            core.Command(1); await Wait(() => core.Now().Playing && core.Now().CurrentIndex == 0, "Play starts first item of a restored upcoming-only queue");
            core.Command(14, 1); core.Command(5, 2); core.Command(1);
            core.Selection(new[] { a }, 2);
        }
    }
    if (audio)
    {
        using (var core = new Core(db))
        {
            var state = core.Now(); Assert(!state.Playing && state.CurrentIndex == 1 && state.CurrentId == b.Id && state.Position > 1.9 && state.Position < 2.4, "Current occurrence and seek position restore paused");
            await Task.Delay(250); Assert(Math.Abs(core.Now().Position - state.Position) < 0.001, "Restored position stays frozen until explicit resume");
            core.Selection(new[] { b }, 2);
            Assert(core.Queue().Tracks.Select(t => t.Id).SequenceEqual(new[] { a.Id, b.Id, a.Id, b.Id, a.Id }), "Restored Play next requests continue FIFO ahead of original suffix");
            core.Command(1); await Wait(() => core.Now().Playing && core.Now().Position > 2.4, "Explicit resume opens WASAPI and seeks restored position");
            core.Command(1);
        }
    }
    var renamed = Path.Combine(root, "03 - Moved.wav"); File.Move(aPath, renamed);
    using (var core = new Core(db)) { core.Rescan(); }
    using (var core = new Core(db))
    {
        Assert(core.Queue().Tracks.Where(t => t.Id == a.Id).All(t => t.Path == renamed), "Restored queue resolves renamed tracks by stable ID");
    }
    File.Delete(bPath);
    using (var core = new Core(db)) { core.Rescan(); }
    using (var core = new Core(db))
    {
        Assert(core.Queue().Tracks.Any(t => t.Id == 0 && t.Path == bPath && t.Title.StartsWith("Unavailable · ")), "Unavailable session occurrences preserve their last known path");
        if (audio)
        {
            bool rejected = false; try { core.Command(1); } catch (InvalidOperationException) { rejected = true; }
            Assert(rejected && !core.Now().Playing && core.Now().CurrentIndex == 1, "Missing restored source fails safely without changing queue");
            core.Command(14, 0); core.Command(5, 7.8);
            await Wait(() => !core.Now().Playing && core.Now().CurrentIndex == 1 && core.Now().Error.Contains("unavailable"), "Natural advance stops visibly at a missing occurrence");
            await Task.Delay(250); Assert(core.Now().CurrentIndex == 1 && core.Now().Position == 0, "Missing occurrence is not silently skipped");
        }
    }
    if (audio)
    {
        var marker = Path.Combine(sandbox, "checkpoint.ready");
        var start = new System.Diagnostics.ProcessStartInfo(Environment.ProcessPath!) { UseShellExecute = false, RedirectStandardError = true };
        if ((Path.GetFileNameWithoutExtension(Environment.ProcessPath) ?? "").Equals("dotnet", StringComparison.OrdinalIgnoreCase)) start.ArgumentList.Add(typeof(Spiewak.Native.Core).Assembly.Location);
        foreach (var value in new[] { dll, "--session-child", db, rootId.ToString(), a.Id.ToString(), marker }) start.ArgumentList.Add(value);
        using var process = System.Diagnostics.Process.Start(start) ?? throw new Exception("Cannot start checkpoint child");
        try
        {
            for (int i = 0; i < 150 && !File.Exists(marker) && !process.HasExited; i++) await Task.Delay(100);
            if (!File.Exists(marker)) throw new Exception("Checkpoint child did not become ready: " + (process.HasExited ? process.StandardError.ReadToEnd() : "timeout"));
            process.Kill(); await process.WaitForExitAsync();
            using var core = new Core(db); var state = core.Now();
            Assert(!state.Playing && state.CurrentId == a.Id && state.Position > 4.5 && state.Position < 6.5 && Math.Abs(state.Volume - 0.19) < 0.001, "Abrupt process termination restores periodic checkpoint without autoplay");
        }
        finally { if (!process.HasExited) { process.Kill(); await process.WaitForExitAsync(); } }
    }
    var invalid = Path.Combine(sandbox, "invalid.db"); File.WriteAllText(invalid, "This is not SQLite");
    bool detailed = false; try { using var bad = new Core(invalid); } catch (IOException e) { detailed = e.Message.Contains("not a database", StringComparison.OrdinalIgnoreCase); }
    Assert(detailed, "Startup reports actual database error instead of a generic permissions claim");
}

static async Task ArtworkChecks(string sandbox, bool audio, string? ffmpeg)
{
    var root = Path.Combine(sandbox, "Artwork files"); var album = Path.Combine(root, "Album"); var loose = Path.Combine(root, "Loose");
    Directory.CreateDirectory(album); Directory.CreateDirectory(loose);
    var red = Png(800, 400, 190, 0, 0); var blue = Png(100, 200, 0, 0, 190); var green = Png(90, 90, 0, 190, 0);
    var files = new[] { Path.Combine(album, "01 - Album.wav"), Path.Combine(album, "02 - Album.wav"), Path.Combine(loose, "01 - Loose.wav"), Path.Combine(loose, "02 - Loose.wav") };
    for (int i = 0; i < files.Length; i++) { Wave(files[i], 220 + i * 30); EmbedArtwork(files[i], i % 2 == 0 ? red : blue, (uint)(i % 2 + 1), i < 2 ? "Coherent album" : "Separate release " + i); }
    var hashes = files.Select(f => SHA256.HashData(File.ReadAllBytes(f))).ToArray();
    File.WriteAllBytes(Path.Combine(album, "COVER.PNG"), green); File.WriteAllBytes(Path.Combine(loose, "cover.png"), green);
    var db = Path.Combine(sandbox, "artwork.db");
    using var core = new Core(db); core.AddRoot(root);
    var rootId = core.Folders().Folders.Single().Id; var children = core.Folders(rootId).Folders;
    var albumFolder = children.Single(f => f.Name == "Album"); var looseFolder = children.Single(f => f.Name == "Loose");
    var albumTracks = core.Tracks(albumFolder.Id).Tracks; var looseTracks = core.Tracks(looseFolder.Id).Tracks;
    Assert(albumFolder.Kind == FolderKind.Album && File.Exists(albumFolder.Artwork) && albumTracks.All(t => t.Artwork == albumFolder.Artwork), "Folder artwork crosses ABI 5 and is shared only by an album folder");
    Assert(looseFolder.Kind == FolderKind.LooseTracks && looseFolder.Artwork.Length == 0 && looseTracks[0].Artwork != looseTracks[1].Artwork, "Unrelated songs keep separate embedded artwork instead of their folder cover");
    Assert(PngSize(looseTracks[0].Artwork) == (512, 256) && PngSize(looseTracks[1].Artwork) == (100, 200), "Cache thumbnails are bounded and preserve aspect ratio");
    Assert(looseTracks.All(t => t.Artwork.StartsWith(Path.ChangeExtension(db, "artwork"), StringComparison.OrdinalIgnoreCase)), "Artwork is written only under the app database cache directory");
    core.SetFolderKind(albumFolder.Id, FolderKind.LooseTracks);
    Assert(core.Folder(albumFolder.Id).Folders.Single().Artwork.Length == 0 && core.Tracks(albumFolder.Id).Tracks[0].Artwork == looseTracks[0].Artwork, "Presentation override restores per-track covers with content deduplication");
    core.SetFolderKind(albumFolder.Id, FolderKind.Auto);
    await Wait(() => (core.LibraryState().Flags & 1) != 0, "Artwork roots monitored");
    if (audio) { core.Command(6, 0.1); core.Command(7); core.Play(albumFolder.Id, albumTracks[0].Id); await Wait(() => core.Now().Playing, "Tagged artwork WAV remains playable"); }
    var beforePosition = core.Now().Position;
    File.WriteAllBytes(Path.Combine(album, "COVER.PNG"), Png(90, 90, 190, 190, 0));
    await Wait(() => core.Folder(albumFolder.Id).Folders.Single().Artwork != albumFolder.Artwork, "External folder-cover edit replaces cache without changing audio tags");
    core.RefreshQueue();
    if (audio) { Assert(core.Now().Playing && core.Now().Position >= beforePosition, "Artwork refresh preserves the playing source"); core.Command(2); }
    File.Delete(looseTracks[0].Artwork); core.Rescan(); Assert(File.Exists(core.Tracks(looseFolder.Id).Tracks[0].Artwork), "Missing thumbnail is rebuilt from embedded artwork");
    for (int i = 0; i < files.Length; i++) Assert(hashes[i].SequenceEqual(SHA256.HashData(File.ReadAllBytes(files[i]))), "Artwork indexing preserves music bytes " + i);
    if (ffmpeg is not null)
    {
        var singles = Path.Combine(root, "MP3 single"); Directory.CreateDirectory(singles);
        var mp3 = Path.Combine(singles, "Song.mp3"); ConvertAudio(ffmpeg, files[2], mp3, "libmp3lame"); EmbedArtwork(mp3, blue, 1, "Another tagged release");
        var bytes = SHA256.HashData(File.ReadAllBytes(mp3)); core.Rescan();
        var folder = core.Folders(rootId).Folders.Single(f => f.Name == "MP3 single"); var track = core.Tracks(folder.Id).Tracks.Single();
        Assert(folder.Kind == FolderKind.LooseTracks && folder.Artwork.Length == 0 && track.Artwork == looseTracks[1].Artwork, "A single MP3 keeps its own ID3 artwork without becoming an album");
        if (audio) { core.Selection(new[] { track }, 0); await Wait(() => core.Now().Playing && core.Now().Position > 0.15, "MP3 with embedded artwork decodes through WASAPI"); core.Command(2); }
        Assert(bytes.SequenceEqual(SHA256.HashData(File.ReadAllBytes(mp3))), "MP3 artwork extraction preserves the original file");
    }
}
static (int, int) PngSize(string path)
{
    var bytes = File.ReadAllBytes(path);
    return (System.Buffers.Binary.BinaryPrimitives.ReadInt32BigEndian(bytes.AsSpan(16, 4)), System.Buffers.Binary.BinaryPrimitives.ReadInt32BigEndian(bytes.AsSpan(20, 4)));
}
static byte[] Png(int width, int height, byte red, byte green, byte blue)
{
    using var result = new MemoryStream(); result.Write(new byte[] { 137, 80, 78, 71, 13, 10, 26, 10 });
    var header = new byte[13]; System.Buffers.Binary.BinaryPrimitives.WriteInt32BigEndian(header.AsSpan(0, 4), width); System.Buffers.Binary.BinaryPrimitives.WriteInt32BigEndian(header.AsSpan(4, 4), height); header[8] = 8; header[9] = 2;
    PngChunk(result, "IHDR", header);
    using var pixels = new MemoryStream(); using (var z = new System.IO.Compression.ZLibStream(pixels, System.IO.Compression.CompressionLevel.Fastest, true))
    {
        for (int y = 0; y < height; y++) { z.WriteByte(0); for (int x = 0; x < width; x++) { z.WriteByte(red); z.WriteByte(green); z.WriteByte(blue); } }
    }
    PngChunk(result, "IDAT", pixels.ToArray()); PngChunk(result, "IEND", []); return result.ToArray();
}
static void PngChunk(Stream stream, string type, byte[] data)
{
    var key = Encoding.ASCII.GetBytes(type); var size = new byte[4]; System.Buffers.Binary.BinaryPrimitives.WriteInt32BigEndian(size, data.Length); stream.Write(size); stream.Write(key); stream.Write(data);
    uint crc = 0xffffffff;
    foreach (byte value in key.Concat(data)) { crc ^= value; for (int i = 0; i < 8; i++) crc = (crc & 1) != 0 ? (crc >> 1) ^ 0xedb88320 : crc >> 1; }
    System.Buffers.Binary.BinaryPrimitives.WriteUInt32BigEndian(size, ~crc); stream.Write(size);
}
static void EmbedArtwork(string path, byte[] png, uint number, string album)
{
    using var frames = new MemoryStream();
    foreach (var (key, value) in new[] { ("TIT2", "Artwork song " + number), ("TPE1", "Fixture artist"), ("TALB", album), ("TRCK", number.ToString()) })
        Id3Frame(frames, key, new byte[] { 3 }.Concat(Encoding.UTF8.GetBytes(value)).ToArray());
    Id3Frame(frames, "APIC", new byte[] { 3 }.Concat(Encoding.ASCII.GetBytes("image/png\0")).Concat(new byte[] { 3, 0 }).Concat(png).ToArray());
    using var id3 = new MemoryStream(); id3.Write(Encoding.ASCII.GetBytes("ID3")); id3.Write(new byte[] { 4, 0, 0 }); id3.Write(SyncSafe((int)frames.Length)); id3.Write(frames.ToArray());
    var wav = File.ReadAllBytes(path);
    if (Path.GetExtension(path).Equals(".mp3", StringComparison.OrdinalIgnoreCase))
    {
        int offset = 0;
        if (wav.Length >= 10 && Encoding.ASCII.GetString(wav, 0, 3) == "ID3") offset = 10 + (wav[6] << 21 | wav[7] << 14 | wav[8] << 7 | wav[9]);
        File.WriteAllBytes(path, id3.ToArray().Concat(wav.Skip(offset)).ToArray()); return;
    }
    using var result = new MemoryStream(); using var writer = new BinaryWriter(result, Encoding.ASCII, true);
    writer.Write(wav); writer.Write(Encoding.ASCII.GetBytes("id3 ")); writer.Write((int)id3.Length); writer.Write(id3.ToArray()); if (id3.Length % 2 != 0) writer.Write((byte)0);
    result.Position = 4; writer.Write((int)result.Length - 8); writer.Flush(); File.WriteAllBytes(path, result.ToArray());
}
static void Id3Frame(Stream stream, string name, byte[] data) { stream.Write(Encoding.ASCII.GetBytes(name)); stream.Write(SyncSafe(data.Length)); stream.Write(new byte[2]); stream.Write(data); }
static byte[] SyncSafe(int size) => new[] { (byte)((size >> 21) & 127), (byte)((size >> 14) & 127), (byte)((size >> 7) & 127), (byte)(size & 127) };
