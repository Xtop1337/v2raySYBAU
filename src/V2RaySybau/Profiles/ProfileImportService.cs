using System.Text;
using System.Text.Json;
using V2RaySybau.Models;
namespace V2RaySybau.Profiles;

public sealed class ProfileImportService
{
    public IReadOnlyList<ConnectionProfile> Import(string text)
    {
        var profiles = new List<ConnectionProfile>();
        foreach (var line in text.Split(['\r', '\n'], StringSplitOptions.RemoveEmptyEntries | StringSplitOptions.TrimEntries))
        {
            if (line.StartsWith("vless://", StringComparison.OrdinalIgnoreCase)) profiles.Add(ParseVless(line));
            else if (line.StartsWith("vmess://", StringComparison.OrdinalIgnoreCase)) profiles.Add(ParseVmess(line));
        }
        return profiles;
    }
    public ConnectionProfile ParseVless(string uri)
    {
        var value = new Uri(uri);
        var parameters = ParseQuery(value.Query);
        return new ConnectionProfile {
            Protocol = "vless", UserId = value.UserInfo, Host = value.Host, Port = value.Port,
            Name = Uri.UnescapeDataString(value.Fragment.TrimStart('#')).DefaultIfBlank($"{value.Host}:{value.Port}"),
            Transport = parameters.GetValueOrDefault("type", "tcp"), Security = parameters.GetValueOrDefault("security", "none"), Parameters = parameters };
    }
    public ConnectionProfile ParseVmess(string uri)
    {
        var payload = uri["vmess://".Length..];
        var json = Encoding.UTF8.GetString(Convert.FromBase64String(PadBase64(payload)));
        using var document = JsonDocument.Parse(json);
        var root = document.RootElement;
        string Read(string key) => root.TryGetProperty(key, out var v) ? v.GetString() ?? "" : "";
        _ = int.TryParse(Read("port"), out var port);
        return new ConnectionProfile {
            Protocol = "vmess", Name = Read("ps").DefaultIfBlank(Read("add")), Host = Read("add"), Port = port,
            UserId = Read("id"), Transport = Read("net").DefaultIfBlank("tcp"), Security = Read("scy").DefaultIfBlank("auto"),
            Parameters = new Dictionary<string, string> { ["tls"] = Read("tls"), ["path"] = Read("path"), ["host"] = Read("host"), ["sni"] = Read("sni") } };
    }
    public async Task<IReadOnlyList<ConnectionProfile>> ImportSubscriptionAsync(string url, HttpClient client, CancellationToken cancellationToken = default)
    {
        var content = await client.GetStringAsync(url, cancellationToken);
        try { content = Encoding.UTF8.GetString(Convert.FromBase64String(PadBase64(content.Trim()))); } catch (FormatException) { }
        return Import(content);
    }
    private static Dictionary<string, string> ParseQuery(string query) => query.TrimStart('?').Split('&', StringSplitOptions.RemoveEmptyEntries).Select(x => x.Split('=', 2)).ToDictionary(x => Uri.UnescapeDataString(x[0]), x => x.Length > 1 ? Uri.UnescapeDataString(x[1]) : "");
    private static string PadBase64(string value) => value.Replace('-', '+').Replace('_', '/') + new string('=', (4 - value.Length % 4) % 4);
}
file static class StringExtensions { public static string DefaultIfBlank(this string value, string fallback) => string.IsNullOrWhiteSpace(value) ? fallback : value; }
