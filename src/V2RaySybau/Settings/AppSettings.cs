namespace V2RaySybau.Settings;

public enum ThemeMode { System, Light, Dark }

public sealed class AppSettings
{
    public ThemeMode Theme { get; set; } = ThemeMode.System;
    public string Language { get; set; } = "auto";
    public bool StartWithWindows { get; set; }
    public bool AutoConnect { get; set; }
    public bool SystemProxy { get; set; }
    public string LastSelectedProfileId { get; set; } = "";
}
