using System.Runtime.InteropServices;
using Microsoft.Win32.SafeHandles;
namespace Spiewak.Native;

public sealed record Folder(long Id, long Parent, string Name, string Path, bool HasChildren)
{
    public override string ToString() => Name;
}
public sealed record Track(long Id, long Folder, string Title, string Artist, string Album, string Path,
    string Artwork, string Codec, double Duration, uint Number, uint Year)
{
    public string Time => Duration > 0 ? FormatTime(Duration) : "—";
    public string TrackNumber => Number > 0 ? Number.ToString("00") : "·";
    public string FileName => System.IO.Path.GetFileName(Path);
    public string ArtistLabel => string.IsNullOrEmpty(Artist) ? "Unknown artist" : Artist;
    public static string FormatTime(double seconds) => TimeSpan.FromSeconds(Math.Max(0, seconds)).ToString(seconds >= 3600 ? @"h\:mm\:ss" : @"m\:ss");
}
public sealed record Result(IReadOnlyList<Folder> Folders, IReadOnlyList<Track> Tracks, double Position,
    long CurrentId, long CurrentIndex, ulong Revision, bool Playing, float Volume, uint Flags, string Error, string Message);

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
    [DllImport(Dll, EntryPoint = "spiewak_tracks", CallingConvention = CallingConvention.Cdecl)] internal static extern nint Tracks(CoreHandle core, long folder, uint offset, uint limit);
    [DllImport(Dll, EntryPoint = "spiewak_add_root", CallingConvention = CallingConvention.Cdecl)] internal static extern nint AddRoot(CoreHandle core, [MarshalAs(UnmanagedType.LPUTF8Str)] string path);
    [DllImport(Dll, EntryPoint = "spiewak_rescan", CallingConvention = CallingConvention.Cdecl)] internal static extern nint Rescan(CoreHandle core);
    [DllImport(Dll, EntryPoint = "spiewak_play_folder", CallingConvention = CallingConvention.Cdecl)] internal static extern nint Play(CoreHandle core, long folder, long track);
    [DllImport(Dll, EntryPoint = "spiewak_audio_command", CallingConvention = CallingConvention.Cdecl)] internal static extern nint Command(CoreHandle core, uint command, double value);
    [DllImport(Dll, EntryPoint = "spiewak_now", CallingConvention = CallingConvention.Cdecl)] internal static extern nint Now(CoreHandle core);
    [DllImport(Dll, EntryPoint = "spiewak_queue", CallingConvention = CallingConvention.Cdecl)] internal static extern nint Queue(CoreHandle core);
    [DllImport(Dll, EntryPoint = "spiewak_selection", CallingConvention = CallingConvention.Cdecl)] internal static extern nint Selection(CoreHandle core, [In] long[] ids, nuint count, uint mode);
}
public sealed class Core : IDisposable
{
    private readonly CoreHandle handle;
    [StructLayout(LayoutKind.Sequential)]
    private struct Header
    {
        public nint Folders; public nuint FolderCount; public nint Tracks; public nuint TrackCount;
        public double Position; public long CurrentId; public long CurrentIndex; public ulong Revision;
        public uint Playing; public float Volume; public uint Flags; public nint Error; public nint Message;
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
    public Core(string databasePath)
    {
        if (NativeMethods.Version() != 2 || Marshal.SizeOf<FolderRow>() != 40 || Marshal.SizeOf<TrackRow>() != 80 || Marshal.SizeOf<Header>() != 96)
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
            return new(folders, tracks, header.Position, header.CurrentId, header.CurrentIndex, header.Revision, header.Playing != 0, header.Volume, header.Flags, Text(header.Error), Text(header.Message));
        }
        finally { NativeMethods.Free(pointer); }
    }
    private static Result Checked(nint pointer)
    { var result = Read(pointer); if (result.Error.Length > 0) throw new InvalidOperationException(result.Error); return result; }
    public Result Folders(long parent = 0) => Checked(NativeMethods.Folders(handle, parent));
    public Result Tracks(long folder, uint offset = 0) => Checked(NativeMethods.Tracks(handle, folder, offset, 500));
    public Result AddRoot(string path) => Checked(NativeMethods.AddRoot(handle, path));
    public Result Rescan() => Checked(NativeMethods.Rescan(handle));
    public Result Play(long folder, long track) => Checked(NativeMethods.Play(handle, folder, track));
    public Result Command(uint command, double value = 0) => Checked(NativeMethods.Command(handle, command, value));
    public Result Now() => Read(NativeMethods.Now(handle));
    public Result Queue() => Checked(NativeMethods.Queue(handle));
    public Result Selection(IEnumerable<Track> tracks, uint mode) { var ids = tracks.Select(t => t.Id).ToArray(); return Checked(NativeMethods.Selection(handle, ids, (nuint)ids.Length, mode)); }
    public void Dispose() => handle.Dispose();
}
