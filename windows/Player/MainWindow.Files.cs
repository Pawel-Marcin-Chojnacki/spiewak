using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using Microsoft.Windows.Storage.Pickers;
using Spiewak.Native;
using Windows.ApplicationModel.DataTransfer;
using Windows.Storage;
namespace Spiewak;
public partial class MainWindow
{
    private bool filePickerOpen;
    private async void ImportPlaylist(object sender, RoutedEventArgs args)
    {
        if (core is null || filePickerOpen) return;
        filePickerOpen = true;
        try
        {
            var picker = new FileOpenPicker(AppWindow.Id) { CommitButtonText = "Import playlist" };
            picker.FileTypeFilter.Add(".m3u8"); picker.FileTypeFilter.Add(".m3u");
            var file = await picker.PickSingleFileAsync();
            if (file is null || closed) return;
            var result = await Task.Run(() => core.ImportPlaylist(file.Path)); if (closed) return;
            LibraryStatus.Text = result.Message; await ReloadPlaylists(); await ShowPlaylist(result.CurrentId);
        }
        catch (Exception e) { Error(e); }
        finally { filePickerOpen = false; }
    }
    private async void ExportSelectedPlaylist(object sender, RoutedEventArgs args)
    { if (PlaylistList.SelectedItem is Playlist playlist) await ExportPlaylist(playlist.Id); }
    private async void ExportCurrentPlaylist(object sender, RoutedEventArgs args)
    { if (currentPlaylist != 0) await ExportPlaylist(currentPlaylist); }
    private async Task ExportPlaylist(long id)
    {
        if (core is null || filePickerOpen) return;
        filePickerOpen = true;
        try
        {
            var playlists = await Task.Run(() => core.Playlists()); if (closed) return;
            var playlist = playlists.Playlists.Single(p => p.Id == id);
            var invalid = System.IO.Path.GetInvalidFileNameChars();
            var name = new string(playlist.Name.Select(c => invalid.Contains(c) ? '_' : c).ToArray()).TrimEnd('.', ' ');
            var picker = new FileSavePicker(AppWindow.Id) { SuggestedFileName = string.IsNullOrEmpty(name) ? "Playlist" : name, DefaultFileExtension = ".m3u8", CommitButtonText = "Export playlist" };
            picker.FileTypeChoices.Add("UTF-8 M3U8 playlist", new List<string> { ".m3u8" });
            var file = await picker.PickSaveFileAsync(); if (file is null || closed) return;
            var result = await Task.Run(() => core.ExportPlaylist(id, file.Path)); if (!closed) LibraryStatus.Text = result.Message;
        }
        catch (Exception e) { Error(e); }
        finally { filePickerOpen = false; }
    }
    private void QueueDragOver(object sender, DragEventArgs args)
    {
        args.Handled = true;
        args.AcceptedOperation = args.DataView.Contains(StandardDataFormats.StorageItems) ? DataPackageOperation.Copy : DataPackageOperation.None;
        args.DragUIOverride.Caption = "Add to queue · files stay in their folders";
    }
    private void LibraryDragOver(object sender, DragEventArgs args)
    {
        args.Handled = true;
        args.AcceptedOperation = args.DataView.Contains(StandardDataFormats.StorageItems) ? DataPackageOperation.Copy : DataPackageOperation.None;
        args.DragUIOverride.Caption = "Add folders to library";
    }
    private async void QueueDrop(object sender, DragEventArgs args)
    {
        args.Handled = true;
        if (core is null || !args.DataView.Contains(StandardDataFormats.StorageItems)) return;
        var deferral = args.GetDeferral();
        try
        {
            var items = await args.DataView.GetStorageItemsAsync(); if (closed) return;
            var paths = items.Select(i => i.Path).ToArray();
            var result = await Task.Run(() => core.DropFiles(paths)); if (!closed) LibraryStatus.Text = result.Message;
        }
        catch (Exception e) { Error(e); }
        finally { deferral.Complete(); }
    }
    private async void LibraryDrop(object sender, DragEventArgs args)
    {
        args.Handled = true;
        if (core is null || !args.DataView.Contains(StandardDataFormats.StorageItems)) return;
        var deferral = args.GetDeferral();
        try
        {
            var items = await args.DataView.GetStorageItemsAsync(); if (closed) return;
            if (items.Count == 0 || items.Any(i => i is not StorageFolder)) throw new InvalidOperationException("Drop folders here to add them to the library. Drop audio files or M3U8 on the track list or queue.");
            foreach (var folder in items) { if (closed) return; await Scan(folder.Path); }
        }
        catch (Exception e) { Error(e); }
        finally { deferral.Complete(); }
    }
}
