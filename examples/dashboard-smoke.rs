// Copyright (c) 2026 Benja (Benjamacs1)
// SPDX-License-Identifier: MIT
//! Integration harness: run only against the throwaway backend in tools/companion-rust-e2e.ts.
use anyhow::{Context, Result, ensure};
use kirk_companion::{
    dashboard::Selection,
    launch::NativeLauncher,
    service::{Command, Shared, Snapshot, Worker},
    storage::{DiskStore, MemoryCredentials},
};
use serde_json::json;
use std::{
    sync::{Arc, RwLock},
    time::Duration,
};

async fn until(shared: &Shared, condition: impl Fn(&Snapshot) -> bool) -> Result<Snapshot> {
    tokio::time::timeout(Duration::from_secs(15), async {
        loop {
            let snapshot = shared.read().unwrap().clone();
            if condition(&snapshot) {
                return snapshot;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await
    .context("Native worker did not reach the expected state")
}
async fn action(
    shared: &Shared,
    sender: &tokio::sync::mpsc::UnboundedSender<Command>,
    op: &str,
    args: serde_json::Value,
    client: u64,
) -> Result<Snapshot> {
    let before = shared.read().unwrap().dashboard.action_revision;
    sender.send(Command::Remote {
        op: op.into(),
        args,
        client: Some(client),
    })?;
    let result = until(shared, |s| s.dashboard.action_revision > before).await?;
    ensure!(
        result.dashboard.error.is_empty(),
        "{}",
        result.dashboard.error
    );
    Ok(result)
}
#[tokio::main]
async fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    ensure!(
        args.len() == 5,
        "Expected backend, pairing code, data directory, client id, report path"
    );
    let store = DiskStore {
        dir: args[2].clone().into(),
    };
    let mut settings = store.load()?;
    settings.backend = args[0].clone();
    let shared = Arc::new(RwLock::new(Snapshot::new(settings.clone())));
    let (sender, receiver) = tokio::sync::mpsc::unbounded_channel();
    let worker = Worker::new(
        shared.clone(),
        receiver,
        store,
        Box::new(MemoryCredentials),
        Arc::new(NativeLauncher),
        Arc::new(|| {}),
    );
    let task = tokio::spawn(worker.run());
    sender.send(Command::Pair {
        backend: args[0].clone(),
        code: args[1].clone(),
        name: "Native test PC".into(),
    })?;
    until(&shared, |s| s.connected).await?;
    let client = args[3].parse()?;
    sender.send(Command::Refresh(Selection {
        client: Some(client),
        ..Default::default()
    }))?;
    let data = until(&shared, |s| s.dashboard.updated_at > 0).await?;
    ensure!(data.dashboard.live["player"] == "Test player");
    ensure!(data.dashboard.data["stats"]["totals"]["coins"] == 123456);
    action(
        &shared,
        &sender,
        "start",
        json!({"macro":"mithril"}),
        client,
    )
    .await?;
    let conversation = action(&shared, &sender, "conversation", json!({}), client).await?;
    ensure!(conversation.dashboard.conversation["lines"][0]["text"] == "Hello from the game");
    let shot = action(&shared, &sender, "screenshot", json!({}), client).await?;
    ensure!(shot.dashboard.screenshot["mime"] == "image/png");
    action(
        &shared,
        &sender,
        "ask",
        json!({"text":"How is my game?"}),
        client,
    )
    .await?;
    action(&shared, &sender, "stop", json!({"macro":"all"}), client).await?;
    std::fs::write(&args[4], "dashboard passed")?;
    // The website revokes pairing after seeing the report.
    let revoked = until(&shared, |s| !s.paired).await?;
    ensure!(revoked.dashboard.data.is_null() && revoked.dashboard.screenshot.is_null());
    sender.send(Command::Quit)?;
    task.await?;
    println!("Native dashboard, game commands, screenshot, chat, and revocation passed.");
    Ok(())
}
