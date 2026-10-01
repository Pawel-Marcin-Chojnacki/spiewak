using System.Runtime.InteropServices;
using Microsoft.Win32.SafeHandles;
namespace Spiewak.Native;

public sealed record Folder(long Id, long Parent, string Name, string Path, bool HasChildren)
{
    public override string ToString() => Name;
}
public sealed class Track(long id, long folder, string title, string artist, string album, string path,
    string artwork, string codec, double duration, uint number, uint year)
{
    public long Id { get; } = id; public long Folder { get; } = folder;
    public string Title { get; } = title; public string Artist { get; } = artist; public string Album { get; } = album;
    public string Path { get; } = path; public string Artwork { get; } = artwork; public string Codec { get; } = codec;
    public double Duration { get; } = duration; public uint Number { get; } = number; public uint Year { get; } = year;
    public string Time => Duration > 0 ? FormatTime(Duration) : "—";
    public string TrackNumber => Number > 0 ? Number.ToString("00") : "·";
    public string FileName => System.IO.Path.GetFileName(Path);
    public string ArtistLabel => string.IsNullOrEmpty(Artist) ? "Unknown artist" : Artist;
    public static string FormatTime(double seconds) => TimeSpan.FromSeconds(Math.Max(0, seconds)).ToString(seconds >= 3600 ? @"h\:mm\:ss" : @"m\:ss");
}
public sealed record Playlist(long Id, string Name, ulong Count) { public override string ToString() => $"{Name} · {Count}"; }
public sealed record Result(IReadOnlyList<Folder> Folders, IReadOnlyList<Track> Tracks, double Position,
    long CurrentId, long CurrentIndex, ulong Revision, bool Playing, float Volume, uint Flags, string Error, string Message, IReadOnlyList<Playlist> Playlists);

internal sealed class CoreHandle : SafeHandleZeroOrMinusOneIsInvalid
{
    public CoreHandle(nint handle) : base(true) { SetHandle(handle); }
    protected override bool ReleaseHandle() { NativeMethods.Close(handle); return true; }
}
internal static class NativeMethods
{
    private const string Dll = "spiewak_core";
    [DllImport(Dll, EntryPoint = "spiewak_abi_version", CallingConvention = CallingConvention.Cdecl)] internal static extern uint Version();
    [DllImport(Dll, EntryPoint = "spiewak_open", CallingConvention = CallingConvention.Cdecl)] internal static extern nint Open([MarshalAs(UnmanagedType.LPUTF8Str)] string path);
    [DllImport(Dll, EntryPoint = "spiewak_close", CallingConvention = CallingConvention.Cdecl)] internal static extern void Close(nint handle);
    [DllImport(Dll, EntryPoint = "spiewak_snapshot_free", CallingConvention = CallingConvention.Cdecl)] internal static extern void Free(nint snapshot);
    [DllImport(Dll, EntryPoint = "spiewak_folders", CallingConvention = CallingConvention.Cdecl)] internal static extern nint Folders(CoreHandle core, long parent);
    [DllImport(Dll, EntryPoint = "spiewak_library_state", CallingConvention = CallingConvention.Cdecl)] internal static extern nint LibraryState(CoreHandle core);
    [DllImport(Dll, EntryPoint = "spiewak_folder", CallingConvention = CallingConvention.Cdecl)] internal static extern nint Folder(CoreHandle core, long id);
    [DllImport(Dll, EntryPoint = "spiewak_refresh_queue", CallingConvention = CallingConvention.Cdecl)] internal static extern nint RefreshQueue(CoreHandle core);
    [DllImport(Dll, EntryPoint = "spiewak_tracks", CallingConvention = CallingConvention.Cdecl)] internal static extern nint Tracks(CoreHandle core, long folder, uint offset, uint limit);
    [DllImport(Dll, EntryPoint = "spiewak_add_root", CallingConvention = CallingConvention.Cdecl)] internal static extern nint AddRoot(CoreHandle core, [MarshalAs(UnmanagedType.LPUTF8Str)] string path);
    [DllImport(Dll, EntryPoint = "spiewak_rescan", CallingConvention = CallingConvention.Cdecl)] internal static extern nint Rescan(CoreHandle core);
    [DllImport(Dll, EntryPoint = "spiewak_play_folder", CallingConvention = CallingConvention.Cdecl)] internal static extern nint Play(CoreHandle core, long folder, long track);
    [DllImport(Dll, EntryPoint = "spiewak_audio_command", CallingConvention = CallingConvention.Cdecl)] internal static extern nint Command(CoreHandle core, uint command, double value);
    [DllImport(Dll, EntryPoint = "spiewak_now", CallingConvention = CallingConvention.Cdecl)] internal static extern nint Now(CoreHandle core);
    [DllImport(Dll, EntryPoint = "spiewak_queue", CallingConvention = CallingConvention.Cdecl)] internal static extern nint Queue(CoreHandle core);
    [DllImport(Dll, EntryPoint = "spiewak_selection", CallingConvention = CallingConvention.Cdecl)] internal static extern nint Selection(CoreHandle core, [In] long[] ids, nuint count, uint mode);
    [DllImport(Dll, EntryPoint = "spiewak_search", CallingConvention = CallingConvention.Cdecl)] internal static extern nint Search(CoreHandle core, [MarshalAs(UnmanagedType.LPUTF8Str)] string query);
    [DllImport(Dll, EntryPoint = "spiewak_tracks_sorted", CallingConvention = CallingConvention.Cdecl)] internal static extern nint Sorted(CoreHandle core, long folder, uint offset, uint limit, uint sort);
    [DllImport(Dll, EntryPoint = "spiewak_play_view", CallingConvention = CallingConvention.Cdecl)] internal static extern nint PlayView(CoreHandle core, long folder, long track, uint sort);

    [DllImport(Dll, EntryPoint = "spiewak_playlists", CallingConvention = CallingConvention.Cdecl)] internal static extern nint Playlists(CoreHandle core);
    [DllImport(Dll, EntryPoint = "spiewak_playlist_tracks", CallingConvention = CallingConvention.Cdecl)] internal static extern nint PlaylistTracks(CoreHandle core, long id, uint offset);
    [DllImport(Dll, EntryPoint = "spiewak_play_playlist", CallingConvention = CallingConvention.Cdecl)] internal static extern nint PlayPlaylist(CoreHandle core, long id, uint index);
    [DllImport(Dll, EntryPoint = "spiewak_playlist_command", CallingConvention = CallingConvention.Cdecl)] internal static extern nint PlaylistCommand(CoreHandle core, long id, uint command, [MarshalAs(UnmanagedType.LPUTF8Str)] string name, [In] long[] ids, nuint count, uint index);

}
public sealed class Core : IDisposable
{
    private readonly CoreHandle handle;
    [StructLayout(LayoutKind.Sequential)]
    private struct Header
    {
        public nint Folders; public nuint FolderCount; public nint Tracks; public nuint TrackCount;
        public double Position; public long CurrentId; public long CurrentIndex; public ulong Revision;
        public uint Playing; public float Volume; public uint Flags; public nint Error; public nint Message; public nint Playlists; public nuint PlaylistCount;
    }
    [StructLayout(LayoutKind.Sequential)]
    private struct FolderRow
    { public long Id, Parent; public nint Name, Path; public uint HasChildren; }
    [StructLayout(LayoutKind.Sequential)]
    private struct TrackRow
    {
        public long Id, Folder; public nint Title, Artist, Album, Path, Artwork, Codec;
        public double Duration; public uint Number, Year;
    }
    [StructLayout(LayoutKind.Sequential)] private struct PlaylistRow { public long Id; public nint Name; public ulong Count; }
    public Core(string databasePath)
    {
        if (NativeMethods.Version() != 3 || Marshal.SizeOf<FolderRow>() != 40 || Marshal.SizeOf<TrackRow>() != 80 || Marshal.SizeOf<Header>() != 112)
            throw new InvalidOperationException("The native core has an incompatible ABI. Rebuild the application.");
        handle = new CoreHandle(NativeMethods.Open(databasePath));
        if (handle.IsInvalid) throw new IOException($"Cannot open the library cache at {databasePath}. Check folder permissions.");
    }
    private static string Text(nint pointer) => pointer == 0 ? "" : Marshal.PtrToStringUTF8(pointer) ?? "";
    private static Result Read(nint pointer)
    {
        if (pointer == 0) throw new InvalidOperationException("The native core returned no result.");
        try
        {
            var header = Marshal.PtrToStructure<Header>(pointer);
            var folders = new List<Folder>(); var tracks = new List<Track>();
            for (nuint i = 0; i < header.FolderCount; i++)
            {
                var row = Marshal.PtrToStructure<FolderRow>(header.Folders + checked((nint)i * Marshal.SizeOf<FolderRow>()));
                folders.Add(new(row.Id, row.Parent, Text(row.Name), Text(row.Path), row.HasChildren != 0));
            }
            for (nuint i = 0; i < header.TrackCount; i++)
            {
                var row = Marshal.PtrToStructure<TrackRow>(header.Tracks + checked((nint)i * Marshal.SizeOf<TrackRow>()));
                tracks.Add(new(row.Id, row.Folder, Text(row.Title), Text(row.Artist), Text(row.Album), Text(row.Path), Text(row.Artwork), Text(row.Codec), row.Duration, row.Number, row.Year));
            }
            var playlists = new List<Playlist>();
            for (nuint i = 0; i < header.PlaylistCount; i++) { var row = Marshal.PtrToStructure<PlaylistRow>(header.Playlists + checked((nint)i * Marshal.SizeOf<PlaylistRow>())); playlists.Add(new(row.Id, Text(row.Name), row.Count)); }
            return new(folders, tracks, header.Position, header.CurrentId, header.CurrentIndex, header.Revision, header.Playing != 0, header.Volume, header.Flags, Text(header.Error), Text(header.Message), playlists);
        }
        finally { NativeMethods.Free(pointer); }
    }
    private static Result Checked(nint pointer)
    { var result = Read(pointer); if (result.Error.Length > 0) throw new InvalidOperationException(result.Error); return result; }
    public Result Folders(long parent = 0) => Checked(NativeMethods.Folders(handle, parent));
    public Result Tracks(long folder, uint offset = 0, uint sort = 0) => Checked(NativeMethods.Sorted(handle, folder, offset, 500, sort));
    public Result LibraryState() => Checked(NativeMethods.LibraryState(handle));
    public Result Folder(long id) => Checked(NativeMethods.Folder(handle, id));
    public Result RefreshQueue() => Checked(NativeMethods.RefreshQueue(handle));
    public Result AddRoot(string path) => Checked(NativeMethods.AddRoot(handle, path));
    public Result Rescan() => Checked(NativeMethods.Rescan(handle));
    public Result Play(long folder, long track, uint sort = 0) => Checked(NativeMethods.PlayView(handle, folder, track, sort));
    public Result Search(string query) => Checked(NativeMethods.Search(handle, query));
    public Result Command(uint command, double value = 0) => Checked(NativeMethods.Command(handle, command, value));
    public Result Now() => Read(NativeMethods.Now(handle));
    public Result Queue() => Checked(NativeMethods.Queue(handle));
    public Result Selection(IEnumerable<Track> tracks, uint mode) { var ids = tracks.Select(t => t.Id).ToArray(); return Checked(NativeMethods.Selection(handle, ids, (nuint)ids.Length, mode)); }
    public Result Playlists() => Checked(NativeMethods.Playlists(handle));
    public Result PlaylistTracks(long id, uint offset = 0) => Checked(NativeMethods.PlaylistTracks(handle, id, offset));
    public Result PlayPlaylist(long id, uint index) => Checked(NativeMethods.PlayPlaylist(handle, id, index));
    public Result PlaylistCommand(long id, uint command, string name = "", IEnumerable<Track>? rows = null, uint index = 0) { var ids = rows?.Select(t => t.Id).ToArray() ?? []; return Checked(NativeMethods.PlaylistCommand(handle, id, command, name, ids, (nuint)ids.Length, index)); }
    public void Dispose() => handle.Dispose();
}
