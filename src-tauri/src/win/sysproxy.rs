//! Windows system proxy (WinINET, per user): what browsers and most apps use.

/// Sets the proxy through `InternetSetOption(INTERNET_OPTION_PER_CONNECTION_OPTION)`, like the
/// "Proxy" page of Windows settings does. Writing only the `ProxyEnable`/`ProxyServer` registry
/// values is not enough: WinHTTP (Chrome, Edge, most apps) reads the per-connection settings, and
/// a PAC script or "Automatically detect settings" would still win over a manual proxy.
#[cfg(windows)]
pub fn set(enabled: bool, http_port: u16, bypass: &str) -> anyhow::Result<()> {
    use windows::core::PWSTR;
    use windows::Win32::Networking::WinInet::{
        InternetSetOptionW, INTERNET_OPTION_PER_CONNECTION_OPTION, INTERNET_OPTION_PROXY_SETTINGS_CHANGED, INTERNET_OPTION_REFRESH,
        INTERNET_OPTION_SETTINGS_CHANGED, INTERNET_PER_CONN_FLAGS, INTERNET_PER_CONN_OPTIONW, INTERNET_PER_CONN_OPTIONW_0,
        INTERNET_PER_CONN_OPTION_LISTW, INTERNET_PER_CONN_PROXY_BYPASS, INTERNET_PER_CONN_PROXY_SERVER, PROXY_TYPE_DIRECT,
        PROXY_TYPE_PROXY,
    };

    let wide = |s: &str| s.encode_utf16().chain(Some(0)).collect::<Vec<u16>>();
    // A plain host:port is used for every scheme; Xray's HTTP inbound handles CONNECT for HTTPS.
    let mut server = wide(&format!("127.0.0.1:{http_port}"));
    let mut bypass = wide(bypass);
    let flags = if enabled { PROXY_TYPE_DIRECT | PROXY_TYPE_PROXY } else { PROXY_TYPE_DIRECT };
    let mut options =
        vec![INTERNET_PER_CONN_OPTIONW { dwOption: INTERNET_PER_CONN_FLAGS, Value: INTERNET_PER_CONN_OPTIONW_0 { dwValue: flags } }];
    if enabled {
        options.push(INTERNET_PER_CONN_OPTIONW {
            dwOption: INTERNET_PER_CONN_PROXY_SERVER,
            Value: INTERNET_PER_CONN_OPTIONW_0 { pszValue: PWSTR(server.as_mut_ptr()) },
        });
        options.push(INTERNET_PER_CONN_OPTIONW {
            dwOption: INTERNET_PER_CONN_PROXY_BYPASS,
            Value: INTERNET_PER_CONN_OPTIONW_0 { pszValue: PWSTR(bypass.as_mut_ptr()) },
        });
    }

    // The LAN settings (no connection name) and every dial-up / VPN / PPPoE entry, which have
    // their own proxy settings that apply while they are connected.
    let mut connections: Vec<Option<Vec<u16>>> = vec![None];
    connections.extend(ras_entries().into_iter().map(Some));
    let mut result = Ok(());
    for mut name in connections {
        let mut list = INTERNET_PER_CONN_OPTION_LISTW {
            dwSize: std::mem::size_of::<INTERNET_PER_CONN_OPTION_LISTW>() as u32,
            pszConnection: name.as_mut().map_or(PWSTR::null(), |n| PWSTR(n.as_mut_ptr())),
            dwOptionCount: options.len() as u32,
            dwOptionError: 0,
            pOptions: options.as_mut_ptr(),
        };
        let applied = unsafe {
            InternetSetOptionW(
                None,
                INTERNET_OPTION_PER_CONNECTION_OPTION,
                Some(std::ptr::from_mut(&mut list).cast()),
                std::mem::size_of::<INTERNET_PER_CONN_OPTION_LISTW>() as u32,
            )
        };
        if name.is_none() {
            result = applied.map_err(|e| anyhow::anyhow!("InternetSetOption: {e}"));
        }
    }
    unsafe {
        let _ = InternetSetOptionW(None, INTERNET_OPTION_PROXY_SETTINGS_CHANGED, None, 0);
        let _ = InternetSetOptionW(None, INTERNET_OPTION_SETTINGS_CHANGED, None, 0);
        let _ = InternetSetOptionW(None, INTERNET_OPTION_REFRESH, None, 0);
    }
    result
}

/// Names of the dial-up / VPN / PPPoE connections of the user's phonebook.
#[cfg(windows)]
fn ras_entries() -> Vec<Vec<u16>> {
    use windows::core::PCWSTR;
    use windows::Win32::NetworkManagement::Rras::{RasEnumEntriesW, RASENTRYNAMEW};

    let empty =
        RASENTRYNAMEW { dwSize: std::mem::size_of::<RASENTRYNAMEW>() as u32, szEntryName: [0; 257], dwFlags: 0, szPhonebookPath: [0; 261] };
    let mut entries = vec![empty; 64];
    let mut bytes = (entries.len() * std::mem::size_of::<RASENTRYNAMEW>()) as u32;
    let mut count = 0u32;
    let status = unsafe { RasEnumEntriesW(PCWSTR::null(), PCWSTR::null(), Some(entries.as_mut_ptr()), &mut bytes, &mut count) };
    if status != 0 {
        return Vec::new();
    }
    entries
        .iter()
        .take(count as usize)
        .map(|e| {
            let len = e.szEntryName.iter().position(|&c| c == 0).unwrap_or(e.szEntryName.len());
            e.szEntryName[..len].iter().copied().chain(Some(0)).collect()
        })
        .collect()
}

/// What WinHTTP (and so Chrome/Edge) actually sees: `None` when our proxy is in effect,
/// otherwise a description of what overrides it.
#[cfg(windows)]
pub fn check(http_port: u16) -> Option<String> {
    use windows::Win32::Foundation::{GlobalFree, HGLOBAL};
    use windows::Win32::Networking::WinHttp::{WinHttpGetIEProxyConfigForCurrentUser, WINHTTP_CURRENT_USER_IE_PROXY_CONFIG};

    let mut config = WINHTTP_CURRENT_USER_IE_PROXY_CONFIG::default();
    if unsafe { WinHttpGetIEProxyConfigForCurrentUser(&mut config) }.is_err() {
        return None;
    }
    let take = |p: windows::core::PWSTR| {
        if p.is_null() {
            return String::new();
        }
        let s = unsafe { p.to_string() }.unwrap_or_default();
        let _ = unsafe { GlobalFree(HGLOBAL(p.0.cast())) };
        s
    };
    let (pac, proxy, _bypass) = (take(config.lpszAutoConfigUrl), take(config.lpszProxy), take(config.lpszProxyBypass));
    if !pac.is_empty() {
        Some(format!("a PAC script is set ({pac})"))
    } else if config.fAutoDetect.as_bool() {
        Some("\"Automatically detect settings\" is on".into())
    } else if !proxy.contains(&format!("127.0.0.1:{http_port}")) {
        Some(format!("the system proxy is \"{proxy}\""))
    } else {
        None
    }
}

/// True when the system proxy currently points at our local port.
#[cfg(windows)]
pub fn is_ours(http_port: u16) -> bool {
    use winreg::enums::HKEY_CURRENT_USER;
    use winreg::RegKey;
    let Ok(key) = RegKey::predef(HKEY_CURRENT_USER).open_subkey(r"Software\Microsoft\Windows\CurrentVersion\Internet Settings") else {
        return false;
    };
    let enabled: u32 = key.get_value("ProxyEnable").unwrap_or(0);
    let server: String = key.get_value("ProxyServer").unwrap_or_default();
    enabled == 1 && server == format!("127.0.0.1:{http_port}")
}

#[cfg(not(windows))]
pub fn set(enabled: bool, http_port: u16, _bypass: &str) -> anyhow::Result<()> {
    tracing::info!("system proxy {} (127.0.0.1:{http_port}) is only applied on Windows", if enabled { "on" } else { "off" });
    Ok(())
}

#[cfg(not(windows))]
pub fn is_ours(_http_port: u16) -> bool {
    false
}

#[cfg(not(windows))]
pub fn check(_http_port: u16) -> Option<String> {
    None
}
