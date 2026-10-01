using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Input;
using Spiewak.Native;
namespace Spiewak;
public partial class MainWindow
{
    private int currentListening = -1;
    private ulong listeningRevision;
    private Track? playingTrack;
    private async void ListeningClicked(object sender, RoutedEventArgs args)
    {
        if (sender is not Button button) return;
        try { await ShowListening(uint.Parse((string)button.Tag)); } catch (Exception e) { Error(e); }
    }
    private async Task ShowListening(uint view)
    {
        if (core is null) return;
        changingSearch = true; SearchBox.Text = ""; changingSearch = false; searchTimer.Stop();
        var version = ++navigationVersion;
        var result = await Task.Run(() => core.ListeningTracks(view));
        var inbox = view == 3 ? await Task.Run(() => core.Inbox()) : null;
        if (closed || version != navigationVersion) return;
        InboxTools.Visibility = view == 3 ? Visibility.Visible : Visibility.Collapsed;
        if (currentFolder is not null && currentListening < 0) history.Push(currentFolder);
        currentListening = (int)view; currentPlaylist = 0; currentFolder = null; listeningRevision = result.Revision;
        tracks.Clear(); foreach (var track in result.Tracks) tracks.Add(track);
        PlaylistTools.Visibility = Visibility.Collapsed; SortBox.Visibility = Visibility.Collapsed; FolderViewButton.Visibility = Visibility.Collapsed;
        ChildFolders.Visibility = Visibility.Collapsed; SetFolderArtwork(false); FolderArt.Source = null;
        FolderHeading.Text = view switch { 0 => "Favorites", 1 => "Recently added", 2 => "Listening history", _ => "Inbox · Needs review" };
        FolderSummary.Text = inbox?.Message ?? $"{tracks.Count} entries loaded · newest first · files stay in their folders";
        Breadcrumb.Text = "Library / " + FolderHeading.Text; PlayFolderButton.Content = "▶ Play " + FolderHeading.Text.ToLowerInvariant(); PlayFolderButton.IsEnabled = tracks.Count > 0;
        EmptyState.Text = view switch { 0 => "Use ♥ on a track you enjoy, or select tracks and press Ctrl+D.", 1 => "Add a music folder to see your collection.", 2 => "Tracks appear here when playback starts successfully.", _ => "No new files need review. Browse a folder and choose Folder options → Use as Inbox. Only new files directly in that folder are included. Playback and your files stay unchanged." };
        EmptyState.Visibility = tracks.Count == 0 ? Visibility.Visible : Visibility.Collapsed;
        MoreButton.Visibility = tracks.Count == 500 ? Visibility.Visible : Visibility.Collapsed;
        BackButton.IsEnabled = history.Count > 0; ForwardButton.IsEnabled = forward.Count > 0; UpdateSelectedFavorite();
    }
    private async Task MoreListening()
    {
        if (core is null || currentListening < 0) return;
        var view = (uint)currentListening;
        var version = navigationVersion;
        MoreButton.IsEnabled = false;
        try
        {
            var result = await Task.Run(() => core.ListeningTracks(view, (uint)tracks.Count));
            if (closed || version != navigationVersion) return;
            foreach (var row in result.Tracks) tracks.Add(row);
            MoreButton.Visibility = result.Tracks.Count == 500 ? Visibility.Visible : Visibility.Collapsed;
            if (view != 3) FolderSummary.Text = $"{tracks.Count} entries loaded · newest first · files stay in their folders";
        }
        catch (Exception e) { Error(e); }
        finally { if (!closed) MoreButton.IsEnabled = true; }
    }
    private async Task PollListening()
    {
        if (core is null || currentListening != 2 || SearchBox.Text.Trim().Length > 0) return;
        var version = navigationVersion;
        var state = await Task.Run(() => core.ListeningState());
        if (!closed && version == navigationVersion && state.Revision != listeningRevision) await ShowListening(2);
    }
    private void UpdateSelectedFavorite()
    {
        var rows = TrackList.SelectedItems.Cast<Track>().ToArray();
        SelectedFavoriteButton.IsEnabled = rows.Length > 0 && rows.All(t => t.ReferenceId > 0);
        SelectedFavoriteButton.Content = rows.Length > 0 && rows.All(t => t.Favorite) ? "♥ Remove from favorites" : "♡ Add to favorites";
    }
    private async void ToggleSelectedFavorite(object sender, RoutedEventArgs args) => await FavoriteSelection();
    private async void FavoriteShortcut(KeyboardAccelerator sender, KeyboardAcceleratorInvokedEventArgs args)
    {
        if (metadataDialogOpen) return;
        if (FocusManager.GetFocusedElement(Shell.XamlRoot) is TextBox) return;
        args.Handled = true;
        await FavoriteSelection();
    }
    private async Task FavoriteSelection()
    {
        var rows = TrackList.SelectedItems.Cast<Track>().ToArray();
        if (rows.Length == 0) return;
        await SetFavorites(rows, !rows.All(t => t.Favorite));
    }
    private async void TogglePlayingFavorite(object sender, RoutedEventArgs args)
    {
        if (playingTrack is Track track && track.Id > 0) await SetFavorites(new[] { track }, !track.Favorite);
    }
    private async Task SetFavorites(Track[] rows, bool favorite)
    {
        if (core is null) return;
        try
        {
            await Task.Run(() => core.Favorites(rows, favorite));
            await RefreshLibraryView();
        }
        catch (Exception e) { Error(e); }
    }
}
