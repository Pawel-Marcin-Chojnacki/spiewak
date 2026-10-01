using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using Spiewak.Native;
namespace Spiewak;

public partial class MainWindow
{
    private bool metadataDialogOpen;
    private static string FieldLabel(MetadataField field) => field switch
    {
        MetadataField.FileName => "Filename (same folder)", MetadataField.AlbumArtist => "Album artist",
        MetadataField.TrackNumber => "Track number", MetadataField.TrackTotal => "Track total",
        MetadataField.DiscNumber => "Disc number", MetadataField.DiscTotal => "Disc total",
        MetadataField.Date => "Recording date / year", MetadataField.Artwork => "Front cover",
        MetadataField.ReplayGainTrack => "Track gain (dB)", MetadataField.ReplayGainAlbum => "Album gain (dB)",
        MetadataField.ReplayPeakTrack => "Track peak", MetadataField.ReplayPeakAlbum => "Album peak", _ => field.ToString()
    };
    private async void EditSelectedMetadata(object sender, RoutedEventArgs args)
    {
        if (core is null || metadataDialogOpen) return;
        if (TrackList.SelectedItems.Count != 1 || TrackList.SelectedItem is not Track track || track.Id == 0)
        { Error(new InvalidOperationException("Select one available track to edit its tags.")); return; }
        metadataDialogOpen = true;
        long preparedId = 0;
        try
        {
            var current = await Task.Run(() => core.Metadata(track.Id)); if (closed) return;
            var form = new StackPanel { Spacing = 10, Width = 480 };
            form.Children.Add(new TextBlock { Text = current.OriginalPath, TextWrapping = TextWrapping.Wrap, IsTextSelectionEnabled = true });
            var fields = new Dictionary<MetadataField, TextBox>();
            var initial = current.Values.ToDictionary(v => v.Field, v => v.Value);
            var advanced = new StackPanel { Spacing = 8 };
            foreach (var value in current.Values.Where(v => v.Field != MetadataField.Artwork))
            {
                var box = new TextBox { Header = FieldLabel(value.Field), Text = value.Value, MaxLength = 16384 };
                fields.Add(value.Field, box);
                if (value.Field is MetadataField.Title or MetadataField.Artist or MetadataField.Album or MetadataField.FileName) form.Children.Add(box);
                else advanced.Children.Add(box);
            }
            var artwork = new TextBox { Header = "Replace front cover (JPEG or PNG)", PlaceholderText = initial[MetadataField.Artwork], IsReadOnly = true };
            fields.Add(MetadataField.Artwork, artwork); form.Children.Add(artwork);
            var removeCover = new CheckBox { Content = "Remove the existing front cover" };
            var coverButton = new Button { Content = "Choose cover…" };
            coverButton.Click += async (_, _) =>
            {
                try
                {
                    var picker = new Microsoft.Windows.Storage.Pickers.FileOpenPicker(AppWindow.Id);
                    picker.FileTypeFilter.Add(".jpg"); picker.FileTypeFilter.Add(".jpeg"); picker.FileTypeFilter.Add(".png");
                    var picked = await picker.PickSingleFileAsync();
                    if (picked is not null && !closed) { artwork.Text = picked.Path; removeCover.IsChecked = false; }
                }
                catch (Exception e) { Error(e); }
            };
            removeCover.Checked += (_, _) => artwork.Text = "";
            form.Children.Add(coverButton); form.Children.Add(removeCover);
            var clean = new Button { Content = "Suggest cleanup of download name" }; form.Children.Add(clean);
            var error = new InfoBar { Severity = InfoBarSeverity.Error, IsClosable = true };
            clean.Click += async (_, _) =>
            {
                clean.IsEnabled = false;
                try
                {
                    var suggested = await Task.Run(() => core.CleanupSuggestion(track.Id)); if (closed) return;
                    foreach (var value in suggested) fields[value.Field].Text = value.Value;
                    clean.Content = suggested.Count > 0 ? "Suggested values — review before applying" : "No known download noise found";
                }
                catch (Exception e) { error.Message = e.Message; error.IsOpen = true; }
                finally { clean.IsEnabled = true; }
            };
            form.Children.Add(new Expander { Header = "More tags", Content = advanced, HorizontalAlignment = HorizontalAlignment.Stretch });
            form.Children.Add(error);
            var dialog = new ContentDialog { XamlRoot = Shell.XamlRoot, RequestedTheme = Shell.ActualTheme, Title = "Edit tags", PrimaryButtonText = "Preview changes", CloseButtonText = "Cancel", Content = new ScrollViewer { Content = form, MaxHeight = 520 } };
            IReadOnlyList<MetadataRecord>? proposal = null;
            List<MetadataValue> changes = [];
            dialog.PrimaryButtonClick += async (_, click) =>
            {
                var deferral = click.GetDeferral();
                try
                {
                    changes = fields.Where(v => v.Key != MetadataField.Artwork && v.Value.Text != initial[v.Key]).Select(v => new MetadataValue(v.Key, v.Value.Text)).ToList();
                    if (artwork.Text.Length > 0 || removeCover.IsChecked == true) changes.Add(new(MetadataField.Artwork, artwork.Text));
                    proposal = await Task.Run(() => core.PrepareMetadata(track.Id, changes)); preparedId = proposal.Last().OperationId;
                }
                catch (Exception e) { click.Cancel = true; error.Message = e.Message; error.IsOpen = true; }
                finally { deferral.Complete(); }
            };
            await dialog.ShowAsync(); if (closed || proposal is null) return;
            if (await PreviewMetadata(proposal, artwork.Text.Length > 0 ? System.IO.Path.GetFileName(artwork.Text) + " (front cover)" : removeCover.IsChecked == true ? "Remove front cover" : "")) preparedId = 0;
        }
        catch (Exception e) { Error(e); }
        finally
        {
            if (preparedId > 0 && !closed && core is not null)
            {
                try { await Task.Run(() => core.CancelMetadata(preparedId)); }
                catch (Exception e) { Error(e); }
            }
            metadataDialogOpen = false;
        }
    }
    private async Task<bool> PreviewMetadata(IReadOnlyList<MetadataRecord> proposal, string artworkLabel = "", string artworkPath = "")
    {
        bool applied = false;
        var before = proposal[0]; var after = proposal[1];
        var preview = new StackPanel { Spacing = 12, Width = 480 };
        preview.Children.Add(new TextBlock { Text = "Review the changes. This file stays in its folder. If it is playing, applying will leave it paused at the saved position.", TextWrapping = TextWrapping.Wrap });
        if (after.Message.Length > 0) preview.Children.Add(new TextBlock { Text = after.Message, TextWrapping = TextWrapping.Wrap, FontSize = 12 });
        foreach (var value in after.Values)
        {
            var old = before.Values.Single(v => v.Field == value.Field).Value;
            if (old == value.Value && !(artworkLabel.Length > 0 && value.Field == MetadataField.Artwork)) continue;
            var display = value.Field == MetadataField.Artwork && artworkLabel.Length > 0 ? artworkLabel : value.Value;
            preview.Children.Add(new TextBlock { Text = $"{FieldLabel(value.Field)}\n{(old.Length > 0 ? old : "(empty)")} → {(display.Length > 0 ? display : "(empty)")}", TextWrapping = TextWrapping.Wrap, IsTextSelectionEnabled = true });
        }
        if (artworkPath.Length > 0) preview.Children.Add(new Microsoft.UI.Xaml.Controls.Image { Source = Image(artworkPath, 192), Width = 192, Height = 192, HorizontalAlignment = HorizontalAlignment.Left });
        var applyError = new InfoBar { Severity = InfoBarSeverity.Error, IsClosable = true }; preview.Children.Add(applyError);
        var confirm = new ContentDialog { XamlRoot = Shell.XamlRoot, RequestedTheme = Shell.ActualTheme, Title = "Apply to this file?", PrimaryButtonText = "Apply changes", CloseButtonText = "Cancel", Content = new ScrollViewer { Content = preview, MaxHeight = 520 } };
        confirm.PrimaryButtonClick += async (_, click) =>
        {
            var deferral = click.GetDeferral();
            try { await Task.Run(() => core!.ApplyMetadata(after.OperationId)); applied = true; }
            catch (Exception e) { click.Cancel = true; applyError.Message = e.Message; applyError.IsOpen = true; }
            finally { deferral.Complete(); }
        };
        await confirm.ShowAsync(); if (!closed) await RefreshLibraryView();
        return applied;
    }
    private async void ShowMetadataHistory(object sender, RoutedEventArgs args)
    {
        if (core is null || metadataDialogOpen) return;
        metadataDialogOpen = true;
        try
        {
            var panel = new StackPanel { Width = 480, Spacing = 16 };
            var error = new InfoBar { Severity = InfoBarSeverity.Error, IsClosable = true };
            async Task Reload()
            {
                var records = await Task.Run(() => core.MetadataOperations()); if (closed) return;
                panel.Children.Clear(); panel.Children.Add(new TextBlock { Text = "Recent file changes. Undo restores the original bytes and filename and pauses this track if it is playing. Files changed by another program are preserved.", TextWrapping = TextWrapping.Wrap });
                panel.Children.Add(error);
                if (records.Count == 0) panel.Children.Add(new TextBlock { Text = "No file changes yet." });
                foreach (var record in records)
                {
                    var item = new StackPanel { Spacing = 5 };
                    item.Children.Add(new TextBlock { Text = $"#{record.OperationId} · {record.State} · {DateTimeOffset.FromUnixTimeSeconds(record.Created).ToLocalTime():g}", FontWeight = Microsoft.UI.Text.FontWeights.SemiBold });
                    item.Children.Add(new TextBlock { Text = record.OriginalPath + (record.OriginalPath != record.TargetPath ? "\n→ " + record.TargetPath : ""), TextWrapping = TextWrapping.Wrap, IsTextSelectionEnabled = true });
                    item.Children.Add(new TextBlock { Text = record.Message, TextWrapping = TextWrapping.Wrap, FontSize = 12 });
                    if (record.State is OperationState.Applied or OperationState.Prepared or OperationState.Conflict)
                    {
                        var button = new Button { Content = record.State == OperationState.Applied ? "Undo this change" : record.State == OperationState.Prepared ? "Cancel proposal" : "Review recovery…" };
                        button.Click += async (_, _) =>
                        {
                            button.IsEnabled = false;
                            try
                            {
                                if (record.State == OperationState.Conflict)
                                {
                                    // ContentDialogs cannot be nested. Expose explicit recovery in a second pass after closing history.
                                    pendingRecovery = record; historyDialog?.Hide(); return;
                                }
                                await Task.Run(() => { if (record.State == OperationState.Applied) core.UndoMetadata(record.OperationId); else core.CancelMetadata(record.OperationId); });
                                await RefreshLibraryView(); await Reload();
                            }
                            catch (Exception e) { error.Message = e.Message; error.IsOpen = true; button.IsEnabled = true; }
                        };
                        item.Children.Add(button);
                    }
                    panel.Children.Add(item);
                }
            }
            pendingRecovery = null;
            await Reload(); if (closed) return;
            historyDialog = new ContentDialog { XamlRoot = Shell.XamlRoot, RequestedTheme = Shell.ActualTheme, Title = "File change history", CloseButtonText = "Close", Content = new ScrollViewer { Content = panel, MaxHeight = 520 } };
            await historyDialog.ShowAsync(); historyDialog = null;
            if (!closed && pendingRecovery is MetadataRecord recovery)
            {
                var confirm = new ContentDialog { XamlRoot = Shell.XamlRoot, RequestedTheme = Shell.ActualTheme, Title = "Restore original file?", PrimaryButtonText = "Preserve current file and restore original", CloseButtonText = "Cancel", Content = new TextBlock { Width = 480, TextWrapping = TextWrapping.Wrap, Text = recovery.OriginalPath + "\n\nThe operation was interrupted or the file changed. Recovery first keeps a separate copy of the current bytes, then restores the original. It will pause this track if playing. A different file at the old name will never be overwritten." } };
                if (await confirm.ShowAsync() == ContentDialogResult.Primary) { await Task.Run(() => core.RecoverMetadata(recovery.OperationId)); if (!closed) await RefreshLibraryView(); }
            }
        }
        catch (Exception e) { Error(e); }
        finally { metadataDialogOpen = false; historyDialog = null; pendingRecovery = null; }
    }
    private ContentDialog? historyDialog;
    private MetadataRecord? pendingRecovery;
}
