using System.Runtime.InteropServices;
namespace Spiewak.Native;

public enum MetadataField : uint { Title = 1, Artist, Album, AlbumArtist, TrackNumber, TrackTotal, DiscNumber, DiscTotal, Date, Genre, Composer, Comment, Artwork, FileName, ReplayGainTrack, ReplayGainAlbum, ReplayPeakTrack, ReplayPeakAlbum }
public enum OperationState : uint { Current, Prepared, Applying, Applied, Undoing, Undone, Conflict, Cancelled }
public sealed record MetadataValue(MetadataField Field, string Value);
public sealed record MetadataRecord(long OperationId, long TrackId, OperationState State, long Created,
    string OriginalPath, string TargetPath, string Message, IReadOnlyList<MetadataValue> Values);
internal static partial class NativeMethods
{
    [StructLayout(LayoutKind.Sequential)] internal struct MetadataFieldRow { public uint Field; public nint Value; }
    [DllImport(Dll, EntryPoint = "spiewak_metadata", CallingConvention = CallingConvention.Cdecl)] internal static extern nint Metadata(CoreHandle core, long id, uint view);
    [DllImport(Dll, EntryPoint = "spiewak_metadata_prepare", CallingConvention = CallingConvention.Cdecl)] internal static extern nint PrepareMetadata(CoreHandle core, long track, [In] MetadataFieldRow[] fields, nuint count);
    [DllImport(Dll, EntryPoint = "spiewak_metadata_command", CallingConvention = CallingConvention.Cdecl)] internal static extern nint MetadataCommand(CoreHandle core, long operation, uint action);
    [DllImport(Dll, EntryPoint = "spiewak_metadata_free", CallingConvention = CallingConvention.Cdecl)] internal static extern void FreeMetadata(nint snapshot);
}
public sealed partial class Core
{
    [StructLayout(LayoutKind.Sequential)] private struct MetadataHeader { public nint Error, Rows; public nuint Count; }
    [StructLayout(LayoutKind.Sequential)] private struct MetadataRow
    {
        public long OperationId, TrackId; public uint State; public long Created;
        public nint OriginalPath, TargetPath, Message, Fields; public nuint FieldCount;
    }
    private static IReadOnlyList<MetadataRecord> ReadMetadata(nint pointer)
    {
        if (pointer == 0) throw new InvalidOperationException("The native core returned no metadata result.");
        try
        {
            var header = Marshal.PtrToStructure<MetadataHeader>(pointer);
            var error = Text(header.Error); if (error.Length > 0) throw new InvalidOperationException(error);
            var records = new List<MetadataRecord>();
            for (nuint i = 0; i < header.Count; i++)
            {
                var row = Marshal.PtrToStructure<MetadataRow>(header.Rows + checked((nint)i * Marshal.SizeOf<MetadataRow>()));
                var fields = new List<MetadataValue>();
                for (nuint j = 0; j < row.FieldCount; j++)
                {
                    var field = Marshal.PtrToStructure<NativeMethods.MetadataFieldRow>(row.Fields + checked((nint)j * Marshal.SizeOf<NativeMethods.MetadataFieldRow>()));
                    fields.Add(new((MetadataField)field.Field, Text(field.Value)));
                }
                records.Add(new(row.OperationId, row.TrackId, (OperationState)row.State, row.Created, Text(row.OriginalPath), Text(row.TargetPath), Text(row.Message), fields));
            }
            return records;
        }
        finally { NativeMethods.FreeMetadata(pointer); }
    }
    public MetadataRecord Metadata(long track) => ReadMetadata(NativeMethods.Metadata(handle, track, 0)).Single();
    public IReadOnlyList<MetadataRecord> MetadataOperations() => ReadMetadata(NativeMethods.Metadata(handle, 0, 1));
    public IReadOnlyList<MetadataValue> CleanupSuggestion(long track) => ReadMetadata(NativeMethods.Metadata(handle, track, 2)).Single().Values;
    public IReadOnlyList<MetadataRecord> PrepareMetadata(long track, IEnumerable<MetadataValue> values)
    {
        var edits = values.Take(19).ToArray();
        if (edits.Length is < 1 or > 18) throw new ArgumentException("Choose between 1 and 18 metadata fields.");
        var rows = new NativeMethods.MetadataFieldRow[edits.Length];
        try
        {
            for (var i = 0; i < edits.Length; i++) rows[i] = new() { Field = (uint)edits[i].Field, Value = Marshal.StringToCoTaskMemUTF8(edits[i].Value) };
            return ReadMetadata(NativeMethods.PrepareMetadata(handle, track, rows, (nuint)rows.Length));
        }
        finally { foreach (var row in rows) Marshal.FreeCoTaskMem(row.Value); }
    }
    public MetadataRecord ApplyMetadata(long operation) => ReadMetadata(NativeMethods.MetadataCommand(handle, operation, 1)).Single();
    public MetadataRecord UndoMetadata(long operation) => ReadMetadata(NativeMethods.MetadataCommand(handle, operation, 2)).Single();
    public void CancelMetadata(long operation) => ReadMetadata(NativeMethods.MetadataCommand(handle, operation, 3));
    public MetadataRecord RecoverMetadata(long operation) => ReadMetadata(NativeMethods.MetadataCommand(handle, operation, 4)).Single();
}
