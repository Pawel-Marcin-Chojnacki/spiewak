using System.Collections.ObjectModel;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Input;
using Microsoft.UI.Xaml.Media.Imaging;
using Spiewak.Native;
using Windows.Storage.Pickers;
using Windows.System;
using System.Diagnostics;
namespace Spiewak;

public partial class MainWindow : Window
{
    private Core? core;
    private readonly ObservableCollection<Track> tracks = [];
    private readonly ObservableCollection<Track> queue = [];
    private readonly Stack<Folder?> history = new();
    private readonly DispatcherTimer timer = new() { Interval = TimeSpan.FromMilliseconds(250) };
    private Folder? currentFolder;
    private bool polling, updatingSeek, updatingVolume, closed, scanning;
    private ulong queueRevision = ulong.MaxValue;
    private string shownAudioError = "";
    private long playingId;
    private int queueStartIndex;
    private double? pendingSeek;
    private readonly DispatcherTimer seekTimer = new() { Interval = TimeSpan.FromMilliseconds(180) };
    private int navigationVersion;
    private ulong libraryRevision;
    private int libraryPollTick;
    public MainWindow()
    {
        InitializeComponent();
        Title = "Śpiewak — Your folders. Your music.";
        AppWindow.Resize(new Windows.Graphics.SizeInt32(1360, 900));
        if (AppWindow.Presenter is Microsoft.UI.Windowing.OverlappedPresenter presenter)
        { presenter.PreferredMinimumWidth = 1060; presenter.PreferredMinimumHeight = 640; }
        TrackList.ItemsSource = tracks; QueueList.ItemsSource = queue;
        timer.Tick += PollPlayback; searchTimer.Tick += async (_, _) => { searchTimer.Stop(); await RunSearch(); };
        seekTimer.Tick += async (_, _) => { seekTimer.Stop(); var value = pendingSeek; if (value is not null) await Command(5, value.Value); pendingSeek = null; };
        Closed += (_, _) => { closed = true; timer.Stop(); seekTimer.Stop(); searchTimer.Stop(); core?.Dispose(); };
    }
    private void Error(Exception error) { if (closed) return; ErrorBar.Message = error.Message; ErrorBar.IsOpen = true; }
    private async void OnLoaded(object sender, RoutedEventArgs args)
    {
        try
        {
            bool demo = Environment.GetCommandLineArgs().Contains("--demo");
            var data = System.IO.Path.Combine(Environment.GetFolderPath(Environment.SpecialFolder.LocalApplicationData), demo ? "SpiewakDemo" : "Spiewak");
            core = await Task.Run(() => new Core(System.IO.Path.Combine(data, "library.db")));
            if (closed) { core.Dispose(); return; }
            if (demo)
            {
                var sample = System.IO.Path.Combine(Environment.GetFolderPath(Environment.SpecialFolder.MyMusic), "Spiewak Demo");
                await Task.Run(() => core.AddRoot(sample));
                Title = "Śpiewak — Demo collection";
            }
            var restored = await Task.Run(() => core.Now());
            updatingVolume = true; VolumeSlider.Value = restored.Volume * 100; updatingVolume = false;
            await ReloadTree(); await ReloadPlaylists(); timer.Start();
            var roots = await Task.Run(() => core.Folders());
            if (roots.Folders.Count > 0) await Browse(roots.Folders[0], false);
            // Cached rows appear first. Reconciliation uses a separate SQLite WAL connection.
            await Scan(null);
        }
        catch (Exception e) { Error(e); }
    }
    private static TreeViewNode Node(Folder folder) => new() { Content = folder, HasUnrealizedChildren = folder.HasChildren };
    private async Task ReloadTree()
    {
        if (core is null) return;
        var result = await Task.Run(() => core.Folders());
        if (closed) return;
        FolderTree.RootNodes.Clear(); foreach (var folder in result.Folders) FolderTree.RootNodes.Add(Node(folder));
    }
    private async void FolderExpanding(TreeView sender, TreeViewExpandingEventArgs args)
    {
        if (core is null || !args.Node.HasUnrealizedChildren || args.Node.Content is not Folder folder) return;
        try { var result = await Task.Run(() => core.Folders(folder.Id)); if (closed) return; args.Node.Children.Clear(); foreach (var child in result.Folders) args.Node.Children.Add(Node(child)); args.Node.HasUnrealizedChildren = false; }
        catch (Exception e) { Error(e); }
    }
    private async void FolderInvoked(TreeView sender, TreeViewItemInvokedEventArgs args)
    {
        var folder = (args.InvokedItem as TreeViewNode)?.Content as Folder ?? args.InvokedItem as Folder;
        if (folder is not null) await TryBrowse(folder);
    }
    private async Task TryBrowse(Folder folder)
    { try { await Browse(folder, true); } catch (Exception e) { Error(e); } }
    private async Task Browse(Folder folder, bool remember)
    {
        if (core is null) return;
        currentListening = -1; currentPlaylist = 0; PlaylistTools.Visibility = Visibility.Collapsed; SortBox.Visibility = Visibility.Visible;
        changingSearch = true; SearchBox.Text = ""; changingSearch = false; searchTimer.Stop();
        var version = ++navigationVersion;
        var latest = await Task.Run(() => core.Folder(folder.Id));
        if (closed || version != navigationVersion) return;
        folder = latest.Folders.FirstOrDefault() ?? throw new IOException("This folder is no longer in the library.");
        var trackTask = Task.Run(() => core.Tracks(folder.Id, 0, sortOrder)); var folderTask = Task.Run(() => core.Folders(folder.Id));
        await Task.WhenAll(trackTask, folderTask);
        if (closed || version != navigationVersion) return;
        if (remember && currentFolder?.Id != folder.Id) { if (currentFolder is not null) history.Push(currentFolder); forward.Clear(); }
        currentFolder = folder; Breadcrumb.Text = folder.Path; ToolTipService.SetToolTip(Breadcrumb, folder.Path);
        FolderHeading.Text = folder.Name; tracks.Clear(); foreach (var track in trackTask.Result.Tracks) tracks.Add(track);
        ChildFolders.ItemsSource = folderTask.Result.Folders;
        ChildFolders.Visibility = folderTask.Result.Folders.Count > 0 ? Visibility.Visible : Visibility.Collapsed;
        EmptyState.Visibility = tracks.Count == 0 ? Visibility.Visible : Visibility.Collapsed;
        EmptyState.Text = folderTask.Result.Folders.Count > 0 ? "Open a subfolder to explore its tracks." : "No supported audio files in this folder.";
        FolderSummary.Text = $"{folder.KindLabel} · {folder.TrackCount} tracks · {folderTask.Result.Folders.Count} subfolders";
        FolderViewButton.Visibility = Visibility.Visible; FolderViewButton.Content = folder.KindLabel;
        ToolTipService.SetToolTip(FolderViewButton, folder.OverrideKind == FolderKind.Auto ? "Automatic presentation · change only how this folder appears" : "Your chosen presentation · files stay in this folder");
        SetFolderArtwork(folder.AlbumPresentation);
        MoreButton.Visibility = tracks.Count == 500 ? Visibility.Visible : Visibility.Collapsed;
        PlayFolderButton.Content = "▶ Play folder";
        PlayFolderButton.IsEnabled = tracks.Count > 0; BackButton.IsEnabled = history.Count > 0; ForwardButton.IsEnabled = forward.Count > 0;
        FolderArt.Source = folder.AlbumPresentation ? Image(folder.Artwork, 192) : null;
    }
    private async void ChildFolderClicked(object sender, ItemClickEventArgs args) { if (args.ClickedItem is Folder folder) await TryBrowse(folder); }
    private async void LoadMore(object sender, RoutedEventArgs args)
    {
        if (currentListening >= 0) { await MoreListening(); return; }
        if (currentPlaylist != 0) { await MorePlaylist(); return; }
        if (core is null || currentFolder is null) return;
        var folder = currentFolder; var version = navigationVersion; MoreButton.IsEnabled = false;
        try { var result = await Task.Run(() => core.Tracks(folder.Id, (uint)tracks.Count, sortOrder)); if (closed || version != navigationVersion) return; foreach (var track in result.Tracks) tracks.Add(track); MoreButton.Visibility = result.Tracks.Count == 500 ? Visibility.Visible : Visibility.Collapsed; FolderSummary.Text = $"{tracks.Count} tracks loaded · {folder.Path}"; }
        catch (Exception e) { Error(e); }
        finally { MoreButton.IsEnabled = true; }
    }
    private async Task Scan(string? path)
    {
        if (core is null || scanning) return;
        scanning = true; AddButton.IsEnabled = false; RescanButton.IsEnabled = false; ScanProgress.Visibility = Visibility.Visible; LibraryStatus.Text = "Indexing folders…";
        try
        {
            var result = await Task.Run(() => path is null ? core.Rescan() : core.AddRoot(path)); if (closed) return;
            LibraryStatus.Text = result.Message; await RefreshLibraryView();
        }
        catch (Exception e) { LibraryStatus.Text = "Scan could not finish"; Error(e); }
        finally { if (!closed) { scanning = false; AddButton.IsEnabled = true; RescanButton.IsEnabled = true; ScanProgress.Visibility = Visibility.Collapsed; } }
    }
    private async void AddFolder(object sender, RoutedEventArgs args)
    {
        try
        {
            var picker = new FolderPicker(); WinRT.Interop.InitializeWithWindow.Initialize(picker, WinRT.Interop.WindowNative.GetWindowHandle(this)); picker.FileTypeFilter.Add("*");
            var folder = await picker.PickSingleFolderAsync(); if (folder is not null) await Scan(folder.Path);
        }
        catch (Exception e) { Error(e); }
    }
    private async void Rescan(object sender, RoutedEventArgs args) => await Scan(null);
    private async Task Play(Track track)
    { if (core is null) return; var search = SearchBox.Text.Trim().Length > 0; var sort = sortOrder; var playlist = currentPlaylist; var index = TrackList.SelectedItem == track ? TrackList.SelectedIndex : tracks.IndexOf(track); try { await Task.Run(() => playlist != 0 ? core.PlayPlaylist(playlist, (uint)Math.Max(0,index)) : search ? core.Selection(new[] { track }, 0) : core.Play(track.Folder, track.Id, sort)); } catch (Exception e) { Error(e); } }
    private async Task PlayOne(Track track)
    { if (core is null) return; try { await Task.Run(() => core.Selection(new[] { track }, 0)); } catch (Exception e) { Error(e); } }
    private async void TrackDoubleTapped(object sender, DoubleTappedRoutedEventArgs args) { if (TrackList.SelectedItem is Track track) await PlayOne(track); }
    private async void PlaySelected(object sender, RoutedEventArgs args) => await Selection(0);
    private async void AddNext(object sender, RoutedEventArgs args) => await Selection(2);
    private async void AddToQueue(object sender, RoutedEventArgs args) => await Selection(1);
    private async Task Selection(uint mode)
    {
        if (core is null) return;
        var selected = tracks.Where(t => TrackList.SelectedItems.Contains(t)).ToArray();
        if (selected.Length == 0) return;
        try { await Task.Run(() => core.Selection(selected, mode)); } catch (Exception e) { Error(e); }
    }
    private async void PlayFolder(object sender, RoutedEventArgs args)
    {
        if (core is null || tracks.Count == 0) return;
        try
        {
            if (SearchBox.Text.Trim().Length > 0)
            {
                var rows = tracks.ToArray();
                await Task.Run(() => core.Selection(rows, 0));
            }
            else if (currentListening >= 0)
            {
                var view = (uint)currentListening;
                await Task.Run(() => core.PlayListening(view));
            }
            else if (tracks.FirstOrDefault() is Track track) await Play(track);
        }
        catch (Exception e) { Error(e); }
    }
    private async void QueueDoubleTapped(object sender, DoubleTappedRoutedEventArgs args) { if (QueueList.SelectedIndex >= 0) await Command(14, queueStartIndex + QueueList.SelectedIndex); }
    private async void ShuffleQueue(object sender, RoutedEventArgs args) => await Command(9);
    private async void CycleRepeat(object sender, RoutedEventArgs args) => await Command(8);
    private async void Mute(object sender, RoutedEventArgs args) => await Command(7);
    private async Task EditQueue(uint command) { if (QueueList.SelectedIndex >= 0) await Command(command, queueStartIndex + QueueList.SelectedIndex); }
    private async void RemoveQueue(object sender, RoutedEventArgs args) => await EditQueue(10);
    private async void MoveQueueUp(object sender, RoutedEventArgs args) => await EditQueue(11);
    private async void MoveQueueDown(object sender, RoutedEventArgs args) => await EditQueue(12);
    private async void ClearQueue(object sender, RoutedEventArgs args) => await Command(13);
    private async void QueueKeyDown(object sender, KeyRoutedEventArgs args) { if (args.Key == VirtualKey.Delete) { args.Handled = true; await EditQueue(10); } else if (args.Key == VirtualKey.Enter && QueueList.SelectedIndex >= 0) { args.Handled = true; await Command(14, queueStartIndex + QueueList.SelectedIndex); } }
    private async Task Command(uint command, double value = 0)
    { if (core is null || closed) return; try { await Task.Run(() => core.Command(command, value)); } catch (Exception e) { Error(e); } }
    private async void TogglePlayback(object sender, RoutedEventArgs args) => await Command(1);
    private async void Stop(object sender, RoutedEventArgs args) => await Command(2);
    private async void Next(object sender, RoutedEventArgs args) => await Command(3);
    private async void Previous(object sender, RoutedEventArgs args) => await Command(4);
    private async void ToggleShortcut(KeyboardAccelerator sender, KeyboardAcceleratorInvokedEventArgs args) { if (metadataDialogOpen || FocusManager.GetFocusedElement(Shell.XamlRoot) is TextBox) return; args.Handled = true; await Command(1); }
    private async void TrackKeyDown(object sender, KeyRoutedEventArgs args) { if (args.Key == VirtualKey.Enter && TrackList.SelectedItem is Track track) { args.Handled = true; await PlayOne(track); } }
    private void TrackSelectionChanged(object sender, SelectionChangedEventArgs args)
    { UpdateSelectedFavorite(); if (TrackList.SelectedItem is Track track) { InspectorPath.Text = track.Path; InspectorAlbum.Text = $"{track.Album}\n{track.Codec} · {track.Time}" + (currentListening >= 0 && SearchBox.Text.Trim().Length == 0 ? $"\n{(currentListening == 2 ? "Started" : currentListening == 1 ? "Added" : "Favorited")}: {track.LibraryTime}" : ""); } }
    private void ShowSelected(object sender, RoutedEventArgs args)
    { if (TrackList.SelectedItem is Track track) Process.Start(new ProcessStartInfo("explorer.exe", $"/select,\"{track.Path}\"") { UseShellExecute = true }); }
    private async void GoBack(object sender, RoutedEventArgs args) => await Back();
    private async void BackShortcut(KeyboardAccelerator sender, KeyboardAcceleratorInvokedEventArgs args) { if (metadataDialogOpen) return; args.Handled = true; await Back(); }
    private async Task Back()
    { if (history.Count == 0) return; if (currentFolder is not null) forward.Push(currentFolder); var folder = history.Pop(); if (folder is not null) { try { await Browse(folder, false); } catch (Exception e) { Error(e); } } BackButton.IsEnabled = history.Count > 0; ForwardButton.IsEnabled = forward.Count > 0; }
    private async void PollPlayback(object? sender, object args)
    {
        if (core is null || polling || closed) return; polling = true;
        try
        {
            if (++libraryPollTick % 4 == 0 && !scanning) { await PollLibrary(); await PollListening(); }
            var state = await Task.Run(() => core.Now()); if (closed) return;
            if (state.Error.Length > 0 && state.Error != shownAudioError) { ErrorBar.Message = state.Error; ErrorBar.IsOpen = true; }
            shownAudioError = state.Error;
            MuteIcon.Glyph = (state.Flags & 1) != 0 ? "\uE74F" : "\uE767";
            RepeatLabel.Text = ((state.Flags >> 1) & 3) switch { 1 => "↻∞", 2 => "↻1", _ => "↻" };
            var track = state.Tracks.FirstOrDefault();
            playingTrack = track; PlayingFavoriteButton.IsEnabled = track?.Id > 0; PlayingFavoriteButton.Content = track?.Favorite == true ? "♥" : "♡"; ToolTipService.SetToolTip(PlayingFavoriteButton, track?.Favorite == true ? "Remove current track from favorites" : "Add current track to favorites");
            if (playingId != state.CurrentId || queueRevision != state.Revision) { playingId = state.CurrentId; PlayingTitle.Text = track?.Title ?? "Nothing playing"; PlayingArtist.Text = track?.ArtistLabel ?? "Choose something from your folders"; PlayingArt.Source = Image(track?.Artwork, 112); }
            updatingSeek = true; SeekSlider.Maximum = Math.Max(1, track?.Duration ?? 1); if (pendingSeek is null) SeekSlider.Value = Math.Clamp(state.Position, 0, SeekSlider.Maximum); SeekSlider.IsEnabled = track?.Duration > 0; updatingSeek = false;
            PositionText.Text = Track.FormatTime(state.Position); DurationText.Text = track?.Time ?? "0:00"; PlayIcon.Glyph = state.Playing ? "\uE769" : "\uE768";
            if (queueRevision != state.Revision)
            {
                queueRevision = state.Revision; var result = await Task.Run(() => core.Queue()); if (closed) return;
                queueStartIndex = (int)Math.Max(0, state.CurrentIndex + 1); queue.Clear(); foreach (var queued in result.Tracks.Skip(queueStartIndex)) queue.Add(queued);
                QueueSummary.Text = $"{queue.Count} upcoming · {track?.Title ?? "Nothing playing"}";
            }
        }
        catch (Exception e) { Error(e); }
        finally { polling = false; updatingSeek = false; }
    }
    private void SeekChanged(object sender, Microsoft.UI.Xaml.Controls.Primitives.RangeBaseValueChangedEventArgs args)
    { if (!updatingSeek && core is not null) { pendingSeek = args.NewValue; seekTimer.Stop(); seekTimer.Start(); } }
    private async void VolumeChanged(object sender, Microsoft.UI.Xaml.Controls.Primitives.RangeBaseValueChangedEventArgs args)
    { if (!updatingVolume && core is not null) await Command(6, args.NewValue / 100); }
    private static BitmapImage? Image(string? path, int size)
    { if (string.IsNullOrWhiteSpace(path)) return null; try { return new BitmapImage(new Uri(path)) { DecodePixelWidth = size }; } catch { return null; } }
    private void QueueVisibilityChanged(object sender, RoutedEventArgs args)
    { QueuePanel.Visibility = QueueToggle.IsChecked == true ? Visibility.Visible : Visibility.Collapsed; QueueColumn.Width = QueueToggle.IsChecked == true ? new GridLength(272) : new GridLength(0); }
    private void DensityChanged(object sender, RoutedEventArgs args)
    {
        var style = new Style(typeof(ListViewItem)); style.Setters.Add(new Setter(Control.HorizontalContentAlignmentProperty, HorizontalAlignment.Stretch)); style.Setters.Add(new Setter(Control.PaddingProperty, new Thickness(12, CompactToggle.IsChecked == true ? 2 : 8, 12, CompactToggle.IsChecked == true ? 2 : 8))); style.Setters.Add(new Setter(FrameworkElement.MinHeightProperty, CompactToggle.IsChecked == true ? 32d : 44d)); TrackList.ItemContainerStyle = style;
    }
}
