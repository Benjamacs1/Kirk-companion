// Copyright (c) 2026 Benja (Benjamacs1)
// SPDX-License-Identifier: MIT
use anyhow::{Context, Result, bail};
use serde_json::{Value, json};
use std::time::Duration;
use zeroize::Zeroizing;

#[derive(Clone, Default)]
pub struct Dashboard {
    pub data: Value,
    pub live: Value,
    pub conversation: Value,
    pub screenshot: Value,
    pub refreshing: bool,
    pub acting: bool,
    pub error: String,
    pub updated_at: u64,
    pub selection: Selection,
    pub action_revision: u64,
    pub pending_actions: u32,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Selection {
    pub days: u32,
    pub license: Option<u64>,
    pub client: Option<u64>,
}
impl Default for Selection {
    fn default() -> Self {
        Self {
            days: 7,
            license: None,
            client: None,
        }
    }
}
pub enum Reply {
    Refresh(Selection, Result<(Value, Value)>),
    Action(String, Option<u64>, Result<Value>),
}
pub struct Api {
    backend: String,
    token: Zeroizing<String>,
    client: reqwest::Client,
}
impl Api {
    pub fn new(backend: String, token: Zeroizing<String>) -> Result<Self> {
        Ok(Self {
            backend,
            token,
            client: reqwest::Client::builder()
                .redirect(reqwest::redirect::Policy::none())
                .timeout(Duration::from_secs(100))
                .build()?,
        })
    }
    async fn read(&self, mut response: reqwest::Response) -> Result<Value> {
        let status = response.status();
        let mut bytes = Vec::new();
        while let Some(chunk) = response.chunk().await? {
            if bytes.len() + chunk.len() > 8 * 1024 * 1024 {
                bail!("The server response is too large.");
            }
            bytes.extend_from_slice(&chunk);
        }
        let value: Value = serde_json::from_slice(&bytes)
            .context("The server returned an unreadable response.")?;
        if !status.is_success() {
            bail!(
                match status.as_u16() {
                    401 => "Pairing was revoked. Reconnect this computer.",
                    404 => "This server needs the desktop dashboard update.",
                    429 => "Too many requests. Wait a minute and try again.",
                    _ => value["error"]
                        .as_str()
                        .unwrap_or("The server could not complete this request."),
                }
                .to_owned()
            );
        }
        Ok(value)
    }
    pub async fn command(&self, op: &str, args: Value, client: Option<u64>) -> Result<Value> {
        let mut body = json!({"op":op,"args":args});
        if let Some(client) = client {
            body["client"] = client.into();
        }
        let response = self
            .client
            .post(format!("{}/companion/command", self.backend))
            .bearer_auth(self.token.as_str())
            .json(&body)
            .send()
            .await
            .context("Could not reach Kirk.")?;
        let value = self.read(response).await?;
        if value["ok"] != true {
            bail!(
                value["error"]
                    .as_str()
                    .unwrap_or("The game could not complete this request.")
                    .replace('_', " ")
            );
        }
        Ok(value["data"].clone())
    }
    pub async fn refresh(&self, selection: Selection) -> Result<(Value, Value)> {
        let mut query = vec![("days", selection.days.to_string())];
        if let Some(license) = selection.license {
            query.push(("license", license.to_string()));
        }
        let response = self
            .client
            .get(format!("{}/companion/dashboard", self.backend))
            .bearer_auth(self.token.as_str())
            .query(&query)
            .timeout(Duration::from_secs(15))
            .send()
            .await
            .context("Could not read your dashboard.")?;
        let data = self.read(response).await?;
        let chosen = data["device"]["clients"].as_array().and_then(|items| {
            items.iter().find(|v| {
                v["online"] == true
                    && (selection.client.is_none() || v["id"].as_u64() == selection.client)
            })
        });
        let live = if let Some(chosen) = chosen {
            self.command("status", json!({}), chosen["id"].as_u64())
                .await
                .unwrap_or_else(|e| json!({"error":e.to_string()}))
        } else {
            Value::Null
        };
        Ok((data, live))
    }
}

/// Explicit sample data, used only by --preview. Never mixed with a connected account.
pub fn preview() -> Dashboard {
    let series: Vec<_> = (0..30).map(|i| json!({"day":format!("2026-10-{:02}", i+1), "seconds": ([1800,3600,7200,2400,9800,5100,6300][i%7]), "coins": (i%7+1)*1_200_000, "items_value": 300_000, "xp":24000})).collect();
    let stats = json!({"licensed":true,"licenses":[{"id":1,"tier":"Member","accounts":["KirkPlayer"]}], "totals":{"seconds":32400,"coins":24_600_000,"items_value":3_200_000,"xp":1_840_000},"today":{"seconds":7200},
        "macros":[{"macro":"Mithril","seconds":21240,"coins":18_200_000,"items_value":2_100_000,"xp":1_240_000},{"macro":"Foraging","seconds":11160,"coins":6_400_000,"items_value":1_100_000,"xp":600_000}],
        "skills":[{"skill":"Mining","xp":1_240_000},{"skill":"Foraging","xp":600_000}],
        "items":[{"item":"ENCHANTED_MITHRIL","count":128,"value":2_100_000}],"series":series});
    Dashboard {
        data: json!({"device":{"paired":true,"online":true,"clients":[{"id":1,"name":"KirkPlayer","online":true}]}, "stats":stats,"month":stats,
        "trackers":{"totals":{"powder":{"mithril":84200,"gemstone":12600,"glacite":0},"counters":{"commissions":18,"bosses":0}}, "trackers":[{"title":"Mithril","profit":20_300_000,"uptime_ms":21_240_000}],"items":[{"name":"Enchanted Mithril","amount":128,"value":2_100_000}]}}),
        live: json!({"player":"KirkPlayer","in_world":true,"skyblock":true,"area":"Dwarven Mines","zone":"Royal Mines","purse":"12,480,320","health":100,"macros":["Auto Miner"],"chat":["[Kirk] Mithril route started.","+120 Mining XP","[Kirk] Commission complete."],"profit":{"profit_per_hour":3_440_000}}),
        conversation: json!({"lines":[{"kirk":true,"text":"Hey! Your game is connected. What would you like to do?"}]}),
        updated_at: crate::model::now_ms(),
        ..Default::default()
    }
}
