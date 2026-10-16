// Copyright (c) 2026 Benja (Benjamacs1)
// SPDX-License-Identifier: MIT
use anyhow::Result;
use futures_util::{SinkExt, StreamExt};
use kirk_companion::{
    launch::Launch,
    model::*,
    service::{Command, Snapshot, Worker},
    storage::{Credentials, DiskStore},
};
use serde_json::{Value, json};
use std::sync::{
    Arc, RwLock,
    atomic::{AtomicUsize, Ordering},
};
use std::time::Duration;
use tokio_tungstenite::{
    accept_hdr_async,
    tungstenite::{
        Message,
        handshake::server::{Request, Response},
        protocol::{CloseFrame, frame::coding::CloseCode},
    },
};
use zeroize::Zeroizing;

struct TestCredentials {
    deletes: Arc<AtomicUsize>,
}
impl Credentials for TestCredentials {
    fn load(&self) -> Result<Option<Zeroizing<String>>> {
        Ok(Some(Zeroizing::new("A".repeat(43))))
    }
    fn save(&self, _: &str) -> Result<bool> {
        Ok(true)
    }
    fn delete(&self) -> Result<()> {
        self.deletes.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }
}
struct TestLauncher {
    calls: Arc<AtomicUsize>,
    fail: bool,
}
impl Launch for TestLauncher {
    fn launch(&self, _: &Profile, _: Option<u64>) -> Result<()> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        anyhow::ensure!(!self.fail, "Launcher failed");
        Ok(())
    }
}
async fn reply(
    ws: &mut tokio_tungstenite::WebSocketStream<tokio::net::TcpStream>,
    body: Value,
) -> Value {
    ws.send(Message::Text(body.to_string().into()))
        .await
        .unwrap();
    loop {
        if let Some(Ok(Message::Text(text))) = ws.next().await {
            let value: Value = serde_json::from_str(&text).unwrap();
            if value.get("id").is_some() {
                return value;
            }
        }
    }
}

#[tokio::test]
#[allow(clippy::result_large_err)] // tungstenite's handshake callback fixes the error type.
async fn real_socket_enforces_profile_only_launch_replay_expiry_cooldown_and_revocation() {
    let dir = tempfile::tempdir().unwrap();
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let calls = Arc::new(AtomicUsize::new(0));
    let deletes = Arc::new(AtomicUsize::new(0));
    let profile = Profile {
        name: "Main".into(),
        target: std::env::current_exe().unwrap(),
        instance: "26.2 Kirk".into(),
        ..Profile::default()
    };
    let settings = Settings {
        backend: format!("http://{}", listener.local_addr().unwrap()),
        profiles: vec![profile.clone()],
        pairing_enabled: true,
        ..Settings::default()
    };
    let shared = Arc::new(RwLock::new(Snapshot::new(settings)));
    let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
    let worker = Worker::new(
        shared.clone(),
        rx,
        DiskStore {
            dir: dir.path().into(),
        },
        Box::new(TestCredentials {
            deletes: deletes.clone(),
        }),
        Arc::new(TestLauncher {
            calls: calls.clone(),
            fail: false,
        }),
        Arc::new(|| {}),
    );
    let task = tokio::spawn(worker.run());
    let (stream, _) = tokio::time::timeout(Duration::from_secs(5), listener.accept())
        .await
        .unwrap()
        .unwrap();
    let mut ws = accept_hdr_async(stream, |request: &Request, response: Response| {
        assert_eq!(
            request.headers()["authorization"],
            format!("Bearer {}", "A".repeat(43))
        );
        Ok(response)
    })
    .await
    .unwrap();
    let Some(Ok(Message::Text(hello))) = ws.next().await else {
        panic!("No hello");
    };
    let hello: Value = serde_json::from_str(&hello).unwrap();
    assert_eq!(hello["version"], 1);
    assert_eq!(hello["profiles"][0]["name"], "Main");
    assert!(hello["profiles"][0].get("target").is_none());
    let request = json!({"id":"rust_request_1234567890","op":"launch_game","profile":profile.id,"expires_at":now_ms()+10000});
    let mut unsafe_request = request.clone();
    unsafe_request["command"] = "evil.exe".into();
    assert_eq!(reply(&mut ws, unsafe_request).await["ok"], false);
    let mut expired = request.clone();
    expired["expires_at"] = (now_ms() - 1).into();
    assert_eq!(reply(&mut ws, expired).await["ok"], false);
    let mut unknown = request.clone();
    unknown["id"] = "unknown_profile_123456789".into();
    unknown["profile"] = uuid::Uuid::new_v4().to_string().into();
    assert_eq!(reply(&mut ws, unknown).await["ok"], false);
    assert_eq!(reply(&mut ws, request.clone()).await["ok"], true);
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    assert!(
        reply(&mut ws, request.clone()).await["error"]
            .as_str()
            .unwrap()
            .contains("already handled")
    );
    let mut second = request;
    second["id"] = "second_request_1234567890".into();
    assert!(
        reply(&mut ws, second).await["error"]
            .as_str()
            .unwrap()
            .contains("30 seconds")
    );
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    ws.send(Message::Ping(vec![1, 2, 3].into())).await.unwrap();
    assert!(matches!(ws.next().await, Some(Ok(Message::Pong(_)))));
    ws.close(Some(CloseFrame {
        code: CloseCode::Library(4001),
        reason: "unpaired".into(),
    }))
    .await
    .unwrap();
    tokio::time::timeout(Duration::from_secs(3), async {
        while shared.read().unwrap().paired {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap();
    assert_eq!(deletes.load(Ordering::SeqCst), 1);
    assert!(!shared.read().unwrap().connected);
    assert!(
        !serde_json::to_string(&shared.read().unwrap().settings)
            .unwrap()
            .contains(&"A".repeat(43))
    );
    tx.send(Command::Quit).unwrap();
    task.await.unwrap();
}

#[tokio::test]
async fn disabled_saved_pairing_never_loads_a_stale_os_credential() {
    let dir = tempfile::tempdir().unwrap();
    let shared = Arc::new(RwLock::new(Snapshot::new(Settings::default())));
    let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
    let worker = Worker::new(
        shared.clone(),
        rx,
        DiskStore {
            dir: dir.path().into(),
        },
        Box::new(TestCredentials {
            deletes: Arc::new(AtomicUsize::new(0)),
        }),
        Arc::new(TestLauncher {
            calls: Arc::new(AtomicUsize::new(0)),
            fail: false,
        }),
        Arc::new(|| {}),
    );
    let task = tokio::spawn(worker.run());
    tokio::time::sleep(Duration::from_millis(30)).await;
    assert!(!shared.read().unwrap().paired);
    tx.send(Command::Quit).unwrap();
    task.await.unwrap();
}
