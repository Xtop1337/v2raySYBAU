# V2Ray Sybau

Windows-first desktop client for VLESS/VMess profiles, backed by a compatible **Xray/V2Ray** core process. The project targets **.NET 8 + WPF** and is intentionally split into independent application layers:

| Layer | Location | Responsibility |
|---|---|---|
| UI | `UI/` | WPF shell: profile list/editor, import, group filtering, export, confirmation, connection log and client options. |
| Profiles | `Profiles/` | VLESS and VMess URI parsing plus HTTP(S) subscription import. |
| Settings | `Settings/` | Theme, language, autostart, auto-connect, system proxy and last profile. |
| Network core | `Core/` | Translates a profile into Xray/V2Ray JSON and controls the core process with captured events. |
| Storage | `Storage/` | Local JSON persistence and selected-profile export. |
| Windows integration | `Infrastructure/` | Current-user startup registration and WinINET system proxy switching. |

## Running on Windows

1. Install the [.NET 8 Desktop Runtime](https://dotnet.microsoft.com/download/dotnet/8.0).
2. Build with `dotnet build V2RaySybau.sln`.
3. Put a compatible `xray.exe` (or rename a compatible V2Ray executable to `xray.exe`) in `core/xray.exe` next to the executable.
4. Copy one or more `vless://` / `vmess://` links, or a subscription URL, to the clipboard and select **Импорт**.

Profiles and settings are stored under `%LOCALAPPDATA%\V2RaySybau`. Export only serializes the profile selection to a user-selected JSON file. The core executable is never bundled; obtain it from its official project and comply with its license.
