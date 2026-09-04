using System.Diagnostics;
using V2RaySybau.Models;
namespace V2RaySybau.Core;
public sealed class CoreProcessService : IAsyncDisposable
{
    private Process? _process;
    public event EventHandler<ConnectionEvent>? EventReceived;
    public bool IsRunning => _process is { HasExited: false };
    public async Task StartAsync(string executable, string config, CancellationToken cancellationToken = default)
    {
        await StopAsync(); var path = Path.Combine(Path.GetTempPath(), $"v2raysybau-{Guid.NewGuid():N}.json"); await File.WriteAllTextAsync(path, config, cancellationToken);
        _process = new Process { StartInfo = new ProcessStartInfo(executable, $"run -c \"{path}\"") { UseShellExecute = false, RedirectStandardError = true, RedirectStandardOutput = true, CreateNoWindow = true, StandardOutputEncoding = System.Text.Encoding.UTF8 } };
        _process.OutputDataReceived += (_, e) => Publish("info", e.Data); _process.ErrorDataReceived += (_, e) => Publish("warning", e.Data); _process.Start(); _process.BeginOutputReadLine(); _process.BeginErrorReadLine(); Publish("info", "Core started");
    }
    public Task StopAsync() { if (_process is { HasExited: false }) { _process.Kill(true); Publish("info", "Core stopped"); } _process?.Dispose(); _process = null; return Task.CompletedTask; }
    private void Publish(string level, string? message) { if (!string.IsNullOrWhiteSpace(message)) EventReceived?.Invoke(this, new ConnectionEvent(DateTimeOffset.Now, level, message)); }
    public async ValueTask DisposeAsync() => await StopAsync();
}
