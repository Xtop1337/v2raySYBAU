using System.Diagnostics;
using V2RaySybau.Models;

namespace V2RaySybau.Core;

public sealed class CoreProcessService : IAsyncDisposable
{
    private Process? _process;
    private string? _configPath;

    public event EventHandler<ConnectionEvent>? EventReceived;

    public bool IsRunning => _process is { HasExited: false };

    public async Task StartAsync(string executable, string config, CancellationToken cancellationToken = default)
    {
        cancellationToken.ThrowIfCancellationRequested();
        await StopAsync();

        if (string.IsNullOrWhiteSpace(executable))
            throw new ArgumentException("Executable path is required.", nameof(executable));

        if (!File.Exists(executable))
            throw new FileNotFoundException("Xray/V2Ray executable was not found.", executable);

        var tempFolder = Path.Combine(Path.GetTempPath(), "V2RaySybau");
        Directory.CreateDirectory(tempFolder);

        _configPath = Path.Combine(tempFolder, $"config-{Guid.NewGuid():N}.json");
        await File.WriteAllTextAsync(_configPath, config, cancellationToken);

        var startInfo = new ProcessStartInfo
        {
            FileName = executable,
            Arguments = $"run -c \"{_configPath}\"",
            WorkingDirectory = Path.GetDirectoryName(executable) ?? Environment.CurrentDirectory,
            UseShellExecute = false,
            RedirectStandardOutput = true,
            RedirectStandardError = true,
            CreateNoWindow = true,
            ErrorDialog = false
        };

        _process = new Process
        {
            StartInfo = startInfo,
            EnableRaisingEvents = true
        };

        _process.OutputDataReceived += (_, args) => Publish("info", args.Data);
        _process.ErrorDataReceived += (_, args) => Publish("warning", args.Data);
        _process.Exited += (_, _) =>
        {
            Publish("error", $"Core exited with code {_process?.ExitCode ?? 0}.");
            _configPath = null;
        };

        _process.Start();
        _process.BeginOutputReadLine();
        _process.BeginErrorReadLine();

        Publish("info", $"Core started: {executable}");
    }

    public async Task StopAsync()
    {
        if (_process is null)
            return;

        try
        {
            if (!_process.HasExited)
            {
                _process.Kill(entireProcessTree: true);
                await _process.WaitForExitAsync();
            }
        }
        catch (InvalidOperationException)
        {
            // Process has already exited or was disposed.
        }
        catch (Exception ex)
        {
            Publish("warning", $"StopAsync warning: {ex.Message}");
        }
        finally
        {
            _process.Dispose();
            _process = null;

            if (!string.IsNullOrWhiteSpace(_configPath) && File.Exists(_configPath))
            {
                try
                {
                    File.Delete(_configPath);
                }
                catch
                {
                    // Ignore cleanup failures; temp file is best-effort only.
                }
            }

            _configPath = null;
            Publish("info", "Core stopped");
        }
    }

    public async ValueTask DisposeAsync() => await StopAsync();

    private void Publish(string level, string? message)
    {
        if (string.IsNullOrWhiteSpace(message))
            return;

        EventReceived?.Invoke(this, new ConnectionEvent(DateTimeOffset.Now, level, message.Trim()));
    }
}
