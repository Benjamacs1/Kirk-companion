// Copyright (c) 2026 Benja (Benjamacs1)
// SPDX-License-Identifier: MIT
#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]
mod desktop;
use anyhow::{Context, Result};
use kirk_companion::{
    launch::NativeLauncher,
    model::*,
    service::{Command, Snapshot, Worker},
    storage::{Credentials, DiskStore, MemoryCredentials, OsCredentials},
};
use std::{
    hash::{Hash, Hasher},
    path::PathBuf,
    sync::{Arc, RwLock},
};

fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().collect();
    let value = |key: &str| args.windows(2).find(|p| p[0] == key).map(|p| p[1].clone());
    let preview = args.iter().any(|a| a == "--preview");
    let headless = args.iter().any(|a| a == "--headless");
    let ephemeral = preview || args.iter().any(|a| a == "--ephemeral");
    let store = DiskStore {
        dir: value("--data-dir")
            .map(PathBuf::from)
            .unwrap_or(DiskStore::default_dir()?),
    };
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    store.dir.hash(&mut hasher);
    let _instance = if preview {
        None
    } else {
        let instance =
            single_instance::SingleInstance::new(&format!("kirk-companion-{:x}", hasher.finish()))?;
        if !instance.is_single() {
            std::fs::create_dir_all(&store.dir)?;
            std::fs::write(store.dir.join("wake"), [])?;
            return Ok(());
        }
        Some(instance)
    };
    let mut load_notice = String::new();
    let mut settings = if preview {
        Settings::default()
    } else {
        store.load().unwrap_or_else(|_| { load_notice = "Saved settings could not be read. Configure this computer again; the original will be backed up before saving.".into(); Settings::default() })
    };
    if let Some(backend) = value("--backend") {
        settings.backend = backend_origin(&backend)?;
    }
    if let Some(name) = value("--name") {
        settings.name = name;
    }
    if preview {
        settings.name = "Gaming PC".into();
        settings.profiles = vec![
            Profile {
                name: "Kirk · Skyblock".into(),
                target: std::env::current_exe()?,
                instance: "26.2(kirk)".into(),
                ..Profile::default()
            },
            Profile {
                name: "Kirk · Vanilla".into(),
                kind: Launcher::Shortcut,
                target: std::env::current_exe()?,
                ..Profile::default()
            },
        ];
    }
    settings.validate()?;
    let mut initial = Snapshot::new(settings.clone());
    initial.notice = load_notice;
    if preview {
        initial.paired = true;
        initial.connected = true;
        initial.status = "Ready for the website".into();
        initial.dashboard = kirk_companion::dashboard::preview();
    }
    let shared = Arc::new(RwLock::new(initial));
    let (sender, receiver) = tokio::sync::mpsc::unbounded_channel();
    let credentials: Box<dyn Credentials> = if ephemeral {
        Box::new(MemoryCredentials)
    } else {
        Box::new(OsCredentials {
            account: settings.machine.to_string(),
        })
    };
    let runtime = tokio::runtime::Runtime::new()?;
    if headless {
        let worker = Worker::new(
            shared.clone(),
            receiver,
            store,
            credentials,
            Arc::new(NativeLauncher),
            Arc::new(|| {}),
        );
        if let Some(code) = value("--pair") {
            sender.send(Command::Pair {
                backend: settings.backend,
                code,
                name: settings.name,
            })?;
        }
        return runtime.block_on(async move {
            tokio::select! { _ = worker.run() => {}, _ = tokio::signal::ctrl_c() => {} }
            Ok(())
        });
    }
    let screenshot = value("--screenshot").map(PathBuf::from);
    let background = args.iter().any(|a| a == "--background");
    let icon = image::load_from_memory(include_bytes!("ui/icon.png"))?.into_rgba8();
    let (width, height) = icon.dimensions();
    let options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_title("Kirk Companion")
            .with_inner_size(if args.iter().any(|a| a == "--narrow") {
                [840., 720.]
            } else {
                [1280., 860.]
            })
            .with_min_inner_size([840., 640.])
            .with_decorations(false)
            .with_icon(eframe::egui::IconData {
                rgba: icon.into_raw(),
                width,
                height,
            }),
        ..Default::default()
    };
    eframe::run_native(
        "Kirk Companion",
        options,
        Box::new(move |cc| {
            let ctx = cc.egui_ctx.clone();
            if !preview {
                let worker = Worker::new(
                    shared.clone(),
                    receiver,
                    store.clone(),
                    credentials,
                    Arc::new(NativeLauncher),
                    Arc::new(move || ctx.request_repaint()),
                );
                runtime.spawn(worker.run());
            }
            let mut app = desktop::Desktop::new(
                cc,
                shared,
                sender,
                Some(runtime),
                store,
                preview,
                screenshot,
                background,
            );
            if preview {
                app.preview_page(value("--page").as_deref().unwrap_or("dashboard"));
            }
            Ok(Box::new(app))
        }),
    )
    .map_err(|e| anyhow::anyhow!(e.to_string()))
    .context("Could not open the native companion window")
}
