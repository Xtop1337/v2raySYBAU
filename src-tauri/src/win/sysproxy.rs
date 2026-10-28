//! Windows system proxy (WinINET, per user): what browsers and most apps use.

#[cfg(windows)]
pub fn set(enabled: bool, http_port: u16, bypass: &str) -> anyhow::Result<()> {
    use windows::Win32::Networking::WinInet::{InternetSetOptionW, INTERNET_OPTION_REFRESH, INTERNET_OPTION_SETTINGS_CHANGED};
    use winreg::enums::{HKEY_CURRENT_USER, KEY_SET_VALUE};
    use winreg::RegKey;

    let key = RegKey::predef(HKEY_CURRENT_USER)
        .open_subkey_with_flags(r"Software\Microsoft\Windows\CurrentVersion\Internet Settings", KEY_SET_VALUE)?;
    key.set_value("ProxyEnable", &u32::from(enabled))?;
    if enabled {
        // A plain host:port is used for every scheme; Xray's HTTP inbound handles CONNECT for HTTPS.
        key.set_value("ProxyServer", &format!("127.0.0.1:{http_port}"))?;
        key.set_value("ProxyOverride", &bypass.to_string())?;
    }
    unsafe {
        let _ = InternetSetOptionW(None, INTERNET_OPTION_SETTINGS_CHANGED, None, 0);
        let _ = InternetSetOptionW(None, INTERNET_OPTION_REFRESH, None, 0);
    }
    Ok(())
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
