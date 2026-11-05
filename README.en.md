[Русский](README.md) | **English**

# V2Ray Sybau

A client for Windows 10/11: paste a key or a subscription link, the servers appear, test their ping and connect.
The logic is written in **Rust** (Tauri 2), the interface in Svelte. Traffic is encrypted by the **Xray-core** and **sing-box** cores.

## Features

- **Protocols:** VLESS (TCP/raw, WebSocket, gRPC, xHTTP, HTTPUpgrade, mKCP; TLS, Reality, XTLS-Vision), VMess, Trojan, Shadowsocks (including 2022), Hysteria2, TUIC.
- **Import:** `vless://`, `vmess://`, `trojan://`, `ss://`, `hy2://`, `tuic://` keys, subscription links, base64 blocks, full Xray and sing-box JSON configs, our own exports and backups. Add them from the input field, from the clipboard (`Ctrl+V` right in the window), from a file or by dropping a file onto the window.
- **Subscriptions and groups:** automatic updates on a schedule and at startup, traffic and expiry from the `subscription-userinfo` header, custom User-Agent.
- **Delay tests:**
  - *real delay* — an HTTP request through the server itself (`generate_204`);
  - *TCP ping* — the time to open a TCP connection;
  - *ICMP ping* — a regular ping, no administrator rights needed.

  Test every server of the list or group at once, or only the chosen server with the method of your choice. Results arrive in real time and are color-coded. Sort the list by delay (fastest or slowest first), by protocol or by transport, and go back to the original order; remove unreachable servers or duplicates.
- **Connection modes:**
  - *Proxy* — local SOCKS5 and HTTP plus the Windows system proxy;
  - *TUN* — a virtual adapter, with the sing-box or the Xray engine. Send all traffic through the tunnel, or **only selected applications** (or all except selected ones).

  A connection only counts as established once traffic really goes through the server: the app shows the exit IP and country, and an unreachable server gives an error instead of "connected".
- **Packet analysis:** latency, jitter, minimum, maximum and packet loss for every node in real time. The list of servers puts the connected one first; pick any server to see its latency chart. Plus a live speed chart and traffic counters: through the proxy and direct.
- **Journal:** every request (from where, to where, through which route: proxy, direct or block) and the cores' events. Filters, search, pause, export to CSV/JSON. Logs are also written to files and kept for 7 days.
- **DNS:** change servers (DoH, DoT, UDP; presets included), DNS from the key or subscription, a local DNS server on a port of your choice, FakeDNS, static records.
- **Routing:** base modes ("everything through the proxy", "local networks direct", "local networks and Russia direct", "rules only"), ad blocking and custom rules by domain, IP, port, process, protocol or network. Rules can be imported and exported. "Domain for routing only" (`routeOnly`) option.
- **Multiplexing** (Mux/XUDP) and TLS **fragmentation** (DPI bypass).
- **Export:** selected servers to JSON, links to the clipboard, a ready core config for any server, a full backup.
- **Conveniences:** snow-white and dark themes (or follow the system), interface scale, Russian and English (or automatic), country flags, notifications ("Creating the tunnel…" → "Tunnel created · IP …"), start with Windows, auto-connect and server scan at startup, delete confirmation, tray icon with the speed.
- **Streamer mode:** hides server addresses, exit IPs, subscription links and folder paths; asks before opening the journal.
- **Conflicting programs:** at launch the app finds what may break the connection (Cloudflare WARP, zapret/winws, GoodbyeDPI, other VPNs and proxy clients) and offers to stop them. You can decline.
- **Factory reset:** settings only, or together with the servers and subscriptions.
- **Subscription site:** click a subscription to open its page (an `https://` link or the `profile-web-page-url` header).
- **New versions:** the app does not update itself. At launch it checks the releases on GitHub and shows what changed, with a link to the installer; a version can be skipped. Xray, sing-box and geoip/geosite are updated with buttons on the About page.

## Installation

1. Open [Releases](https://github.com/Xtop1337/v2raySYBAU/releases) and download `V2Ray Sybau_x.y.z_x64-setup.exe`.
2. Run the installer. No administrator rights needed: the app installs into the user profile. The latest Xray, sing-box and geo files are included.
3. Copy a key or a subscription link and press **From clipboard**.

**TUN** mode needs administrator rights. The app offers to restart itself with them.

## Building from source

You need Rust (stable), Node.js 22 and pnpm 10.

```bash
pnpm install
```

```bash
pwsh ./scripts/fetch-cores.ps1
```

```bash
pnpm tauri build --bundles nsis
```

The installer appears in `target/release/bundle/nsis/`. For development use `pnpm tauri dev`.

Core tests. With the paths to real cores, every generated config is checked by `xray run -test` and `sing-box check`, and the end-to-end tests start local Xray servers:

```bash
XRAY_BIN=/path/to/xray SINGBOX_BIN=/path/to/sing-box cargo test --workspace
```

## Structure

| Directory | Contents |
|---|---|
| `crates/sybau-core` | A pure Rust library: models, link parsing and building, import, subscriptions, Xray and sing-box config generation, pings and batch tests, node monitor, core process control, traffic statistics, journal, core updates. |
| `src-tauri` | The Tauri shell: commands for the UI, background tasks (packet analysis, subscription updates), tray, WinINET system proxy, restart with administrator rights, autostart, new version and conflicting program checks. |
| `ui` | The Svelte 5 + TypeScript interface: Servers, Analytics, Journal, Routing, Subscriptions, Settings and About pages. |
| `scripts` | Core download, version setting and license collection for CI. |

How a connection works:

- **Proxy.** Xray opens SOCKS and HTTP on `127.0.0.1` (ports 10808 and 10809 by default) and, if enabled, sets the system proxy.
- **TUN.** With the sing-box engine, sing-box creates the `SybauTun` adapter and forwards traffic to Xray's local SOCKS; switching servers restarts only Xray. With the Xray engine, Xray's built-in TUN is used. The cores' own processes are excluded from the tunnel to avoid a loop.
- **Hysteria2 and TUIC** run through sing-box directly.
- Before a connection is reported as established, a request through the server must reach the internet; in TUN mode the app also waits until Windows routes traffic into the adapter.

Data, logs and cores are stored in `%LOCALAPPDATA%\V2RaySybau`.

## Licenses

The application code is licensed under the [MIT License](LICENSE). Xray-core (MPL-2.0) and sing-box (GPL-3.0) are separate programs. They are shipped unmodified and run as separate processes.

The installer also includes Wintun (`wintun.dll`, Wintun Prebuilt Binaries License), the geoip/geosite routing files from [Loyalsoldier/v2ray-rules-dat](https://github.com/Loyalsoldier/v2ray-rules-dat) (GPL-3.0; GeoIP is GeoLite2 data by MaxMind, CC BY-SA 4.0) and the Twemoji Country Flags font (MIT, Twemoji graphics under CC BY 4.0).

- The full list of components and their licenses: [THIRD-PARTY-NOTICES.md](THIRD-PARTY-NOTICES.md).
- The license texts are installed with the app: About → Licenses → License texts. The same folder holds the exact component versions (`SOURCES.txt`) and the licenses of every bundled library (`THIRD-PARTY-DEPENDENCIES.txt`).
- The source code of the shipped sing-box and geo data versions (GPL-3.0) is attached to every [release](https://github.com/Xtop1337/v2raySYBAU/releases).
