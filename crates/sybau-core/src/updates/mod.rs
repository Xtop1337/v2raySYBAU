//! Updates from GitHub Releases: release lookup and core (Xray, sing-box, geo files) updates.
//! The application itself is updated by the Tauri updater plugin from the same releases.

use std::io::Read;
use std::path::{Path, PathBuf};
use std::time::Duration;

use anyhow::{anyhow, bail, Context, Result};
use serde::{Deserialize, Serialize};

use crate::runtime::CoreKind;
use crate::storage::write_atomic;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Release {
    #[serde(alias = "tag_name")]
    pub tag_name: String,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub body: Option<String>,
    #[serde(alias = "html_url")]
    pub html_url: String,
    #[serde(default)]
    pub prerelease: bool,
    #[serde(default)]
    pub draft: bool,
    #[serde(default, alias = "published_at")]
    pub published_at: Option<String>,
    #[serde(default)]
    pub assets: Vec<Asset>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Asset {
    pub name: String,
    #[serde(alias = "browser_download_url")]
    pub browser_download_url: String,
    #[serde(default)]
    pub size: u64,
}

impl Release {
    pub fn version(&self) -> Option<semver::Version> {
        parse_version(&self.tag_name)
    }
}

/// Lenient semver: `v26.3.27`, `1.14.4`, `v1.0` all parse.
pub fn parse_version(tag: &str) -> Option<semver::Version> {
    let trimmed = tag.trim().trim_start_matches(['v', 'V']);
    if let Ok(v) = semver::Version::parse(trimmed) {
        return Some(v);
    }
    let mut parts = trimmed.split(['.', '-']).map(|p| p.parse::<u64>().ok());
    let major = parts.next()??;
    let minor = parts.next().flatten().unwrap_or(0);
    let patch = parts.next().flatten().unwrap_or(0);
    Some(semver::Version::new(major, minor, patch))
}

pub fn is_newer(candidate: &str, current: &str) -> bool {
    match (parse_version(candidate), parse_version(current)) {
        (Some(a), Some(b)) => a > b,
        (Some(_), None) => true,
        _ => false,
    }
}

pub fn http_client(proxy: Option<&str>) -> Result<reqwest::Client> {
    let mut builder = reqwest::Client::builder()
        .user_agent(format!("{}/{}", crate::APP_NAME, crate::APP_VERSION))
        .connect_timeout(Duration::from_secs(15));
    builder = match proxy {
        Some(proxy) => builder.proxy(reqwest::Proxy::all(proxy)?),
        None => builder.no_proxy(),
    };
    Ok(builder.build()?)
}

/// Latest release of `owner/repo` (skipping drafts, and pre-releases unless allowed).
pub async fn latest_release(client: &reqwest::Client, repo: &str, allow_prerelease: bool) -> Result<Release> {
    let url = format!("https://api.github.com/repos/{repo}/releases?per_page=10");
    let releases: Vec<Release> =
        client.get(url).header("Accept", "application/vnd.github+json").send().await?.error_for_status()?.json().await?;
    releases.into_iter().find(|r| !r.draft && (allow_prerelease || !r.prerelease)).ok_or_else(|| anyhow!("no releases in {repo}"))
}

pub async fn download(client: &reqwest::Client, url: &str) -> Result<Vec<u8>> {
    let response = client.get(url).timeout(Duration::from_secs(600)).send().await?.error_for_status()?;
    Ok(response.bytes().await?.to_vec())
}

/// Asset name of the Windows x64 build (or the current platform when testing elsewhere).
pub fn core_asset(kind: CoreKind, release: &Release) -> Option<&Asset> {
    let (os, arch) = if cfg!(windows) {
        ("windows", "64")
    } else if cfg!(target_os = "macos") {
        ("macos", "64")
    } else {
        ("linux", "64")
    };
    release.assets.iter().find(|a| {
        let name = a.name.to_ascii_lowercase();
        match kind {
            CoreKind::Xray => name == format!("xray-{os}-{arch}.zip"),
            CoreKind::SingBox => {
                name.starts_with("sing-box-")
                    && name.ends_with(if cfg!(windows) {
                        "-windows-amd64.zip"
                    } else if cfg!(target_os = "macos") {
                        "-darwin-amd64.tar.gz"
                    } else {
                        "-linux-amd64.tar.gz"
                    })
                    && !name.contains("legacy")
            }
        }
    })
}

pub const XRAY_REPO: &str = "XTLS/Xray-core";
pub const SINGBOX_REPO: &str = "SagerNet/sing-box";
pub const GEO_BASE_URL: &str = "https://github.com/Loyalsoldier/v2ray-rules-dat/releases/latest/download";

/// Extracts the files of a core archive into `dir`. Files are written next to the old ones
/// and then renamed over them; the core must be stopped before (Windows locks running exes).
pub fn install_core_zip(kind: CoreKind, archive: &[u8], dir: &Path) -> Result<Vec<PathBuf>> {
    let mut zip = zip::ZipArchive::new(std::io::Cursor::new(archive)).context("open core archive")?;
    let mut installed = Vec::new();
    for i in 0..zip.len() {
        let mut file = zip.by_index(i)?;
        if file.is_dir() {
            continue;
        }
        let Some(name) = Path::new(file.name()).file_name().map(|n| n.to_string_lossy().to_string()) else { continue };
        let lower = name.to_ascii_lowercase();
        let wanted = match kind {
            CoreKind::Xray => lower == CoreKind::Xray.exe_name() || lower.ends_with(".dat"),
            CoreKind::SingBox => lower == CoreKind::SingBox.exe_name() || lower.ends_with(".dll"),
        };
        if !wanted {
            continue;
        }
        let mut bytes = Vec::new();
        file.read_to_end(&mut bytes)?;
        let target = dir.join(&name);
        write_atomic(&target, &bytes)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            if !lower.ends_with(".dat") {
                std::fs::set_permissions(&target, std::fs::Permissions::from_mode(0o755))?;
            }
        }
        installed.push(target);
    }
    if !installed.iter().any(|p| p.file_name().is_some_and(|n| n == kind.exe_name())) {
        bail!("{} executable not found in the archive", kind.label());
    }
    Ok(installed)
}

/// Downloads the latest release of a core and installs it. Returns the installed version.
pub async fn update_core(client: &reqwest::Client, kind: CoreKind, dir: &Path) -> Result<String> {
    let repo = match kind {
        CoreKind::Xray => XRAY_REPO,
        CoreKind::SingBox => SINGBOX_REPO,
    };
    let release = latest_release(client, repo, false).await?;
    let asset = core_asset(kind, &release).ok_or_else(|| anyhow!("no {} build in release {}", kind.label(), release.tag_name))?;
    let archive = download(client, &asset.browser_download_url).await?;
    install_core_zip(kind, &archive, dir)?;
    Ok(release.tag_name.trim_start_matches('v').to_string())
}

/// Updates `geoip.dat` and `geosite.dat` (Loyalsoldier rules, a superset of the v2fly lists).
pub async fn update_geo(client: &reqwest::Client, dir: &Path) -> Result<()> {
    for name in ["geoip.dat", "geosite.dat"] {
        let bytes = download(client, &format!("{GEO_BASE_URL}/{name}")).await?;
        if bytes.len() < 1024 {
            bail!("{name} download is too small");
        }
        write_atomic(&dir.join(name), &bytes)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn compares_versions() {
        assert!(is_newer("v1.2.0", "1.1.9"));
        assert!(is_newer("v26.3.27", "25.12.8"));
        assert!(!is_newer("v1.0.0", "1.0.0"));
        assert!(is_newer("1.0.1", "garbage"));
        assert_eq!(parse_version("v1.14").unwrap(), semver::Version::new(1, 14, 0));
    }

    #[test]
    fn parses_github_release_json() {
        let json = r#"[{"tag_name":"v1.1.0","name":"1.1","body":"notes","html_url":"https://x","prerelease":true,"draft":false,"assets":[]},
                       {"tag_name":"v1.0.0","html_url":"https://y","assets":[{"name":"Xray-windows-64.zip","browser_download_url":"https://d","size":5}]}]"#;
        let releases: Vec<Release> = serde_json::from_str(json).unwrap();
        assert_eq!(releases.len(), 2);
        assert!(releases[0].prerelease);
        assert_eq!(releases[1].assets[0].name, "Xray-windows-64.zip");
    }

    #[test]
    fn installs_core_from_zip() {
        let mut buffer = std::io::Cursor::new(Vec::new());
        {
            let mut zip = zip::ZipWriter::new(&mut buffer);
            let options = zip::write::SimpleFileOptions::default();
            zip.start_file(format!("folder/{}", CoreKind::Xray.exe_name()), options).unwrap();
            zip.write_all(b"binary").unwrap();
            zip.start_file("geoip.dat", options).unwrap();
            zip.write_all(b"geo").unwrap();
            zip.start_file("README.md", options).unwrap();
            zip.write_all(b"readme").unwrap();
        }
        let dir = tempfile::tempdir().unwrap();
        let installed = install_core_zip(CoreKind::Xray, buffer.get_ref(), dir.path()).unwrap();
        assert_eq!(installed.len(), 2);
        assert_eq!(std::fs::read(dir.path().join(CoreKind::Xray.exe_name())).unwrap(), b"binary");
        assert!(!dir.path().join("README.md").exists());
    }
}
