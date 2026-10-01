using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using Spiewak.Native;
namespace Spiewak;

public partial class MainWindow
{
    private async void EnableCurrentInbox(object sender, RoutedEventArgs args)
    {
        if (core is null || currentFolder is not Folder folder || metadataDialogOpen) return;
        metadataDialogOpen = true;
        try
        {
            var error = new InfoBar { Severity = InfoBarSeverity.Error, IsClosable = true };
            var panel = new StackPanel { Spacing = 12, Width = 460 };
            panel.Children.Add(new TextBlock { Text = folder.Path, TextWrapping = TextWrapping.Wrap, IsTextSelectionEnabled = true });
            panel.Children.Add(new TextBlock { Text = "Only new audio files directly in this folder will enter Inbox after at least 10 seconds without size or modification-time changes and a readable metadata check. Existing files and subfolders are excluded.\n\nInbox does not send network requests or change files automatically. Find tags and cover, Edit tags, Preview and Undo remain available for each ready file.", TextWrapping = TextWrapping.Wrap });
            panel.Children.Add(error);
            var dialog = new ContentDialog { XamlRoot = Shell.XamlRoot, RequestedTheme = Shell.ActualTheme, Title = "Use this folder as Inbox?", PrimaryButtonText = "Enable Inbox", CloseButtonText = "Cancel", Content = panel };
            dialog.PrimaryButtonClick += async (_, click) =>
            {
                var deferral = click.GetDeferral();
                try { await Task.Run(() => core.SetInbox(folder.Id, true)); }
                catch (Exception e) { click.Cancel = true; error.Message = e.Message; error.IsOpen = true; }
                finally { deferral.Complete(); }
            };
            if (await dialog.ShowAsync() == ContentDialogResult.Primary && !closed) await ShowListening(3);
        }
        catch (Exception e) { Error(e); }
        finally { metadataDialogOpen = false; }
    }

    private async void ManageInboxFolders(object sender, RoutedEventArgs args)
    {
        if (core is null || metadataDialogOpen) return;
        metadataDialogOpen = true;
        try
        {
            var panel = new StackPanel { Width = 480, Spacing = 12 };
            var error = new InfoBar { Severity = InfoBarSeverity.Error, IsClosable = true };
            bool busy = false;
            async Task Reload()
            {
                var result = await Task.Run(() => core.Inbox()); if (closed) return;
                panel.Children.Clear();
                panel.Children.Add(new TextBlock { Text = "To add an Inbox, browse its real folder and choose Folder options → Use as Inbox. Each folder covers only its direct new files. Removing a folder here clears its Inbox list, never its music; enabling it again starts a new baseline.", TextWrapping = TextWrapping.Wrap });
                panel.Children.Add(new TextBlock { Text = result.Message, TextWrapping = TextWrapping.Wrap });
                panel.Children.Add(error);
                foreach (var folder in result.Folders)
                {
                    var item = new StackPanel { Spacing = 4 };
                    item.Children.Add(new TextBlock { Text = folder.Path, TextWrapping = TextWrapping.Wrap, IsTextSelectionEnabled = true });
                    var remove = new Button { Content = "Stop using as Inbox" };
                    remove.Click += async (_, _) =>
                    {
                        if (busy) return;
                        busy = true; remove.IsEnabled = false;
                        try { await Task.Run(() => core.SetInbox(folder.Id, false)); await Reload(); }
                        catch (Exception e) { error.Message = e.Message; error.IsOpen = true; remove.IsEnabled = true; }
                        finally { busy = false; }
                    };
                    item.Children.Add(remove); panel.Children.Add(item);
                }
            }
            await Reload(); if (closed) return;
            var dialog = new ContentDialog { XamlRoot = Shell.XamlRoot, RequestedTheme = Shell.ActualTheme, Title = "Inbox folders", CloseButtonText = "Close", Content = new ScrollViewer { Content = panel, MaxHeight = 520 } };
            dialog.Closing += (_, closing) => closing.Cancel = busy;
            await dialog.ShowAsync();
            if (!closed && currentListening == 3) await ShowListening(3);
        }
        catch (Exception e) { Error(e); }
        finally { metadataDialogOpen = false; }
    }

    private async void CompleteSelectedInbox(object sender, RoutedEventArgs args)
    {
        if (core is null || currentListening != 3 || metadataDialogOpen) return;
        if (TrackList.SelectedItems.Count != 1 || TrackList.SelectedItem is not Track track)
        { Error(new InvalidOperationException("Choose one reviewed file. This only removes it from Needs review; it stays in its folder.")); return; }
        try { await Task.Run(() => core.CompleteInbox(track)); if (!closed) await ShowListening(3); }
        catch (Exception e) { Error(e); }
    }
}
