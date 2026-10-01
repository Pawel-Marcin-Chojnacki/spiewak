using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Input;
using Spiewak.Native;
using Windows.System;
namespace Spiewak;
public partial class MainWindow
{
    private readonly Stack<Folder> forward = new();
    private readonly DispatcherTimer searchTimer = new() { Interval = TimeSpan.FromMilliseconds(220) };
    private bool changingSearch;
    private uint sortOrder;
    private void SearchChanged(object sender, TextChangedEventArgs args)
    {
        if (changingSearch || core is null) return;
        ++navigationVersion; searchTimer.Stop(); searchTimer.Start();
    }
    private async Task RunSearch()
    {
        if (core is null || closed) return;
        var query = SearchBox.Text.Trim(); var version = navigationVersion;
        if (query.Length == 0) { if (currentListening >= 0) await ShowListening((uint)currentListening); else if (currentPlaylist != 0) await ShowPlaylist(currentPlaylist); else if (currentFolder is not null) await Browse(currentFolder, false); return; }
        try
        {
            var result = await Task.Run(() => core.Search(query)); if (closed || version != navigationVersion) return;
            PlaylistTools.Visibility = Visibility.Collapsed; SortBox.Visibility = Visibility.Visible;
            tracks.Clear(); foreach (var track in result.Tracks) tracks.Add(track);
            ChildFolders.ItemsSource = result.Folders; ChildFolders.Visibility = result.Folders.Count > 0 ? Visibility.Visible : Visibility.Collapsed;
            FolderViewButton.Visibility = Visibility.Collapsed; SetFolderArtwork(false);
            FolderHeading.Text = "Search results"; FolderSummary.Text = $"{tracks.Count} tracks · {result.Folders.Count} folders · matching ‘{query}’";
            Breadcrumb.Text = "All library folders"; FolderArt.Source = null; PlayFolderButton.Content = "▶ Play results"; PlayFolderButton.IsEnabled = tracks.Count > 0;
            MoreButton.Visibility = Visibility.Collapsed; EmptyState.Text = "No matches. Try a title, artist, album or filename."; EmptyState.Visibility = tracks.Count == 0 ? Visibility.Visible : Visibility.Collapsed;
        }
        catch (Exception e) { Error(e); }
    }
    private void SearchShortcut(KeyboardAccelerator sender, KeyboardAcceleratorInvokedEventArgs args) { if (metadataDialogOpen) return; args.Handled = true; SearchBox.Focus(FocusState.Programmatic); SearchBox.SelectAll(); }
    private void SearchKeyDown(object sender, KeyRoutedEventArgs args)
    {
        if (args.Key == VirtualKey.Escape) { SearchBox.Text = ""; TrackList.Focus(FocusState.Programmatic); args.Handled = true; }
        if (args.Key == VirtualKey.Down) { TrackList.Focus(FocusState.Programmatic); if (tracks.Count > 0) TrackList.SelectedIndex = 0; args.Handled = true; }
    }
    private async void SortChanged(object sender, SelectionChangedEventArgs args)
    { sortOrder = (uint)Math.Max(0, SortBox.SelectedIndex); if (core is not null && currentFolder is not null && SearchBox.Text.Length == 0) { try { await Browse(currentFolder, false); } catch (Exception e) { Error(e); } } }
    private async void GoForward(object sender, RoutedEventArgs args) => await Forward();
    private async void ForwardShortcut(KeyboardAccelerator sender, KeyboardAcceleratorInvokedEventArgs args) { if (metadataDialogOpen) return; args.Handled = true; await Forward(); }
    private async Task Forward()
    { if (forward.Count == 0) return; if (currentFolder is not null) history.Push(currentFolder); var folder = forward.Pop(); try { await Browse(folder, false); } catch (Exception e) { Error(e); } }
}
