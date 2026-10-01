using Spiewak.Native;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
namespace Spiewak;
public partial class MainWindow
{
    private void SetFolderArtwork(bool album)
    {
        FolderHeader.ColumnSpacing = album ? 20 : 0;
        FolderArtFrame.Visibility = album ? Visibility.Visible : Visibility.Collapsed;
        FolderArtColumn.Width = new GridLength(album ? 96 : 0);
        FolderHeading.FontSize = album ? 28 : 22;
    }
    private async void FolderKindChanged(object sender, RoutedEventArgs args)
    {
        if (core is null || currentFolder is null || sender is not MenuFlyoutItem item) return;
        var id = currentFolder.Id; var version = navigationVersion;
        try
        {
            var kind = (FolderKind)uint.Parse((string)item.Tag);
            var result = await Task.Run(() => core.SetFolderKind(id, kind));
            if (closed || version != navigationVersion) return;
            await Browse(result.Folders.Single(), false); await ReloadTree();
        }
        catch (Exception e) { Error(e); }
    }
    private async Task PollLibrary()
    {
        if (core is null || closed) return;
        var state = await Task.Run(() => core.LibraryState()); if (closed) return;
        if (state.Revision == libraryRevision) return;
        libraryRevision = state.Revision; LibraryStatus.Text = state.Message;
        await RefreshLibraryView();
    }
    private async Task RefreshLibraryView()
    {
        if (core is null) return;
        var version = navigationVersion; var folder = currentFolder; var playlist = currentPlaylist;
        var selected = TrackList.SelectedItems.Cast<Track>().Select(t => t.Id).ToHashSet();
        var selectedOccurrences = tracks.Select((t,i) => (t,i)).Where(row => TrackList.SelectedItems.Contains(row.t)).Select(row => row.i).ToHashSet();
        await Task.Run(() => core.RefreshQueue()); await ReloadTree(); await ReloadPlaylists();
        if (closed || version != navigationVersion) return;
        if (SearchBox.Text.Trim().Length > 0) await RunSearch();
        else if (playlist != 0) await ShowPlaylist(playlist);
        else
        {
            var existing = folder is null ? null : (await Task.Run(() => core.Folder(folder.Id))).Folders.FirstOrDefault();
            if (closed || version != navigationVersion) return;
            if (existing is not null) await Browse(existing, false);
            else
            {
                var roots = await Task.Run(() => core.Folders());
                if (closed || version != navigationVersion) return;
                if (roots.Folders.FirstOrDefault() is Folder root) await Browse(root, false);
            }
        }
        if (!closed && playlist == currentPlaylist && (folder?.Id ?? 0) == (currentFolder?.Id ?? 0))
            for (var i = 0; i < tracks.Count; i++)
                if (playlist != 0 ? selectedOccurrences.Contains(i) : selected.Contains(tracks[i].Id)) TrackList.SelectedItems.Add(tracks[i]);
    }
}
