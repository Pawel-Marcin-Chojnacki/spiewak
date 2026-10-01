using System.Runtime.InteropServices;
namespace Spiewak.Native;

public sealed record MetadataCandidate(long Id, long TrackId, uint Flags, uint Score, double Duration, string Title, string Artist,
    string Album, string Date, string Edition, string Evidence, string Source, string Artwork, string Recording, string Release)
{
    public bool VersionCompatible => (Flags & 8) != 0;
    public string Summary => $"{Title} · {Artist}\n{(Album.Length > 0 ? Album : "Recording without a release")} · {Date} · {Edition}\n{(Duration > 0 ? TimeSpan.FromSeconds(Duration).ToString(@"m\:ss") : "Unknown duration")}\n{Evidence}";
}
public sealed record Discovery(bool Enabled, string Title, string Artist, string Message, IReadOnlyList<MetadataCandidate> Candidates);
internal static partial class NativeMethods
{
    [DllImport(Dll, EntryPoint = "spiewak_discovery", CallingConvention = CallingConvention.Cdecl)] internal static extern nint Discovery(CoreHandle core, long track, uint action,
        [MarshalAs(UnmanagedType.LPUTF8Str)] string title, [MarshalAs(UnmanagedType.LPUTF8Str)] string artist);
    [DllImport(Dll, EntryPoint = "spiewak_discovery_prepare", CallingConvention = CallingConvention.Cdecl)] internal static extern nint PrepareDiscovery(CoreHandle core, long candidate, uint replaceTags, uint replaceCover);
    [DllImport(Dll, EntryPoint = "spiewak_discovery_free", CallingConvention = CallingConvention.Cdecl)] internal static extern void FreeDiscovery(nint snapshot);
}
public sealed partial class Core
{
    [StructLayout(LayoutKind.Sequential)] private struct DiscoveryHeader { public nint Error, Message; public uint Enabled; public nint Rows; public nuint Count; public nint Title, Artist; }
    [StructLayout(LayoutKind.Sequential)] private struct CandidateRow
    {
        public long Id, TrackId; public uint Flags, Score; public double Duration;
        public nint Title, Artist, Album, Date, Edition, Evidence, Source, Artwork, Recording, Release;
    }
    private static Discovery ReadDiscovery(nint pointer)
    {
        if (pointer == 0) throw new InvalidOperationException("The native core returned no discovery result.");
        try
        {
            var header = Marshal.PtrToStructure<DiscoveryHeader>(pointer);
            var error = Text(header.Error); if (error.Length > 0) throw new InvalidOperationException(error);
            var candidates = new List<MetadataCandidate>();
            for (nuint i = 0; i < header.Count; i++)
            {
                var row = Marshal.PtrToStructure<CandidateRow>(header.Rows + checked((nint)i * Marshal.SizeOf<CandidateRow>()));
                candidates.Add(new(row.Id, row.TrackId, row.Flags, row.Score, row.Duration, Text(row.Title), Text(row.Artist), Text(row.Album), Text(row.Date), Text(row.Edition), Text(row.Evidence), Text(row.Source), Text(row.Artwork), Text(row.Recording), Text(row.Release)));
            }
            return new(header.Enabled != 0, Text(header.Title), Text(header.Artist), Text(header.Message), candidates);
        }
        finally { NativeMethods.FreeDiscovery(pointer); }
    }
    public Discovery Discovery(long track = 0) => ReadDiscovery(NativeMethods.Discovery(handle, track, 0, "", ""));
    public Discovery SetDiscoveryEnabled(bool enabled) => ReadDiscovery(NativeMethods.Discovery(handle, 0, enabled ? 2u : 3u, "", ""));
    public Discovery DiscoverMetadata(long track, string title, string artist) => ReadDiscovery(NativeMethods.Discovery(handle, track, 1, title, artist));
    public IReadOnlyList<MetadataRecord> PrepareDiscovery(long candidate, bool replaceTags = false, bool replaceCover = false)
        => ReadMetadata(NativeMethods.PrepareDiscovery(handle, candidate, replaceTags ? 1u : 0u, replaceCover ? 1u : 0u));
}
