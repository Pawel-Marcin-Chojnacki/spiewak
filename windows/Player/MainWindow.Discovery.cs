using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Markup;
using Spiewak.Native;
namespace Spiewak;

public sealed partial class MainWindow
{
    private async void DiscoverSelectedMetadata(object sender, RoutedEventArgs args)
    {
        if (core is null || metadataDialogOpen) return;
        if (TrackList.SelectedItems.Count != 1 || TrackList.SelectedItem is not Track track || track.Id == 0)
        { Error(new InvalidOperationException("Select one available track to find its tags and cover.")); return; }
        metadataDialogOpen = true;
        long preparedId = 0;
        try
        {
            var state = await Task.Run(() => core.Discovery(track.Id)); if (closed) return;
            var form = new StackPanel { Width = 520, Spacing = 10 };
            form.Children.Add(new TextBlock { Text = "Find metadata for this file. Your folders stay in place. Every result needs review before applying.", TextWrapping = TextWrapping.Wrap });
            var online = new CheckBox { Content = "Enable online tag and cover lookup", IsChecked = state.Enabled };
            form.Children.Add(online);
            form.Children.Add(new TextBlock { Text = "Search sends the artist and title below to MusicBrainz. Covers are downloaded from Cover Art Archive / Internet Archive. Audio files are not uploaded.", TextWrapping = TextWrapping.Wrap, FontSize = 12 });
            var title = new TextBox { Header = "Title (keep Live / Remix / Acoustic)", Text = state.Title, MaxLength = 512 };
            var artist = new TextBox { Header = "Artist", Text = state.Artist, MaxLength = 512 };
            form.Children.Add(title); form.Children.Add(artist);
            var search = new Button { Content = "Search recordings", IsEnabled = state.Enabled };
            var progress = new ProgressBar { IsIndeterminate = true, Visibility = Visibility.Collapsed };
            var status = new TextBlock { Text = state.Message, TextWrapping = TextWrapping.Wrap, FontSize = 12 };
            var error = new InfoBar { Severity = InfoBarSeverity.Error, IsClosable = true };
            var results = new ListView { SelectionMode = ListViewSelectionMode.Single, MaxHeight = 250, ItemsSource = state.Candidates,
                ItemTemplate = (DataTemplate)XamlReader.Load("<DataTemplate xmlns='http://schemas.microsoft.com/winfx/2006/xaml/presentation'><TextBlock Text='{Binding Summary}' TextWrapping='Wrap' Margin='0,6' MaxWidth='470'/></DataTemplate>") };
            var replaceTags = new CheckBox { Content = "Replace existing tags (otherwise fill empty fields only)" };
            var replaceCover = new CheckBox { Content = "Replace existing artwork (otherwise keep local artwork)" };
            form.Children.Add(search); form.Children.Add(progress); form.Children.Add(status); form.Children.Add(results); form.Children.Add(replaceTags); form.Children.Add(replaceCover); form.Children.Add(error);
            var dialog = new ContentDialog { XamlRoot = Shell.XamlRoot, RequestedTheme = Shell.ActualTheme, Title = "Find tags and cover", PrimaryButtonText = "Preview this candidate", CloseButtonText = "Close", IsPrimaryButtonEnabled = false, Content = new ScrollViewer { Content = form, MaxHeight = 560 } };
            bool busy = false;
            void Update() { dialog.IsPrimaryButtonEnabled = !busy && results.SelectedItem is MetadataCandidate { VersionCompatible: true }; search.IsEnabled = !busy && online.IsChecked == true; }
            void Busy(bool value)
            {
                busy = value; progress.Visibility = value ? Visibility.Visible : Visibility.Collapsed;
                online.IsEnabled = title.IsEnabled = artist.IsEnabled = results.IsEnabled = replaceTags.IsEnabled = replaceCover.IsEnabled = !value;
                dialog.IsSecondaryButtonEnabled = !value; Update();
            }
            dialog.CloseButtonClick += (_, click) => click.Cancel = busy;
            results.SelectionChanged += (_, _) => Update();
            online.Click += async (_, _) =>
            {
                var enableLookup = online.IsChecked == true;
                Busy(true);
                try { await Task.Run(() => core.SetDiscoveryEnabled(enableLookup)); }
                catch (Exception e) { online.IsChecked = !online.IsChecked; error.Message = e.Message; error.IsOpen = true; }
                finally { Busy(false); }
            };
            search.Click += async (_, _) =>
            {
                var queryTitle = title.Text; var queryArtist = artist.Text;
                Busy(true); error.IsOpen = false;
                try
                {
                    var found = await Task.Run(() => core.DiscoverMetadata(track.Id, queryTitle, queryArtist)); if (closed) return;
                    results.ItemsSource = found.Candidates; status.Text = found.Message;
                }
                catch (Exception e) { error.Message = e.Message; error.IsOpen = true; }
                finally { Busy(false); }
            };
            IReadOnlyList<MetadataRecord>? proposal = null;
            dialog.PrimaryButtonClick += async (_, click) =>
            {
                var deferral = click.GetDeferral(); Busy(true); error.IsOpen = false;
                try
                {
                    if (results.SelectedItem is not MetadataCandidate { VersionCompatible: true } candidate) throw new InvalidOperationException("Choose a recording with compatible version markers.");
                    var overwriteTags = replaceTags.IsChecked == true; var overwriteCover = replaceCover.IsChecked == true;
                    proposal = await Task.Run(() => core.PrepareDiscovery(candidate.Id, overwriteTags, overwriteCover)); preparedId = proposal.Last().OperationId;
                }
                catch (Exception e) { click.Cancel = true; error.Message = e.Message; error.IsOpen = true; }
                finally { Busy(false); deferral.Complete(); }
            };
            await dialog.ShowAsync(); if (closed || proposal is null) return;
            // The lookup dialog must close before the shared before/after review opens.
            var refreshed = await Task.Run(() => core.Discovery(track.Id));
            var coverPath = refreshed.Candidates.FirstOrDefault(c => c.Id == ((MetadataCandidate)results.SelectedItem).Id)?.Artwork ?? "";
            if (await PreviewMetadata(proposal, coverPath.Length > 0 ? "Downloaded front cover" : "", coverPath)) preparedId = 0;
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
}
