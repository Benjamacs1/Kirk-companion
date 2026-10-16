// Copyright (c) 2026 Benja (Benjamacs1)
// SPDX-License-Identifier: MIT
use serde_json::{Value, json};

fn rows(value: &Value) -> &[Value] {
    value.as_array().map(Vec::as_slice).unwrap_or(&[])
}
fn word(value: &Value) -> &str {
    value.as_str().unwrap_or("—")
}
fn number(value: &Value) -> f64 {
    value.as_f64().filter(|n| n.is_finite()).unwrap_or(0.)
}
fn amount(n: f64) -> String {
    if n.abs() >= 1e9 {
        format!("{:.2}B", n / 1e9)
    } else if n.abs() >= 1e6 {
        format!("{:.2}M", n / 1e6)
    } else if n.abs() >= 1e3 {
        format!("{:.1}k", n / 1e3)
    } else {
        format!("{n:.0}")
    }
}
fn elapsed(seconds: f64) -> String {
    format!(
        "{}h {}m",
        seconds.max(0.) as u64 / 3600,
        seconds.max(0.) as u64 % 3600 / 60
    )
}
const MACROS: [(&str, &str, &str); 9] = [
    ("mithril", "Mithril", "Dwarven miner"),
    ("commission", "Commission", "Mine commissions"),
    ("fishing", "Fishing", "Auto fisher"),
    ("foraging", "Foraging", "Foraging route"),
    ("worm", "Worm Setup", "Lava worm setup"),
    ("revenant", "Revenant", "Zombie slayer"),
    ("tarantula", "Tarantula", "Spider slayer"),
    ("sven", "Sven", "Wolf slayer"),
    ("hunting", "Chill Shards", "Shard hunter"),
];

impl Desktop {
    pub fn preview_page(&mut self, name: &str) {
        self.page = match name {
            "macros" => Page::Macros,
            "statistics" => Page::Statistics,
            "trackers" => Page::Trackers,
            "screen" => Page::Screen,
            "assistant" => Page::Assistant,
            "connection" => Page::Connection,
            "profiles" | "editor" => Page::Profiles,
            "preferences" => Page::Preferences,
            _ => Page::Dashboard,
        };
        if name == "connection" {
            let mut s = self.shared.write().unwrap_or_else(|e| e.into_inner());
            s.paired = false;
            s.connected = false;
            s.status = "Not paired".into();
        }
        if name == "editor" {
            self.editor = Some(Profile::default());
        }
    }
    fn refresh_dashboard(&mut self) {
        if self.preview {
            return;
        }
        self.send(Command::Refresh(self.selection));
        self.next_refresh = std::time::Instant::now() + Duration::from_secs(15);
    }
    fn remote(&mut self, op: &str, args: Value) {
        self.send(Command::Remote {
            op: op.into(),
            args,
            client: self.selection.client,
        });
        self.next_refresh = std::time::Instant::now() + Duration::from_secs(2);
    }
    fn online(&self, s: &kirk_companion::service::Snapshot) -> bool {
        s.paired
            && rows(&s.dashboard.data["device"]["clients"])
                .iter()
                .any(|c| {
                    c["online"] == true
                        && (self.selection.client.is_none()
                            || c["id"].as_u64() == self.selection.client)
                })
    }
    fn dashboard_toolbar(&mut self, ui: &mut egui::Ui, s: &kirk_companion::service::Snapshot) {
        let old = self.selection;
        ui.horizontal_wrapped(|ui| {
            egui::Frame::new()
                .fill(SURFACE)
                .corner_radius(18)
                .inner_margin(5)
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing.x = 3.;
                        for (days, name) in [(1, "Today"), (7, "7 days"), (30, "30 days")] {
                            if Self::button(
                                ui,
                                name,
                                if self.selection.days == days {
                                    TEXT
                                } else {
                                    SURFACE
                                },
                                if self.selection.days == days {
                                    SURFACE
                                } else {
                                    MUTED
                                },
                            )
                            .clicked()
                            {
                                self.selection.days = days;
                            }
                        }
                    });
                });
            let choices = rows(&s.dashboard.data["stats"]["licenses"]);
            if choices.len() > 1 {
                egui::ComboBox::from_id_salt("license-view")
                    .selected_text(
                        self.selection
                            .license
                            .map_or("All licenses".into(), |id| format!("License {id}")),
                    )
                    .show_ui(ui, |ui| {
                        ui.selectable_value(&mut self.selection.license, None, "All licenses");
                        for c in choices {
                            ui.selectable_value(
                                &mut self.selection.license,
                                c["id"].as_u64(),
                                format!(
                                    "License {} · {}",
                                    c["id"],
                                    rows(&c["accounts"])
                                        .iter()
                                        .map(word)
                                        .collect::<Vec<_>>()
                                        .join(", ")
                                ),
                            );
                        }
                    });
            }
            if ui
                .add_enabled(
                    !s.dashboard.refreshing,
                    egui::Button::new(if s.dashboard.refreshing {
                        "Refreshing…"
                    } else {
                        "Refresh"
                    })
                    .frame(false),
                )
                .clicked()
            {
                self.refresh_dashboard();
            }
            if s.dashboard.updated_at > 0 {
                ui.label(
                    RichText::new(format!(
                        "Updated {}s ago",
                        now_ms().saturating_sub(s.dashboard.updated_at) / 1000
                    ))
                    .size(12.)
                    .color(MUTED),
                );
            }
        });
        if self.selection != old {
            self.refresh_dashboard();
        }
        ui.add_space(20.);
    }
    fn metric(ui: &mut egui::Ui, title: &str, value: &str, note: &str, tint: Color) {
        Self::card().show(ui, |ui| {
            ui.set_min_width(ui.available_width());
            ui.label(RichText::new(title).size(12.).color(MUTED));
            ui.add_space(13.);
            ui.label(RichText::new(value).size(29.).color(TEXT));
            ui.add_space(9.);
            Self::badge(ui, note, tint, TEXT);
        });
    }
    fn dashboard_page(&mut self, ui: &mut egui::Ui, s: &kirk_companion::service::Snapshot) {
        let data = &s.dashboard.data;
        let totals = &data["stats"]["totals"];
        let loaded = !totals.is_null();
        ui.label(
            RichText::new("Your game, at a glance.")
                .size(27.)
                .color(TEXT),
        );
        ui.add_space(4.);
        ui.label(
            RichText::new("A home for your clients, progress, and next session.")
                .size(12.)
                .color(MUTED),
        );
        ui.add_space(20.);
        self.dashboard_toolbar(ui, s);
        if !s.paired {
            Self::card().show(ui, |ui| {
                Self::heading(
                    ui,
                    "Bring your Kirk account here.",
                    "Pair once to see your figures and control your connected games.",
                );
                ui.add_space(18.);
                if Self::button(ui, "Connect computer", TEXT, SURFACE).clicked() {
                    self.page = Page::Connection;
                }
            });
            ui.add_space(20.);
        }
        ui.columns(3, |cols| {
            Self::metric(
                &mut cols[0],
                "Earned",
                &if loaded {
                    amount(number(&totals["coins"]) + number(&totals["items_value"]))
                } else {
                    "—".into()
                },
                "Coins + item value",
                Color::from_rgb(53, 65, 42),
            );
            Self::metric(
                &mut cols[1],
                "Macro time",
                &if loaded {
                    elapsed(number(&totals["seconds"]))
                } else {
                    "—".into()
                },
                "In this period",
                Color::from_rgb(57, 52, 65),
            );
            Self::metric(
                &mut cols[2],
                "Skill XP",
                &if loaded {
                    amount(number(&totals["xp"]))
                } else {
                    "—".into()
                },
                "Across your skills",
                Color::from_rgb(41, 61, 61),
            );
        });
        ui.add_space(20.);
        if ui.available_width() > 740. {
            let available = ui.available_width();
            ui.horizontal_top(|ui| {
                ui.allocate_ui_with_layout(
                    Vec2::new(available * 0.61 - 8., 0.),
                    egui::Layout::top_down(Align::LEFT),
                    |ui| {
                        self.activity_card(ui, s);
                        ui.add_space(18.);
                        self.quick_launch(ui, s);
                    },
                );
                ui.allocate_ui_with_layout(
                    Vec2::new(available * 0.39 - 8., 0.),
                    egui::Layout::top_down(Align::LEFT),
                    |ui| {
                        self.game_card(ui, s);
                        ui.add_space(18.);
                        self.powder_card(ui, s);
                    },
                );
            });
        } else {
            self.activity_card(ui, s);
            ui.add_space(18.);
            self.game_card(ui, s);
            ui.add_space(18.);
            self.quick_launch(ui, s);
        }
    }
    fn activity_card(&mut self, ui: &mut egui::Ui, s: &kirk_companion::service::Snapshot) {
        Self::card().show(ui, |ui| {
            Self::heading(
                ui,
                "Time well spent",
                "Macro activity · last 30 days · midnight UTC",
            );
            ui.add_space(23.);
            let series = rows(&s.dashboard.data["month"]["series"]);
            let sum = series
                .iter()
                .rev()
                .take(self.selection.days as usize)
                .map(|d| number(&d["seconds"]))
                .sum();
            ui.label(
                RichText::new(if series.is_empty() {
                    "—".into()
                } else {
                    elapsed(sum)
                })
                .size(34.)
                .color(TEXT),
            );
            ui.add_space(22.);
            if series.is_empty() {
                ui.label(
                    RichText::new(if s.paired {
                        "Your activity will appear here after the dashboard connects."
                    } else {
                        "Connect your account to see your history."
                    })
                    .color(MUTED),
                );
                ui.add_space(125.);
            } else {
                let (area, _) = ui.allocate_exact_size(
                    Vec2::new(ui.available_width(), 155.),
                    egui::Sense::hover(),
                );
                let peak = series
                    .iter()
                    .map(|d| number(&d["seconds"]))
                    .fold(3600., f64::max);
                let step = area.width() / series.len() as f32;
                for (i, d) in series.iter().enumerate() {
                    let height = (number(&d["seconds"]) / peak) as f32 * (area.height() - 10.);
                    let bar = egui::Rect::from_min_max(
                        egui::pos2(
                            area.left() + i as f32 * step + 2.,
                            area.bottom() - height.max(3.),
                        ),
                        egui::pos2(area.left() + (i + 1) as f32 * step - 2., area.bottom()),
                    );
                    ui.painter().rect_filled(
                        bar,
                        4.,
                        if i + self.selection.days as usize >= series.len() {
                            Color::from_rgb(228, 209, 103)
                        } else {
                            Color::from_rgb(62, 65, 63)
                        },
                    );
                    ui.interact(
                        bar.expand(2.),
                        ui.id().with(("day", i)),
                        egui::Sense::hover(),
                    )
                    .on_hover_text(format!(
                        "{}\n{} macro time\n{} earned",
                        word(&d["day"]),
                        elapsed(number(&d["seconds"])),
                        amount(number(&d["coins"]) + number(&d["items_value"]))
                    ));
                }
                ui.add_space(8.);
                ui.horizontal(|ui| {
                    ui.label(RichText::new(word(&series[0]["day"])).small().color(MUTED));
                    ui.with_layout(egui::Layout::right_to_left(Align::Center), |ui| {
                        ui.label(
                            RichText::new(word(&series[series.len() - 1]["day"]))
                                .small()
                                .color(MUTED),
                        );
                    });
                });
            }
        });
    }
    fn game_card(&mut self, ui: &mut egui::Ui, s: &kirk_companion::service::Snapshot) {
        let online = self.online(s);
        let live = &s.dashboard.live;
        egui::Frame::new()
            .fill(Color::from_rgb(19, 21, 21))
            .corner_radius(23)
            .inner_margin(24)
            .show(ui, |ui| {
                ui.set_min_width(ui.available_width());
                ui.label(
                    RichText::new("YOUR CLIENT")
                        .size(12.)
                        .color(Color::from_gray(170)),
                );
                ui.add_space(16.);
                ui.horizontal(|ui| {
                    Self::icon(ui, "monitor", AQUA, 25.);
                    ui.label(
                        RichText::new(if online {
                            live["player"].as_str().unwrap_or("Game online")
                        } else {
                            "Game offline"
                        })
                        .size(21.)
                        .color(TEXT),
                    );
                });
                ui.add_space(12.);
                ui.label(
                    RichText::new(if online {
                        [live["area"].as_str(), live["zone"].as_str()]
                            .into_iter()
                            .flatten()
                            .collect::<Vec<_>>()
                            .join(" · ")
                    } else {
                        "Start your saved instance to connect.".into()
                    })
                    .size(12.)
                    .color(Color::from_gray(185)),
                );
                ui.add_space(17.);
                let clients = rows(&s.dashboard.data["device"]["clients"]);
                if clients.len() > 1 {
                    egui::ComboBox::from_id_salt("game-client")
                        .selected_text(
                            clients
                                .iter()
                                .find(|c| c["id"].as_u64() == self.selection.client)
                                .map_or("Choose a client", |c| word(&c["name"])),
                        )
                        .show_ui(ui, |ui| {
                            for c in clients {
                                if ui
                                    .selectable_value(
                                        &mut self.selection.client,
                                        c["id"].as_u64(),
                                        format!(
                                            "{} · {}",
                                            word(&c["name"]),
                                            if c["online"] == true {
                                                "Online"
                                            } else {
                                                "Offline"
                                            }
                                        ),
                                    )
                                    .changed()
                                {
                                    self.refresh_dashboard();
                                }
                            }
                        });
                    ui.add_space(12.);
                }
                for (label, value) in [
                    ("Purse", live["purse"].as_str().unwrap_or("—")),
                    (
                        "Running",
                        rows(&live["macros"]).first().map_or("Nothing", word),
                    ),
                ] {
                    ui.horizontal(|ui| {
                        ui.label(RichText::new(label).size(12.).color(Color::from_gray(170)));
                        ui.with_layout(egui::Layout::right_to_left(Align::Center), |ui| {
                            ui.add(
                                egui::Label::new(RichText::new(value).size(12.).color(TEXT))
                                    .truncate(),
                            );
                        });
                    });
                }
                ui.add_space(18.);
                ui.add_enabled_ui(online, |ui| {
                    ui.horizontal(|ui| {
                        if Self::button(
                            ui,
                            "Stop all",
                            Color::from_rgb(74, 48, 55),
                            Color::from_rgb(255, 174, 190),
                        )
                        .clicked()
                        {
                            self.remote("stop", json!({"macro":"all"}));
                        }
                        if ui
                            .add_enabled(
                                !s.dashboard.acting,
                                egui::Button::new(RichText::new("Screen ↗").color(TEXT))
                                    .fill(Color::from_gray(58))
                                    .corner_radius(11),
                            )
                            .clicked()
                        {
                            self.page = Page::Screen;
                            self.remote("screenshot", json!({}));
                        }
                    });
                });
            });
    }
    fn quick_launch(&mut self, ui: &mut egui::Ui, s: &kirk_companion::service::Snapshot) {
        Self::card().show(ui, |ui| {
            Self::heading(
                ui,
                "Ready when you are",
                "Launch a saved instance on this computer.",
            );
            ui.add_space(15.);
            if s.settings.profiles.is_empty()
                && Self::button(ui, "+ Add launch profile", BG, TEXT).clicked()
            {
                self.page = Page::Profiles;
                self.editor = Some(Profile::default());
            }
            for profile in s.settings.profiles.iter().take(3) {
                ui.horizontal(|ui| {
                    ui.with_layout(egui::Layout::right_to_left(Align::Center), |ui| {
                        if ui
                            .add_enabled(
                                !s.working,
                                egui::Button::new(RichText::new("Launch ↗").color(TEXT))
                                    .fill(BG)
                                    .corner_radius(12),
                            )
                            .clicked()
                        {
                            self.send(Command::Test(profile.id));
                        }
                        ui.add(egui::Label::new(RichText::new(&profile.name).size(12.)).truncate());
                    });
                });
            }
        });
    }
    fn powder_card(&self, ui: &mut egui::Ui, s: &kirk_companion::service::Snapshot) {
        Self::card().show(ui, |ui| {
            Self::heading(ui, "A little more progress", "Powder gained in this period");
            ui.add_space(18.);
            let powder = &s.dashboard.data["trackers"]["totals"]["powder"];
            for (id, name, tint) in [
                ("mithril", "Mithril", Color::from_rgb(42, 57, 60)),
                ("gemstone", "Gemstone", Color::from_rgb(65, 44, 49)),
                ("glacite", "Glacite", Color::from_rgb(55, 51, 64)),
            ] {
                ui.horizontal(|ui| {
                    Self::badge(ui, name, tint, TEXT);
                    ui.with_layout(egui::Layout::right_to_left(Align::Center), |ui| {
                        ui.label(
                            RichText::new(if powder.is_null() {
                                "—".into()
                            } else {
                                amount(number(&powder[id]))
                            })
                            .size(17.)
                            .color(TEXT),
                        );
                    });
                });
            }
        });
    }
    fn macro_page(&mut self, ui: &mut egui::Ui, s: &kirk_companion::service::Snapshot) {
        Self::heading(
            ui,
            "Make your next move.",
            "Start and stop Home macros on the selected game client.",
        );
        ui.add_space(20.);
        self.game_card(ui, s);
        ui.add_space(20.);
        let columns = if ui.available_width() > 830. { 3 } else { 2 };
        let online = self.online(s);
        let running: Vec<_> = rows(&s.dashboard.live["macros"])
            .iter()
            .map(|v| word(v).to_lowercase())
            .collect();
        for chunk in MACROS.chunks(columns) {
            ui.columns(columns, |cols| {
                for (i, (id, title, about)) in chunk.iter().enumerate() {
                    Self::card().show(&mut cols[i], |ui| {
                        ui.set_min_width(ui.available_width());
                        let on = running.iter().any(|r| {
                            r.contains(id)
                                || (*id == "mithril" && r.contains("auto miner"))
                                || (*id == "hunting" && r.contains("hunting"))
                        });
                        Self::badge(
                            ui,
                            if on {
                                "Running"
                            } else if online {
                                "Ready"
                            } else {
                                "Game closed"
                            },
                            if on { Color::from_rgb(53, 65, 42) } else { BG },
                            TEXT,
                        );
                        ui.add_space(15.);
                        ui.label(RichText::new(*title).size(17.).color(TEXT));
                        ui.label(RichText::new(*about).size(12.).color(MUTED));
                        ui.add_space(18.);
                        ui.add_enabled_ui(online && !s.dashboard.acting, |ui| {
                            if Self::button(
                                ui,
                                if on { "Stop" } else { "Start" },
                                if on {
                                    Color::from_rgb(68, 44, 49)
                                } else {
                                    TEXT
                                },
                                if on { PINK } else { SURFACE },
                            )
                            .clicked()
                            {
                                self.remote(if on { "stop" } else { "start" }, json!({"macro":id}));
                            }
                        });
                    });
                }
            });
            ui.add_space(18.);
        }
        Self::card().show(ui, |ui| {
            Self::heading(
                ui,
                "Go somewhere",
                "Uses Kirk's existing in-game warp commands.",
            );
            ui.add_space(15.);
            ui.horizontal(|ui| {
                egui::ComboBox::from_id_salt("warp-place")
                    .selected_text(&self.place)
                    .show_ui(ui, |ui| {
                        for p in [
                            "hub", "island", "park", "end", "forge", "mines", "garden", "crypts",
                            "dhub", "barn", "desert", "spider", "gold", "deep", "museum", "castle",
                            "wizard", "trapper", "winter", "cn",
                        ] {
                            ui.selectable_value(&mut self.place, p.into(), p);
                        }
                    });
                ui.add_enabled_ui(online && !s.dashboard.acting, |ui| {
                    if Self::button(ui, "Warp", BG, TEXT).clicked() {
                        self.remote("lobby", json!({"place":self.place}));
                    }
                });
            });
        });
    }
    fn statistics_page(&mut self, ui: &mut egui::Ui, s: &kirk_companion::service::Snapshot) {
        Self::heading(
            ui,
            "See how far you've come.",
            "Your own game reports, saved across sessions.",
        );
        ui.add_space(20.);
        self.dashboard_toolbar(ui, s);
        self.activity_card(ui, s);
        ui.add_space(18.);
        Self::card().show(ui, |ui| {
            Self::heading(
                ui,
                "By macro",
                "Time, coins, item value, and skill XP in this period.",
            );
            ui.add_space(20.);
            egui::ScrollArea::horizontal().show(ui, |ui| {
                egui::Grid::new("macro-stats")
                    .spacing([24., 15.])
                    .striped(true)
                    .show(ui, |ui| {
                        for name in ["Macro", "Time", "Coins", "Items", "XP"] {
                            ui.label(RichText::new(name).size(12.).color(MUTED));
                        }
                        ui.end_row();
                        for row in rows(&s.dashboard.data["stats"]["macros"]) {
                            ui.label(word(&row["macro"]));
                            ui.label(elapsed(number(&row["seconds"])));
                            for key in ["coins", "items_value", "xp"] {
                                ui.label(amount(number(&row[key])));
                            }
                            ui.end_row();
                        }
                    });
            });
            if rows(&s.dashboard.data["stats"]["macros"]).is_empty() {
                ui.label(RichText::new("No macro activity recorded in this period.").color(MUTED));
            }
        });
        ui.add_space(18.);
        Self::card().show(ui, |ui| {
            Self::heading(ui, "Skill XP", "Gained while your macros ran.");
            ui.add_space(17.);
            for row in rows(&s.dashboard.data["stats"]["skills"]) {
                Self::value_row(ui, word(&row["skill"]), &amount(number(&row["xp"])));
            }
        });
    }
    fn value_row(ui: &mut egui::Ui, label: &str, value: &str) {
        ui.horizontal(|ui| {
            ui.label(RichText::new(label).size(12.).color(MUTED));
            ui.with_layout(egui::Layout::right_to_left(Align::Center), |ui| {
                ui.add(egui::Label::new(RichText::new(value).size(13.).color(TEXT)).truncate());
            });
        });
    }
    fn trackers_page(&mut self, ui: &mut egui::Ui, s: &kirk_companion::service::Snapshot) {
        Self::heading(
            ui,
            "Every little gain.",
            "Powder, profit, drops, and items counted by your game.",
        );
        ui.add_space(20.);
        self.dashboard_toolbar(ui, s);
        self.powder_card(ui, s);
        ui.add_space(18.);
        let trk = &s.dashboard.data["trackers"];
        Self::card().show(ui, |ui| {
            Self::heading(
                ui,
                "Profit by macro",
                "Saved tracker profit for this period.",
            );
            ui.add_space(18.);
            for row in rows(&trk["trackers"]) {
                Self::value_row(
                    ui,
                    row["title"].as_str().unwrap_or(word(&row["tracker"])),
                    &format!(
                        "{} · {}",
                        amount(number(&row["profit"])),
                        elapsed(number(&row["uptime_ms"]) / 1000.)
                    ),
                );
            }
            if rows(&trk["trackers"]).is_empty() {
                ui.label(
                    RichText::new(
                        "No tracker reports saved yet. Reports arrive while your game is running.",
                    )
                    .color(MUTED),
                );
            }
            if let Some(counters) = trk["totals"]["counters"].as_object() {
                for (name, n) in counters {
                    Self::value_row(ui, name, &amount(number(n)));
                }
            }
        });
        ui.add_space(18.);
        Self::card().show(ui, |ui| {
            Self::heading(
                ui,
                "Tracked items",
                "Counts and values from your game, at the prices when last read.",
            );
            ui.add_space(18.);
            for row in rows(&trk["items"]) {
                Self::value_row(
                    ui,
                    row["name"].as_str().unwrap_or(word(&row["item"])),
                    &format!(
                        "× {}   ·   {}",
                        amount(number(&row["amount"])),
                        amount(number(&row["value"]))
                    ),
                );
            }
            if rows(&trk["items"]).is_empty() {
                ui.label(RichText::new("Nothing picked up in this period yet.").color(MUTED));
            }
        });
    }
    fn screen_page(&mut self, ui: &mut egui::Ui, s: &kirk_companion::service::Snapshot) {
        use base64::Engine;
        Self::heading(
            ui,
            "A window into your game.",
            "Capture the selected game client when you want a fresh look.",
        );
        ui.add_space(20.);
        self.game_card(ui, s);
        ui.add_space(18.);
        let shot = &s.dashboard.screenshot;
        if let Some(encoded) = shot["base64"].as_str()
            && self.screenshot_stamp != encoded
        {
            self.screenshot_stamp = encoded.into();
            self.screenshot_texture = None;
            let result = (|| -> anyhow::Result<_> {
                anyhow::ensure!(
                    encoded.len() <= 8 * 1024 * 1024 && shot["mime"] == "image/png",
                    "Invalid game screenshot."
                );
                let bytes = base64::engine::general_purpose::STANDARD.decode(encoded)?;
                let mut reader = image::ImageReader::with_format(
                    std::io::Cursor::new(bytes),
                    image::ImageFormat::Png,
                );
                let mut limits = image::Limits::default();
                limits.max_image_width = Some(4096);
                limits.max_image_height = Some(4096);
                limits.max_alloc = Some(64 * 1024 * 1024);
                reader.limits(limits);
                let rgba = reader.decode()?.into_rgba8();
                Ok(egui::ColorImage::from_rgba_unmultiplied(
                    [rgba.width() as usize, rgba.height() as usize],
                    rgba.as_raw(),
                ))
            })();
            match result {
                Ok(image) => {
                    self.screenshot_texture = Some(ui.ctx().load_texture(
                        "game-screen",
                        image,
                        egui::TextureOptions::LINEAR,
                    ))
                }
                Err(e) => self.notice = e.to_string(),
            }
        }
        Self::card().show(ui, |ui| {
            if let Some(texture) = &self.screenshot_texture {
                ui.add(
                    egui::Image::new(texture)
                        .fit_to_exact_size(Vec2::new(
                            ui.available_width(),
                            ui.available_width() * texture.size_vec2().y / texture.size_vec2().x,
                        ))
                        .corner_radius(15),
                );
            } else {
                ui.add_space(60.);
                ui.label(
                    RichText::new(if s.dashboard.acting {
                        "Capturing your game…"
                    } else {
                        "Your next screenshot will appear here."
                    })
                    .color(MUTED),
                );
                ui.add_space(60.);
            }
        });
    }
    fn assistant_page(&mut self, ui: &mut egui::Ui, s: &kirk_companion::service::Snapshot) {
        Self::heading(
            ui,
            "A word with Kirk.",
            "The same assistant conversation you have in the game.",
        );
        ui.add_space(20.);
        let online = self.online(s);
        Self::card().show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.add_enabled_ui(online && !s.dashboard.acting, |ui| {
                    if Self::button(ui, "Sync conversation", BG, TEXT).clicked() {
                        self.remote("conversation", json!({}));
                    }
                    if ui.button("Clear conversation").clicked() {
                        self.remote("conversation", json!({"do":"forget"}));
                    }
                });
            });
            ui.add_space(20.);
            egui::ScrollArea::vertical()
                .id_salt("assistant-thread")
                .max_height(370.)
                .stick_to_bottom(true)
                .show(ui, |ui| {
                    for line in rows(&s.dashboard.conversation["lines"]) {
                        egui::Frame::new()
                            .fill(if line["kirk"] == true {
                                BG
                            } else {
                                Color::from_rgb(57, 54, 42)
                            })
                            .corner_radius(16)
                            .inner_margin(16)
                            .show(ui, |ui| {
                                ui.label(
                                    RichText::new(if line["kirk"] == true {
                                        "Kirk"
                                    } else {
                                        "You"
                                    })
                                    .size(12.)
                                    .color(MUTED),
                                );
                                ui.label(word(&line["text"]));
                            });
                        ui.add_space(12.);
                    }
                    if s.dashboard.acting {
                        ui.label(RichText::new("Kirk is working…").color(MUTED));
                    }
                });
            ui.add_space(20.);
            let response = ui.add(
                egui::TextEdit::multiline(&mut self.question)
                    .desired_rows(3)
                    .desired_width(f32::INFINITY)
                    .char_limit(500)
                    .hint_text("Ask Kirk about your game…")
                    .margin(Vec2::splat(14.)),
            );
            let keyboard_send = response.has_focus()
                && ui.input(|i| i.modifiers.ctrl && i.key_pressed(egui::Key::Enter));
            ui.add_space(10.);
            ui.add_enabled_ui(
                online && !s.dashboard.acting && !self.question.trim().is_empty(),
                |ui| {
                    if Self::button(ui, "Send to Kirk", TEXT, SURFACE).clicked() || keyboard_send {
                        let text = self.question.trim().to_owned();
                        self.remote("ask", json!({"text":text}));
                        self.question.clear();
                    }
                },
            );
            if !online {
                ui.label(RichText::new("Open your linked game to talk with Kirk.").color(MUTED));
            }
        });
        ui.add_space(18.);
        Self::card().show(ui, |ui| {
            Self::heading(
                ui,
                "Recent game chat",
                "The latest messages from your client.",
            );
            ui.add_space(16.);
            for line in rows(&s.dashboard.live["chat"]) {
                ui.label(RichText::new(word(line)).size(12.));
            }
        });
    }
}
