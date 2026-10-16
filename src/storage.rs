// Copyright (c) 2026 Benja (Benjamacs1)
// SPDX-License-Identifier: MIT
use crate::model::Settings;
use anyhow::{Context, Result};
use std::{fs, path::PathBuf};
use zeroize::Zeroizing;

#[derive(Clone)]
pub struct DiskStore {
    pub dir: PathBuf,
}
impl DiskStore {
    pub fn default_dir() -> Result<PathBuf> {
        directories::ProjectDirs::from("uk", "CharlieKirk", "KirkCompanion")
            .map(|d| d.config_dir().to_owned())
            .context("Could not find a configuration folder.")
    }
    pub fn load(&self) -> Result<Settings> {
        let file = self.dir.join("settings.json");
        if !file.exists() {
            return Ok(Settings::default());
        }
        anyhow::ensure!(
            fs::metadata(&file)?.len() <= 1_048_576,
            "Saved settings are too large."
        );
        let settings: Settings = serde_json::from_slice(&fs::read(file)?)
            .context("Saved settings could not be read.")?;
        settings.validate()?;
        Ok(settings)
    }
    pub fn save(&self, settings: &Settings) -> Result<()> {
        settings.validate()?;
        fs::create_dir_all(&self.dir).context("Could not create the settings folder.")?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&self.dir, fs::Permissions::from_mode(0o700))?;
        }
        let path = self.dir.join("settings.json");
        // Preserve a damaged original before replacing it with a freshly configured file.
        if path.exists() && self.load().is_err() {
            fs::copy(
                &path,
                self.dir
                    .join(format!("settings-damaged-{}.json", crate::model::now_ms())),
            )?;
        }
        let temp = self.dir.join("settings.json.new");
        fs::write(&temp, serde_json::to_vec_pretty(settings)?)
            .context("Could not save settings.")?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&temp, fs::Permissions::from_mode(0o600))?;
        }
        fs::rename(temp, path).context("Could not replace saved settings.")?;
        Ok(())
    }
}

pub trait Credentials: Send + Sync {
    fn load(&self) -> Result<Option<Zeroizing<String>>>;
    /// False means a deliberate memory-only session (e.g. no available OS credential store).
    fn save(&self, token: &str) -> Result<bool>;
    fn delete(&self) -> Result<()>;
}
pub struct OsCredentials {
    pub account: String,
}
impl OsCredentials {
    fn entry(&self) -> Result<keyring::v1::Entry> {
        Ok(keyring::v1::Entry::new(
            "uk.charlie-kirk.companion",
            &self.account,
        )?)
    }
}
impl Credentials for OsCredentials {
    fn load(&self) -> Result<Option<Zeroizing<String>>> {
        match self.entry()?.get_password() {
            Ok(token) => Ok(Some(Zeroizing::new(token))),
            Err(keyring::v1::Error::NoEntry) => Ok(None),
            Err(e) => Err(e.into()),
        }
    }
    fn save(&self, token: &str) -> Result<bool> {
        self.entry()?.set_password(token)?;
        Ok(true)
    }
    fn delete(&self) -> Result<()> {
        match self.entry()?.delete_credential() {
            Ok(()) | Err(keyring::v1::Error::NoEntry) => Ok(()),
            Err(e) => Err(e.into()),
        }
    }
}
#[derive(Default)]
pub struct MemoryCredentials;
impl Credentials for MemoryCredentials {
    fn load(&self) -> Result<Option<Zeroizing<String>>> {
        Ok(None)
    }
    fn save(&self, _token: &str) -> Result<bool> {
        Ok(false)
    }
    fn delete(&self) -> Result<()> {
        Ok(())
    }
}
