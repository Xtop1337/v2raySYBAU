using System.Text.Json;
namespace V2RaySybau.Settings;
public sealed class SettingsService
{
    private readonly string _path = Path.Combine(Environment.GetFolderPath(Environment.SpecialFolder.LocalApplicationData), "V2RaySybau", "settings.json");
    public async Task<AppSettings> LoadAsync() => File.Exists(_path) ? await JsonSerializer.DeserializeAsync<AppSettings>(File.OpenRead(_path)) ?? new() : new();
    public async Task SaveAsync(AppSettings settings) { Directory.CreateDirectory(Path.GetDirectoryName(_path)!); await File.WriteAllTextAsync(_path, JsonSerializer.Serialize(settings, new JsonSerializerOptions { WriteIndented = true })); }
}
