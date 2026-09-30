using Microsoft.UI.Xaml;
using System.Runtime.InteropServices;
namespace Spiewak;
public partial class MainWindow : Window
{
    [DllImport("spiewak_core", CallingConvention = CallingConvention.Cdecl)]
    private static extern uint spiewak_abi_version();
    public MainWindow()
    {
        InitializeComponent();
        Title = "Śpiewak";
        CoreStatus.Text = $"Native Rust core connected · ABI {spiewak_abi_version()}";
        AppWindow.Resize(new Windows.Graphics.SizeInt32(1200, 800));
    }
}
