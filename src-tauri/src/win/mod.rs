//! Windows integration: system proxy, elevation, running processes. No-ops on other platforms
//! so the application still builds and runs (proxy mode) for development.

pub mod processes;
pub mod sysproxy;

#[cfg(windows)]
pub fn is_elevated() -> bool {
    use windows::Win32::Foundation::{CloseHandle, HANDLE};
    use windows::Win32::Security::{GetTokenInformation, TokenElevation, TOKEN_ELEVATION, TOKEN_QUERY};
    use windows::Win32::System::Threading::{GetCurrentProcess, OpenProcessToken};
    unsafe {
        let mut token = HANDLE::default();
        if OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token).is_err() {
            return false;
        }
        let mut elevation = TOKEN_ELEVATION::default();
        let mut size = 0u32;
        let ok = GetTokenInformation(
            token,
            TokenElevation,
            Some(&mut elevation as *mut _ as *mut _),
            std::mem::size_of::<TOKEN_ELEVATION>() as u32,
            &mut size,
        )
        .is_ok();
        let _ = CloseHandle(token);
        ok && elevation.TokenIsElevated != 0
    }
}

#[cfg(not(windows))]
pub fn is_elevated() -> bool {
    // SAFETY: geteuid has no preconditions.
    unsafe extern "C" {
        fn geteuid() -> u32;
    }
    unsafe { geteuid() == 0 }
}

/// Starts a new elevated copy of the application after this one exits. A hidden `cmd` waits
/// a moment first, so the single-instance guard of the current process is already released.
#[cfg(windows)]
pub fn restart_elevated(extra_args: &str) -> anyhow::Result<()> {
    use windows::core::{HSTRING, PCWSTR};
    use windows::Win32::UI::Shell::ShellExecuteW;
    use windows::Win32::UI::WindowsAndMessaging::SW_HIDE;

    let exe = std::env::current_exe()?;
    let params = format!("/c ping -n 3 127.0.0.1 >nul & start \"\" \"{}\" {extra_args}", exe.display());
    let result =
        unsafe { ShellExecuteW(None, &HSTRING::from("runas"), &HSTRING::from("cmd.exe"), &HSTRING::from(params), PCWSTR::null(), SW_HIDE) };
    // ShellExecute returns a value > 32 on success; the user may also decline the UAC prompt.
    if result.0 as isize <= 32 {
        anyhow::bail!("elevation was cancelled");
    }
    Ok(())
}

#[cfg(not(windows))]
pub fn restart_elevated(_extra_args: &str) -> anyhow::Result<()> {
    anyhow::bail!("restart with administrator rights is only available on Windows")
}
