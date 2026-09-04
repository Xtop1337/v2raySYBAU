using System.Text.Json;
using V2RaySybau.Models;
namespace V2RaySybau.Storage;
public sealed class ProfileStore
{
    private readonly string _path;
    private readonly JsonSerializerOptions _json = new() { WriteIndented = true };
    public ProfileStore(string? path = null) => _path = path ?? Path.Combine(Environment.GetFolderPath(Environment.SpecialFolder.LocalApplicationData), "V2RaySybau", "profiles.json");
    public async Task<List<ConnectionProfile>> LoadAsync() { if (!File.Exists(_path)) return []; await using var stream = File.OpenRead(_path); return await JsonSerializer.DeserializeAsync<List<ConnectionProfile>>(stream, _json) ?? []; }
    public async Task SaveAsync(IEnumerable<ConnectionProfile> profiles) { Directory.CreateDirectory(Path.GetDirectoryName(_path)!); await using var stream = File.Create(_path); await JsonSerializer.SerializeAsync(stream, profiles, _json); }
    public string Export(IEnumerable<ConnectionProfile> profiles) => JsonSerializer.Serialize(profiles, _json);
}
