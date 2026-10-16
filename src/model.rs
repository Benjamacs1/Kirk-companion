// Copyright (c) 2026 Benja (Benjamacs1)
// SPDX-License-Identifier: MIT
use anyhow::{Result, bail};
use serde::{Deserialize, Serialize};
use std::{
    path::PathBuf,
    time::{SystemTime, UNIX_EPOCH},
};
use url::Url;
use uuid::Uuid;

pub const DEFAULT_BACKEND: &str = "https://api.charlie-kirk.uk";
pub fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Layout {
    #[default]
    Slate,
    Canvas,
    Studio,
}
impl Layout {
    pub const ALL: [Self; 3] = [Self::Slate, Self::Canvas, Self::Studio];
    pub fn name(self) -> &'static str {
        match self {
            Self::Slate => "Slate",
            Self::Canvas => "Canvas",
            Self::Studio => "Studio",
        }
    }
}
#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Theme {
    #[default]
    Ghost,
    Kirk,
}
#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Launcher {
    #[default]
    Prism,
    Multimc,
    Shortcut,
    Open,
}
impl Launcher {
    pub const ALL: [Self; 4] = [Self::Prism, Self::Multimc, Self::Shortcut, Self::Open];
    pub fn name(self) -> &'static str {
        match self {
            Self::Prism => "Prism Launcher",
            Self::Multimc => "MultiMC",
            Self::Shortcut => "Instance shortcut",
            Self::Open => "Open launcher only",
        }
    }
    pub fn direct(self) -> bool {
        matches!(self, Self::Prism | Self::Multimc)
    }
    pub fn capability(self) -> &'static str {
        if self == Self::Open { "open" } else { "launch" }
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Profile {
    pub id: Uuid,
    pub name: String,
    pub kind: Launcher,
    pub target: PathBuf,
    #[serde(default)]
    pub instance: String,
    #[serde(default)]
    pub root: Option<PathBuf>,
}
impl Default for Profile {
    fn default() -> Self {
        Self {
            id: Uuid::new_v4(),
            name: String::new(),
            kind: Launcher::Prism,
            target: PathBuf::new(),
            instance: String::new(),
            root: None,
        }
    }
}
impl Profile {
    pub fn validate(&self) -> Result<()> {
        if self.name.trim().is_empty() || self.name.chars().count() > 80 {
            bail!("Give this profile a name, up to 80 characters.");
        }
        let target = self.target.to_string_lossy();
        if !self.target.is_absolute() || target.len() > 4096 || target.chars().any(char::is_control)
        {
            bail!("Choose a local launcher or shortcut.");
        }
        if self.kind.direct() {
            if self.instance.is_empty()
                || self.instance.chars().count() > 200
                || self.instance.starts_with('-')
                || self.instance == "."
                || self.instance == ".."
                || self
                    .instance
                    .chars()
                    .any(|c| c.is_control() || c == '/' || c == '\\')
            {
                bail!("Enter the instance folder name without a path or command-line flags.");
            }
            if let Some(root) = &self.root
                && (!root.is_absolute()
                    || root.to_string_lossy().len() > 4096
                    || root.to_string_lossy().chars().any(char::is_control))
            {
                bail!("Choose an absolute launcher data folder.");
            }
            if cfg!(target_os = "macos") && self.target.extension().is_some_and(|e| e == "app") {
                bail!("For direct launch, choose the executable inside the app bundle.");
            }
        }
        Ok(())
    }
    pub fn public(&self) -> PublicProfile {
        PublicProfile {
            id: self.id,
            name: self.name.clone(),
            launcher: self.kind,
            capability: self.kind.capability().into(),
        }
    }
}
#[derive(Clone, Debug, Serialize)]
pub struct PublicProfile {
    pub id: Uuid,
    pub name: String,
    pub launcher: Launcher,
    pub capability: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub machine: Uuid,
    pub backend: String,
    pub name: String,
    pub profiles: Vec<Profile>,
    pub layout: Layout,
    pub theme: Theme,
    pub pairing_enabled: bool,
}
impl Default for Settings {
    fn default() -> Self {
        Self {
            machine: Uuid::new_v4(),
            backend: DEFAULT_BACKEND.into(),
            name: hostname::get()
                .ok()
                .and_then(|v| v.into_string().ok())
                .unwrap_or("Gaming PC".into()),
            profiles: vec![],
            layout: Layout::Slate,
            theme: Theme::Ghost,
            pairing_enabled: false,
        }
    }
}
impl Settings {
    pub fn validate(&self) -> Result<()> {
        backend_origin(&self.backend)?;
        if self.name.trim().is_empty() || self.name.chars().count() > 64 || self.profiles.len() > 32
        {
            bail!("Invalid saved computer settings.");
        }
        let mut ids = std::collections::HashSet::new();
        for p in &self.profiles {
            p.validate()?;
            if !ids.insert(p.id) {
                bail!("Duplicate saved launch profile.");
            }
        }
        Ok(())
    }
}

pub fn backend_origin(value: &str) -> Result<String> {
    let url = Url::parse(value).map_err(|_| anyhow::anyhow!("Enter a valid server URL."))?;
    let loopback = matches!(
        url.host_str(),
        Some("localhost" | "127.0.0.1" | "[::1]" | "::1")
    );
    if !(url.scheme() == "https" || (url.scheme() == "http" && loopback))
        || !url.username().is_empty()
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
        || url.path() != "/"
    {
        bail!("Use an HTTPS server origin, or HTTP on localhost for development.");
    }
    Ok(url.origin().ascii_serialization())
}
pub fn pairing_code(value: &str) -> Result<String> {
    let value: String = value
        .chars()
        .filter(|c| !c.is_whitespace() && *c != '-')
        .flat_map(char::to_uppercase)
        .collect();
    if value.len() != 8
        || !value
            .bytes()
            .all(|b| b.is_ascii_uppercase() || b.is_ascii_digit())
    {
        bail!("Enter the eight-character code from the dashboard.");
    }
    Ok(value)
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LaunchRequest {
    pub id: String,
    pub op: String,
    pub profile: Uuid,
    pub expires_at: u64,
}
impl LaunchRequest {
    pub fn validate(&self, now: u64) -> Result<()> {
        if !(16..=64).contains(&self.id.len())
            || !self
                .id
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
            || self.op != "launch_game"
        {
            bail!("Invalid launch request.");
        }
        if self.expires_at <= now || self.expires_at > now.saturating_add(30_000) {
            bail!("Launch request expired. Try again.");
        }
        Ok(())
    }
}
