// Copyright (c) 2026 Benja (Benjamacs1)
// SPDX-License-Identifier: MIT
use crate::model::{Launcher, Profile, now_ms};
use anyhow::{Context, Result, bail};
use std::{
    ffi::OsString,
    path::{Path, PathBuf},
    process::{Command, Stdio},
};

pub trait Launch: Send + Sync {
    fn launch(&self, profile: &Profile, deadline: Option<u64>) -> Result<()>;
}
pub struct NativeLauncher;
pub fn arguments(profile: &Profile) -> Vec<OsString> {
    let mut args = vec![];
    if let Some(root) = &profile.root {
        args.extend([OsString::from("--dir"), root.as_os_str().into()]);
    }
    args.extend([OsString::from("--launch"), profile.instance.clone().into()]);
    args
}
impl Launch for NativeLauncher {
    fn launch(&self, profile: &Profile, deadline: Option<u64>) -> Result<()> {
        profile.validate()?;
        std::fs::metadata(&profile.target)
            .context("The launcher was moved or removed. Choose it again.")?;
        if deadline.is_some_and(|end| now_ms() >= end) {
            bail!("Launch request expired. Try again.");
        }
        if profile.kind.direct() {
            let mut command = Command::new(&profile.target);
            command
                .args(arguments(profile))
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::null());
            #[cfg(windows)]
            {
                use std::os::windows::process::CommandExt;
                command.creation_flags(0x0800_0200);
            }
            command
                .spawn()
                .context("Could not start the selected launcher.")?;
            Ok(())
        } else {
            open_path(&profile.target)
        }
    }
}
pub fn open_path(path: &Path) -> Result<()> {
    #[cfg(windows)]
    {
        use std::os::windows::ffi::OsStrExt;
        let wide: Vec<u16> = path.as_os_str().encode_wide().chain(Some(0)).collect();
        let verb: Vec<u16> = "open\0".encode_utf16().collect();
        // Only local, explicitly selected profiles reach this native call. No shell command is built.
        let result = unsafe {
            windows_sys::Win32::UI::Shell::ShellExecuteW(
                std::ptr::null_mut(),
                verb.as_ptr(),
                wide.as_ptr(),
                std::ptr::null(),
                std::ptr::null(),
                1,
            )
        };
        if result as isize <= 32 {
            bail!("Could not open the selected launcher or shortcut.");
        }
        Ok(())
    }
    #[cfg(not(windows))]
    {
        let mut command = Command::new(if cfg!(target_os = "macos") {
            "/usr/bin/open"
        } else {
            "xdg-open"
        });
        if cfg!(target_os = "macos") {
            command.arg("--");
        }
        command
            .arg(path)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .context("Could not open the selected launcher or shortcut.")?;
        Ok(())
    }
}
pub fn open_dashboard() -> Result<()> {
    open_path(Path::new("https://charlie-kirk.uk/app"))
}

#[derive(Clone, Debug)]
pub struct Discovered {
    pub name: String,
    pub kind: Launcher,
    pub instance: String,
    pub root: PathBuf,
}
pub fn discover() -> Vec<Discovered> {
    let Some(home) = directories::BaseDirs::new() else {
        return vec![];
    };
    let mut roots = vec![];
    if cfg!(target_os = "windows") {
        roots.push((Launcher::Prism, home.config_dir().join("PrismLauncher")));
        roots.push((Launcher::Multimc, home.config_dir().join("MultiMC")));
    } else if cfg!(target_os = "macos") {
        roots.push((Launcher::Prism, home.data_dir().join("PrismLauncher")));
        roots.push((Launcher::Multimc, home.data_dir().join("MultiMC")));
    } else {
        roots.push((Launcher::Prism, home.data_dir().join("PrismLauncher")));
        roots.push((Launcher::Multimc, home.data_dir().join("multimc")));
        roots.push((
            Launcher::Prism,
            home.home_dir()
                .join(".var/app/org.prismlauncher.PrismLauncher/data/PrismLauncher"),
        ));
    }
    discover_at(&roots)
}
pub fn discover_at(roots: &[(Launcher, PathBuf)]) -> Vec<Discovered> {
    let mut found = vec![];
    for (kind, root) in roots {
        let Ok(entries) = std::fs::read_dir(root.join("instances")) else {
            continue;
        };
        for entry in entries.flatten().take(100) {
            if !entry.file_type().is_ok_and(|t| t.is_dir()) {
                continue;
            }
            let file = entry.path().join("instance.cfg");
            if !std::fs::metadata(&file).is_ok_and(|s| s.len() < 65_536) {
                continue;
            }
            let Ok(text) = std::fs::read_to_string(file) else {
                continue;
            };
            let instance = entry.file_name().to_string_lossy().into_owned();
            let name = text
                .lines()
                .find_map(|line| line.strip_prefix("name="))
                .unwrap_or(&instance)
                .trim()
                .chars()
                .take(80)
                .collect();
            found.push(Discovered {
                name,
                kind: *kind,
                instance,
                root: root.clone(),
            });
        }
    }
    found
}
