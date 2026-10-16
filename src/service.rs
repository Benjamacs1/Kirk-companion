// Copyright (c) 2026 Benja (Benjamacs1)
// SPDX-License-Identifier: MIT
use crate::{
    dashboard::{Api, Dashboard, Reply, Selection},
    launch::Launch,
    model::*,
    storage::{Credentials, DiskStore},
};
use anyhow::{Context, Result, bail};
use futures_util::{SinkExt, StreamExt};
use serde_json::{Value, json};
use std::{
    collections::HashMap,
    sync::{Arc, RwLock},
    time::Duration,
};
use tokio::{
    sync::mpsc,
    time::{Instant, sleep_until},
};
use tokio_tungstenite::{
    connect_async_with_config,
    tungstenite::{
        self, Message,
        client::IntoClientRequest,
        protocol::{WebSocketConfig, frame::coding::CloseCode},
    },
};
use uuid::Uuid;
use zeroize::Zeroizing;

#[derive(Clone)]
pub struct Snapshot {
    pub settings: Settings,
    pub paired: bool,
    pub connected: bool,
    pub working: bool,
    pub status: String,
    pub notice: String,
    pub ephemeral: bool,
    pub dashboard: Dashboard,
}
impl Snapshot {
    pub fn new(settings: Settings) -> Self {
        Self {
            settings,
            paired: false,
            connected: false,
            working: false,
            status: "Not paired".into(),
            notice: String::new(),
            ephemeral: false,
            dashboard: Dashboard::default(),
        }
    }
}
#[derive(Clone, Debug)]
pub enum Command {
    Pair {
        backend: String,
        code: String,
        name: String,
    },
    Unpair,
    Add(Profile),
    Remove(Uuid),
    Test(Uuid),
    Refresh(Selection),
    Remote {
        op: String,
        args: Value,
        client: Option<u64>,
    },
    Appearance {
        layout: Layout,
        theme: Theme,
    },
    Quit,
}
pub type Shared = Arc<RwLock<Snapshot>>;
pub struct Worker {
    pub snapshot: Shared,
    pub receiver: mpsc::UnboundedReceiver<Command>,
    pub store: DiskStore,
    pub credentials: Box<dyn Credentials>,
    pub launcher: Arc<dyn Launch>,
    pub wake: Arc<dyn Fn() + Send + Sync>,
    token: Option<Zeroizing<String>>,
    seen: HashMap<String, u64>,
    cooldown: HashMap<Uuid, u64>,
    generation: u64,
    jobs: tokio::task::JoinSet<(u64, Reply)>,
}
impl Worker {
    pub fn new(
        snapshot: Shared,
        receiver: mpsc::UnboundedReceiver<Command>,
        store: DiskStore,
        credentials: Box<dyn Credentials>,
        launcher: Arc<dyn Launch>,
        wake: Arc<dyn Fn() + Send + Sync>,
    ) -> Self {
        Self {
            snapshot,
            receiver,
            store,
            credentials,
            launcher,
            wake,
            token: None,
            seen: HashMap::new(),
            cooldown: HashMap::new(),
            generation: 0,
            jobs: tokio::task::JoinSet::new(),
        }
    }
    pub fn read(&self) -> Snapshot {
        self.snapshot
            .read()
            .unwrap_or_else(|e| e.into_inner())
            .clone()
    }
    fn update(&self, change: impl FnOnce(&mut Snapshot)) {
        change(&mut self.snapshot.write().unwrap_or_else(|e| e.into_inner()));
        (self.wake)();
    }
    fn hello(&self) -> Message {
        Message::Text(json!({"type":"hello", "version":1, "profiles":self.read().settings.profiles.iter().map(Profile::public).collect::<Vec<_>>()}).to_string().into())
    }
    fn persist(&self, settings: Settings) -> Result<()> {
        self.store.save(&settings)?;
        self.update(|s| s.settings = settings);
        Ok(())
    }
    async fn pair(&mut self, backend: String, code: String, name: String) -> Result<()> {
        if self.token.is_some() {
            bail!("Unpair this computer before pairing it again.");
        }
        let backend = backend_origin(&backend)?;
        let code = pairing_code(&code)?;
        let name = name.trim().to_owned();
        if name.is_empty() || name.chars().count() > 64 {
            bail!("Give this computer a name, up to 64 characters.");
        }
        let mut settings = self.read().settings;
        let client = reqwest::Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .timeout(Duration::from_secs(15))
            .build()?;
        let response = client
            .post(format!("{backend}/companion/claim"))
            .json(&json!({"code":code,"machine":settings.machine,"name":name}))
            .send()
            .await
            .context("Could not reach the server. Check your connection and server URL.")?;
        let status = response.status();
        let body: Value = response.json().await.unwrap_or(Value::Null);
        if !status.is_success() {
            bail!(match body["error"].as_str() {
                Some("invalid_or_expired_code") =>
                    "That code expired or was already used. Get a new code.",
                Some("too_many_computers") =>
                    "Remove an old computer from the dashboard before pairing another.",
                _ if status.as_u16() == 404 => "This server needs the companion backend update.",
                _ => "Pairing failed. Check the server and try again.",
            });
        }
        let token = Zeroizing::new(body["token"].as_str().unwrap_or_default().to_owned());
        if token.len() != 43
            || !token
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
        {
            bail!("The server returned an invalid pairing credential.");
        }
        settings.backend = backend.clone();
        settings.name = name;
        let persistent = self.credentials.save(token.as_str()).unwrap_or(false);
        settings.pairing_enabled = persistent;
        if let Err(error) = self.persist(settings) {
            let _ = self.credentials.delete();
            let _ = client
                .post(format!("{backend}/companion/revoke"))
                .bearer_auth(token.as_str())
                .send()
                .await;
            return Err(error);
        }
        self.token = Some(token);
        self.generation += 1;
        self.update(|s| {
            s.paired = true;
            s.ephemeral = !persistent;
            s.status = "Connecting…".into();
            s.notice = "Computer paired. Keep the companion running for remote launch.".into();
        });
        Ok(())
    }
    async fn unpair(&mut self) -> Result<()> {
        self.generation += 1;
        self.jobs.abort_all();
        let token = self.token.take();
        let mut settings = self.read().settings;
        settings.pairing_enabled = false;
        let disk_result = self.persist(settings);
        let deletion = self.credentials.delete();
        self.update(|s| {
            s.paired = false;
            s.connected = false;
            s.ephemeral = false;
            s.status = "Not paired".into();
            s.notice = "Computer unpaired.".into();
            s.dashboard = Dashboard::default();
        });
        if let Some(token) = token {
            let client = reqwest::Client::builder()
                .redirect(reqwest::redirect::Policy::none())
                .timeout(Duration::from_secs(5))
                .build()?;
            let response = client
                .post(format!("{}/companion/revoke", self.read().settings.backend))
                .bearer_auth(token.as_str())
                .send()
                .await;
            if !response.is_ok_and(|r| r.status().is_success() || r.status().as_u16() == 401) {
                bail!(
                    "Removed locally. The server could not be reached; remove this computer on the dashboard too."
                );
            }
        }
        deletion.context(
            "Could not remove the saved OS credential. Remove the computer on the dashboard too.",
        )?;
        disk_result?;
        Ok(())
    }
    fn revoked(&mut self) {
        self.token = None;
        self.generation += 1;
        self.jobs.abort_all();
        self.update(|s| s.dashboard = Dashboard::default());
        let result = self.credentials.delete();
        let mut settings = self.read().settings;
        settings.pairing_enabled = false;
        let _ = self.persist(settings);
        self.update(|s| { s.paired = false; s.connected = false; s.ephemeral = false; s.status = "Pairing removed".into(); s.notice = if result.is_ok() { "This computer was unpaired. Pair it again to reconnect.".into() } else { "Pairing revoked. The OS credential could not be deleted; remove it from your system credential manager.".into() }; });
    }
    async fn launch(&mut self, id: Uuid, deadline: Option<u64>) -> Result<Value> {
        let profile = self
            .read()
            .settings
            .profiles
            .into_iter()
            .find(|p| p.id == id)
            .context("This profile was removed. Refresh the dashboard.")?;
        if self
            .cooldown
            .get(&id)
            .is_some_and(|time| now_ms().saturating_sub(*time) < 30_000)
        {
            bail!("A launch was just requested. Wait 30 seconds before trying again.");
        }
        let launcher = self.launcher.clone();
        let copy = profile.clone();
        let result = tokio::task::spawn_blocking(move || launcher.launch(&copy, deadline));
        tokio::time::timeout(Duration::from_secs(8), result)
            .await
            .context(
                "Launcher did not answer in time. Check this computer before trying again.",
            )???;
        self.cooldown.insert(id, now_ms());
        self.update(|s| {
            s.notice = if profile.kind == Launcher::Open {
                "Launcher opened. Choose your instance and press Play.".into()
            } else {
                format!(
                    "Launch sent for {}. Your launcher may ask you to sign in.",
                    profile.name
                )
            }
        });
        Ok(json!({"status":"dispatched","profile":id,"capability":profile.kind.capability()}))
    }
    async fn command(&mut self, cmd: Command) -> Result<bool> {
        match cmd {
            Command::Pair {
                backend,
                code,
                name,
            } => self.pair(backend, code, name).await?,
            Command::Unpair => self.unpair().await?,
            Command::Add(mut p) => {
                p.name = p.name.trim().into();
                p.instance = p.instance.trim().into();
                p.validate()?;
                let mut settings = self.read().settings;
                if settings.profiles.len() >= 32 {
                    bail!("You can save up to 32 profiles.");
                }
                settings.profiles.push(p);
                self.persist(settings)?;
                self.update(|s| s.notice = "Profile saved. Use Test launch to check it.".into());
            }
            Command::Remove(id) => {
                let mut settings = self.read().settings;
                settings.profiles.retain(|p| p.id != id);
                self.persist(settings)?;
            }
            Command::Test(id) => {
                self.launch(id, Some(now_ms() + 8_000)).await?;
            }
            Command::Refresh(selection) => {
                if ![1, 7, 30].contains(&selection.days) {
                    bail!("Choose Today, 7 days, or 30 days.");
                }
                if self.read().dashboard.refreshing {
                    return Ok(true);
                }
                let api = Api::new(
                    self.read().settings.backend,
                    self.token.clone().context("Connect this computer first.")?,
                )?;
                self.update(|s| {
                    s.dashboard.refreshing = true;
                    if s.dashboard.selection.client != selection.client {
                        s.dashboard.live = Value::Null;
                        s.dashboard.conversation = Value::Null;
                        s.dashboard.screenshot = Value::Null;
                    }
                    s.dashboard.selection = selection;
                });
                let generation = self.generation;
                self.jobs.spawn(async move {
                    (
                        generation,
                        Reply::Refresh(selection, api.refresh(selection).await),
                    )
                });
            }
            Command::Remote { op, args, client } => {
                let pending = self.read().dashboard.pending_actions;
                if pending > 0 && (op != "stop" || args["macro"] != "all" || pending >= 2) {
                    bail!("Wait for the current game action to finish.");
                }
                let api = Api::new(
                    self.read().settings.backend,
                    self.token.clone().context("Connect this computer first.")?,
                )?;
                self.update(|s| {
                    s.dashboard.acting = true;
                    s.dashboard.pending_actions += 1;
                    s.dashboard.error.clear();
                    if op == "ask" {
                        if !s.dashboard.conversation.is_object() {
                            s.dashboard.conversation = json!({"lines":[]});
                        }
                        let lines = &mut s.dashboard.conversation["lines"];
                        if !lines.is_array() {
                            *lines = json!([]);
                        }
                        lines
                            .as_array_mut()
                            .unwrap()
                            .push(json!({"kirk":false,"text":args["text"]}));
                    }
                });
                let generation = self.generation;
                self.jobs.spawn(async move {
                    let result = api.command(&op, args, client).await;
                    (generation, Reply::Action(op, client, result))
                });
            }
            Command::Appearance { layout, theme } => {
                let mut settings = self.read().settings;
                settings.layout = layout;
                settings.theme = theme;
                self.persist(settings)?;
            }
            Command::Quit => return Ok(false),
        }
        Ok(true)
    }
    async fn request(&mut self, text: &str) -> Option<Message> {
        let value: Value = serde_json::from_str(text).ok()?;
        let id = value.get("id")?.as_str()?.to_owned();
        if !(16..=64).contains(&id.len())
            || !id
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
        {
            return None;
        }
        let result = async {
            let req: LaunchRequest =
                serde_json::from_value(value).context("Invalid launch request.")?;
            req.validate(now_ms())?;
            self.seen.retain(|_, expiry| *expiry > now_ms());
            if self.seen.contains_key(&req.id) {
                bail!("This request was already handled.");
            }
            if self.seen.len() >= 512 {
                bail!("Too many requests. Try again later.");
            }
            self.seen.insert(req.id, req.expires_at);
            self.launch(req.profile, Some(req.expires_at)).await
        }
        .await;
        Some(Message::Text(
            match result {
                Ok(data) => json!({"id":id,"ok":true,"data":data}),
                Err(error) => json!({"id":id,"ok":false,"error":error.to_string()}),
            }
            .to_string()
            .into(),
        ))
    }
    pub async fn run(mut self) {
        match if self.read().settings.pairing_enabled {
            self.credentials.load()
        } else {
            Ok(None)
        } {
            Ok(token) => {
                self.token = token.filter(|t| {
                    t.len() == 43
                        && t.bytes()
                            .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
                });
                self.update(|s| s.paired = self.token.is_some());
            }
            Err(_) => self.update(|s| {
                s.notice = "Saved pairing could not be unlocked. Pair this computer again.".into()
            }),
        }
        let mut socket: Option<
            tokio_tungstenite::WebSocketStream<
                tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
            >,
        > = None;
        let mut retry = Instant::now();
        let mut backoff = 2u64;
        let mut last_message = Instant::now();
        let mut heartbeat = tokio::time::interval(Duration::from_secs(5));
        loop {
            let reconnect = socket.is_none() && self.token.is_some();
            tokio::select! {
                result = self.jobs.join_next(), if !self.jobs.is_empty() => {
                    if let Some(Ok((generation, reply))) = result && generation == self.generation {
                        self.update(|s| match reply {
                            Reply::Refresh(selection, result) => { s.dashboard.refreshing = false; match result {
                                Ok((data, live)) => { s.dashboard.data = data; s.dashboard.live = live; s.dashboard.selection = selection; s.dashboard.updated_at = now_ms(); s.dashboard.error.clear(); },
                                Err(e) => s.dashboard.error = e.to_string(),
                            } },
                            Reply::Action(op, client, result) => { s.dashboard.pending_actions = s.dashboard.pending_actions.saturating_sub(1); s.dashboard.acting = s.dashboard.pending_actions > 0; s.dashboard.action_revision += 1; match result {
                                Ok(data) => {
                                    if client == s.dashboard.selection.client {
                                        match op.as_str() { "screenshot" => s.dashboard.screenshot = data, "conversation" => s.dashboard.conversation = data,
                                            "ask" => { let lines = &mut s.dashboard.conversation["lines"]; if !lines.is_array() { *lines = json!([]); } lines.as_array_mut().unwrap().push(json!({"kirk":true,"text":data["reply"].as_str().unwrap_or("Done.")})); },
                                            "status" => s.dashboard.live = data, _ => { s.dashboard.live = Value::Null; s.notice = "Game action completed. Refreshing status…".into(); } }
                                    }
                                }, Err(e) => s.dashboard.error = e.to_string(),
                            } },
                        });
                    }
                }
                cmd = self.receiver.recv() => {
                    let Some(cmd) = cmd else { break; }; let generation = self.generation;
                    self.update(|s| s.working = true);
                    let result = self.command(cmd).await;
                    self.update(|s| s.working = false);
                    match result { Ok(false) => break, Err(e) => self.update(|s| s.notice = e.to_string()), _ => {} }
                    if generation != self.generation { if let Some(mut old) = socket.take() { let _ = old.close(None).await; } retry = Instant::now(); }
                    else if let Some(ws) = socket.as_mut() && ws.send(self.hello()).await.is_err() { socket = None; self.update(|s| s.connected = false); }
                }
                _ = sleep_until(retry), if reconnect => {
                    self.update(|s| s.status = "Connecting…".into());
                    let address = self.read().settings.backend.replacen("http", "ws", 1) + "/companion/ws";
                    let mut request = match address.into_client_request() { Ok(r) => r, Err(_) => { self.revoked(); continue; } };
                    request.headers_mut().insert("authorization", format!("Bearer {}", self.token.as_ref().unwrap().as_str()).parse().unwrap());
                    let config = WebSocketConfig::default().max_message_size(Some(16_384)).max_frame_size(Some(16_384));
                    let result = tokio::time::timeout(Duration::from_secs(10), connect_async_with_config(request, Some(config), false)).await;
                    match result {
                        Ok(Ok((mut ws, _))) => { if ws.send(self.hello()).await.is_ok() { socket = Some(ws); backoff = 2; last_message = Instant::now(); self.update(|s| { s.connected = true; s.status = "Ready for the website".into(); }); } else { retry = Instant::now() + Duration::from_secs(backoff); backoff = (backoff * 2).min(60); } },
                        Ok(Err(tungstenite::Error::Http(response))) if response.status().as_u16() == 401 => self.revoked(),
                        _ => { self.update(|s| { s.connected = false; s.status = "Server unavailable. Reconnecting…".into(); }); retry = Instant::now() + Duration::from_secs(backoff); backoff = (backoff * 2).min(60); },
                    }
                }
                message = async { match socket.as_mut() { Some(ws) => ws.next().await, None => std::future::pending().await } } => {
                    last_message = Instant::now();
                    match message {
                        Some(Ok(Message::Text(text))) => { if let Some(reply) = self.request(&text).await && let Some(ws) = socket.as_mut() { let _ = ws.send(reply).await; } },
                        Some(Ok(Message::Ping(data))) => { if let Some(ws) = socket.as_mut() { let _ = ws.send(Message::Pong(data)).await; } },
                        Some(Ok(Message::Close(frame))) => { if frame.is_some_and(|f| f.code == CloseCode::Library(4001)) { self.revoked(); } else { self.update(|s| { s.connected = false; s.status = "Connection lost. Reconnecting…".into(); }); } socket = None; retry = Instant::now() + Duration::from_secs(2); },
                        Some(Ok(_)) => {},
                        _ => { socket = None; self.update(|s| { s.connected = false; s.status = if s.paired { "Connection lost. Reconnecting…" } else { "Not paired" }.into(); }); retry = Instant::now() + Duration::from_secs(2); },
                    }
                }
                _ = heartbeat.tick() => {
                    if socket.is_some() && last_message.elapsed() > Duration::from_secs(75) { socket = None; self.update(|s| { s.connected = false; s.status = "Connection lost. Reconnecting…".into(); }); retry = Instant::now(); }
                }
            }
        }
        if let Some(mut ws) = socket {
            let _ = ws.close(None).await;
        }
    }
}
