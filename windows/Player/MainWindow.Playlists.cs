using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using Spiewak.Native;
namespace Spiewak;
public partial class MainWindow
{
    private long currentPlaylist, deletedPlaylist;
    private async Task ReloadPlaylists()
    {
        if (core is null) return;
        var result = await Task.Run(() => core.Playlists()); if (closed) return;
        PlaylistList.ItemsSource = result.Playlists;
        AddToPlaylistMenu.Items.Clear();
        foreach (var playlist in result.Playlists)
        { var item = new MenuFlyoutItem { Text = playlist.Name, Tag = playlist.Id }; item.Click += async (_, _) => await AppendPlaylist((long)item.Tag); AddToPlaylistMenu.Items.Add(item); }
    }
    private async Task AppendPlaylist(long id)
    {
        if (core is null) return; var rows = tracks.Where(t => TrackList.SelectedItems.Contains(t)).ToArray(); if (rows.Length == 0) return;
        try { await Task.Run(() => core.PlaylistCommand(id, 3, rows: rows)); await ReloadPlaylists(); if (currentPlaylist == id) await ShowPlaylist(id); } catch (Exception e) { Error(e); }
    }
    private async void PlaylistClicked(object sender, ItemClickEventArgs args) { if (args.ClickedItem is Playlist playlist) { try { await ShowPlaylist(playlist.Id); } catch (Exception e) { Error(e); } } }
    private async Task ShowPlaylist(long id)
    {
        if (core is null) return;
        changingSearch = true; SearchBox.Text = ""; changingSearch = false; searchTimer.Stop(); var version = ++navigationVersion;
        var result = await Task.Run(() => core.PlaylistTracks(id)); var lists = await Task.Run(() => core.Playlists()); if (closed || version != navigationVersion) return;
        if (currentFolder is not null && currentPlaylist == 0) history.Push(currentFolder);
        var playlist = lists.Playlists.Single(p => p.Id == id); InboxTools.Visibility = Visibility.Collapsed; currentListening = -1; currentPlaylist = id; currentFolder = null;
        tracks.Clear(); foreach (var track in result.Tracks) tracks.Add(track);
        FolderViewButton.Visibility = Visibility.Collapsed; SetFolderArtwork(false);
        ChildFolders.Visibility = Visibility.Collapsed; FolderArt.Source = null; FolderHeading.Text = playlist.Name; FolderSummary.Text = $"Playlist · {playlist.Count} tracks"; Breadcrumb.Text = "Playlists / " + playlist.Name;
        EmptyState.Text = "Add tracks using their context menu, or save your queue here."; EmptyState.Visibility = tracks.Count == 0 ? Visibility.Visible : Visibility.Collapsed;
        MoreButton.Visibility = tracks.Count == 500 ? Visibility.Visible : Visibility.Collapsed; PlayFolderButton.Content = "▶ Play playlist"; PlayFolderButton.IsEnabled = tracks.Count > 0; PlaylistTools.Visibility = Visibility.Visible; SortBox.Visibility = Visibility.Collapsed;
        BackButton.IsEnabled = history.Count > 0;
    }
    private async Task MorePlaylist()
    {
        if (core is null) return; var id = currentPlaylist; var version = navigationVersion;
        try { var result = await Task.Run(() => core.PlaylistTracks(id, (uint)tracks.Count)); if (closed || version != navigationVersion) return; foreach (var row in result.Tracks) tracks.Add(row); MoreButton.Visibility = result.Tracks.Count == 500 ? Visibility.Visible : Visibility.Collapsed; } catch (Exception e) { Error(e); }
    }
    private async Task<string?> PlaylistName(string title, string initial = "")
    {
        var input = new TextBox { Text = initial, PlaceholderText = "Playlist name", MaxLength = 100 };
        var dialog = new ContentDialog { XamlRoot = Shell.XamlRoot, RequestedTheme = Shell.ActualTheme, Title = title, Content = input, PrimaryButtonText = "Save", CloseButtonText = "Cancel", DefaultButton = ContentDialogButton.Primary };
        return await dialog.ShowAsync() == ContentDialogResult.Primary ? input.Text : null;
    }
    private async void CreatePlaylist(object sender, RoutedEventArgs args) => await NewPlaylist(null);
    private async void SaveQueue(object sender, RoutedEventArgs args)
    { if (core is null) return; try { var rows = (await Task.Run(() => core.Queue())).Tracks; await NewPlaylist(rows); } catch (Exception e) { Error(e); } }
    private async Task NewPlaylist(IReadOnlyList<Track>? rows)
    {
        if (core is null) return; var name = await PlaylistName(rows is null ? "Create playlist" : "Save queue as playlist"); if (name is null) return;
        try { var result = await Task.Run(() => rows is null ? core.PlaylistCommand(0, 0, name) : core.SaveQueuePlaylist(name)); await ReloadPlaylists(); await ShowPlaylist(result.CurrentId); } catch (Exception e) { Error(e); }
    }
    private async void RenamePlaylist(object sender, RoutedEventArgs args)
    {
        if (core is null || PlaylistList.SelectedItem is not Playlist playlist) return; var name = await PlaylistName("Rename playlist", playlist.Name); if (name is null) return;
        try { await Task.Run(() => core.PlaylistCommand(playlist.Id, 1, name)); await ReloadPlaylists(); if (currentPlaylist == playlist.Id) await ShowPlaylist(playlist.Id); } catch (Exception e) { Error(e); }
    }
    private async void DeletePlaylist(object sender, RoutedEventArgs args)
    {
        if (core is null || PlaylistList.SelectedItem is not Playlist playlist) return;
        var dialog = new ContentDialog { XamlRoot = Shell.XamlRoot, RequestedTheme = Shell.ActualTheme, Title = "Delete ‘" + playlist.Name + "’?", Content = "Music files remain in their folders.", PrimaryButtonText = "Delete playlist", CloseButtonText = "Cancel" };
        if (await dialog.ShowAsync() != ContentDialogResult.Primary) return;
        try { await Task.Run(() => core.PlaylistCommand(playlist.Id, 2)); deletedPlaylist = playlist.Id; RestorePlaylistButton.Visibility = Visibility.Visible; await ReloadPlaylists(); if (currentPlaylist == playlist.Id) { currentPlaylist = 0; tracks.Clear(); PlaylistTools.Visibility = Visibility.Collapsed; FolderHeading.Text = "Playlist deleted"; EmptyState.Text = "Choose a folder or playlist."; EmptyState.Visibility = Visibility.Visible; PlayFolderButton.IsEnabled = false; } } catch (Exception e) { Error(e); }
    }
    private async void RestorePlaylist(object sender, RoutedEventArgs args)
    { if (core is null || deletedPlaylist == 0) return; try { await Task.Run(() => core.PlaylistCommand(deletedPlaylist, 7)); await ReloadPlaylists(); await ShowPlaylist(deletedPlaylist); deletedPlaylist = 0; RestorePlaylistButton.Visibility = Visibility.Collapsed; } catch (Exception e) { Error(e); } }
    private async Task EditPlaylist(uint command)
    {
        if (core is null || currentPlaylist == 0 || TrackList.SelectedIndex < 0) return; var id = currentPlaylist;
        var indexes = command == 4 ? Enumerable.Range(0, tracks.Count).Where(i => TrackList.SelectedItems.Contains(TrackList.Items[i])).OrderDescending().ToArray() : new[] { TrackList.SelectedIndex };
        try { foreach (var index in indexes) await Task.Run(() => core.PlaylistCommand(id, command, index: (uint)index)); await ReloadPlaylists(); await ShowPlaylist(id); } catch (Exception e) { Error(e); }
    }
    private async void PlaylistUp(object sender, RoutedEventArgs args) => await EditPlaylist(5);
    private async void PlaylistDown(object sender, RoutedEventArgs args) => await EditPlaylist(6);
    private async void PlaylistRemove(object sender, RoutedEventArgs args) => await EditPlaylist(4);
}
