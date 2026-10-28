//! JSON persistence under the data directory (`%LOCALAPPDATA%\V2RaySybau` on Windows).

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::de::DeserializeOwned;
use serde::Serialize;

use crate::import::{Backup, BACKUP_APP};
use crate::model::{Profile, Subscription};
use crate::settings::Settings;

#[derive(Debug, Clone)]
pub struct Storage {
    root: PathBuf,
}

impl Storage {
    pub fn new(root: impl Into<PathBuf>) -> Result<Self> {
        let root = root.into();
        fs::create_dir_all(&root).with_context(|| format!("create {}", root.display()))?;
        Ok(Storage { root })
    }

    /// Default data directory: `%LOCALAPPDATA%\V2RaySybau`, `~/.local/share/V2RaySybau` elsewhere.
    pub fn default_root() -> PathBuf {
        let base = std::env::var_os("LOCALAPPDATA")
            .map(PathBuf::from)
            .or_else(|| std::env::var_os("XDG_DATA_HOME").map(PathBuf::from))
            .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".local/share")))
            .unwrap_or_else(std::env::temp_dir);
        base.join("V2RaySybau")
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn path(&self, name: &str) -> PathBuf {
        self.root.join(name)
    }

    pub fn dir(&self, name: &str) -> Result<PathBuf> {
        let dir = self.root.join(name);
        fs::create_dir_all(&dir)?;
        Ok(dir)
    }

    pub fn load_profiles(&self) -> Result<Vec<Profile>> {
        self.load("profiles.json")
    }

    pub fn save_profiles(&self, profiles: &[Profile]) -> Result<()> {
        self.save("profiles.json", &profiles)
    }

    pub fn load_subscriptions(&self) -> Result<Vec<Subscription>> {
        self.load("subscriptions.json")
    }

    pub fn save_subscriptions(&self, subscriptions: &[Subscription]) -> Result<()> {
        self.save("subscriptions.json", &subscriptions)
    }

    pub fn load_settings(&self) -> Result<Settings> {
        self.load("settings.json")
    }

    pub fn save_settings(&self, settings: &Settings) -> Result<()> {
        self.save("settings.json", settings)
    }

    fn load<T: DeserializeOwned + Default>(&self, name: &str) -> Result<T> {
        let path = self.path(name);
        if !path.exists() {
            return Ok(T::default());
        }
        let text = fs::read_to_string(&path).with_context(|| format!("read {}", path.display()))?;
        match serde_json::from_str(&text) {
            Ok(value) => Ok(value),
            Err(e) => {
                // Keep the broken file for inspection instead of silently overwriting it later.
                let _ = fs::copy(&path, path.with_extension("json.broken"));
                Err(e).with_context(|| format!("parse {}", path.display()))
            }
        }
    }

    fn save<T: Serialize + ?Sized>(&self, name: &str, value: &T) -> Result<()> {
        write_atomic(&self.path(name), serde_json::to_string_pretty(value)?.as_bytes())
    }
}

/// Writes to a temp file next to `path` and renames it over, so a crash never leaves half a file.
pub fn write_atomic(path: &Path, bytes: &[u8]) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let tmp = path.with_extension("tmp");
    fs::write(&tmp, bytes).with_context(|| format!("write {}", tmp.display()))?;
    fs::rename(&tmp, path).with_context(|| format!("replace {}", path.display()))?;
    Ok(())
}

pub fn export_profiles(profiles: &[Profile]) -> String {
    serde_json::to_string_pretty(profiles).unwrap_or_default()
}

pub fn export_backup(profiles: &[Profile], subscriptions: &[Subscription], settings: &Settings) -> String {
    let backup = Backup {
        app: BACKUP_APP.into(),
        version: 1,
        profiles: profiles.to_vec(),
        subscriptions: subscriptions.to_vec(),
        rules: Some(settings.routing.rules.clone()),
        settings: Some(settings.clone()),
    };
    serde_json::to_string_pretty(&backup).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn saves_and_loads() {
        let dir = tempfile::tempdir().unwrap();
        let storage = Storage::new(dir.path()).unwrap();
        assert!(storage.load_profiles().unwrap().is_empty());
        let profile = Profile { name: "x".into(), address: "a.com".into(), ..Profile::default() };
        storage.save_profiles(std::slice::from_ref(&profile)).unwrap();
        assert_eq!(storage.load_profiles().unwrap(), vec![profile]);

        let mut settings = Settings::default();
        settings.proxy.socks_port = 2080;
        storage.save_settings(&settings).unwrap();
        assert_eq!(storage.load_settings().unwrap().proxy.socks_port, 2080);
    }

    #[test]
    fn partial_settings_fill_defaults() {
        let settings: Settings = serde_json::from_str(r#"{"general": {"theme": "dark"}}"#).unwrap();
        assert_eq!(settings.general.theme, crate::settings::Theme::Dark);
        assert!(settings.general.confirm_delete);
        assert_eq!(settings.proxy.socks_port, 10808);
    }
}
