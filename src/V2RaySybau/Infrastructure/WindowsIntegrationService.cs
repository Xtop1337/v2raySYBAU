using Microsoft.Win32;
using System.Runtime.InteropServices;
namespace V2RaySybau.Infrastructure;
public sealed class WindowsIntegrationService
{
    private const string RunKey = @"Software\Microsoft\Windows\CurrentVersion\Run";
    public void SetAutostart(bool enabled)
    {
        using var key = Registry.CurrentUser.OpenSubKey(RunKey, true)!;
        if (enabled) key.SetValue("V2RaySybau", $"\"{Environment.ProcessPath}\""); else key.DeleteValue("V2RaySybau", false);
    }
    public void SetSystemProxy(bool enabled, int port = 10808)
    {
        using var key = Registry.CurrentUser.OpenSubKey(@"Software\Microsoft\Windows\CurrentVersion\Internet Settings", true)!;
        key.SetValue("ProxyEnable", enabled ? 1 : 0, RegistryValueKind.DWord);
        if (enabled) key.SetValue("ProxyServer", $"socks=127.0.0.1:{port}");
        InternetSetOption(IntPtr.Zero, 39, IntPtr.Zero, 0); InternetSetOption(IntPtr.Zero, 37, IntPtr.Zero, 0);
    }
    [DllImport("wininet.dll", SetLastError = true)] private static extern bool InternetSetOption(IntPtr handle, int option, IntPtr buffer, int length);
}
