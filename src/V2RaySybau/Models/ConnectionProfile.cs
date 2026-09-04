namespace V2RaySybau.Models;
public sealed class ConnectionProfile
{
    public Guid Id { get; init; } = Guid.NewGuid();
    public string Name { get; set; } = "New profile";
    public string Protocol { get; set; } = "vless";
    public string Host { get; set; } = "";
    public int Port { get; set; }
    public string UserId { get; set; } = "";
    public string Transport { get; set; } = "tcp";
    public string Security { get; set; } = "none";
    public string Group { get; set; } = "Default";
    public Dictionary<string, string> Parameters { get; set; } = [];
    public DateTimeOffset CreatedAt { get; init; } = DateTimeOffset.UtcNow;
}
public sealed record Subscription(string Url, string Name, DateTimeOffset? UpdatedAt = null);
public sealed record ConnectionEvent(DateTimeOffset At, string Level, string Message);
