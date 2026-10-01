using Spiewak.Native;
using System.Runtime.InteropServices;
using System.Text;
using System.Security.Cryptography;

if (args.Length < 1) throw new ArgumentException("Usage: Core.Smoke <absolute Rust DLL path> [--audio]");
var dll = Path.GetFullPath(args[0]);
NativeLibrary.SetDllImportResolver(typeof(Core).Assembly, (name, _, _) => name == "spiewak_core" ? NativeLibrary.Load(dll) : 0);
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
        var moved = Path.Combine(album, "03 - Renamed.wav"); File.Move(two, moved); core.Rescan(); Assert(core.Tracks(folderId).Tracks.Count == 2, "External rename reconciles");
        Assert(core.Search("renam").Tracks.Count == 1, "Renamed filename updates FTS");
        File.Delete(moved);
    }
    using (var core = new Core(db)) { Assert(core.Playlists().Playlists.Single().Count == 2, "Playlist survives restart"); core.Rescan(); Assert(core.PlaylistTracks(playlistId).Tracks.Any(t => t.Id == 0), "Unavailable playlist entry remains visible"); Assert(core.Tracks(folderId).Tracks.Count == 1 && core.Tracks(folderId).Tracks[0].Id == trackId, "Cache persists and external delete reconciles after restart"); }
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
    foreach (var argument in new[] { "-hide_banner", "-loglevel", "error", "-i", source, "-c:a", codec, target }) start.ArgumentList.Add(argument);
    using var process = System.Diagnostics.Process.Start(start) ?? throw new Exception("Cannot start ffmpeg");
    var error = process.StandardError.ReadToEnd(); process.WaitForExit(); if (process.ExitCode != 0) throw new Exception("Fixture conversion failed: " + error);
}
