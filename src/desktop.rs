// Copyright (c) 2026 Benja (Benjamacs1)
// SPDX-License-Identifier: MIT
use eframe::egui::{self, Align, Color32 as Color, FontId, RichText, Vec2};
use kirk_companion::{
    launch::{self, Discovered},
    model::*,
    service::{Command, Shared},
    storage::DiskStore,
};
use std::{path::PathBuf, sync::mpsc, time::Duration};
use tray_icon::{
    MouseButton, MouseButtonState, TrayIcon, TrayIconBuilder, TrayIconEvent,
    menu::{Menu, MenuEvent, MenuItem, PredefinedMenuItem},
};

const BG: Color = Color::from_rgb(25, 26, 28);
const SURFACE: Color = Color::from_rgb(39, 41, 42);
const TEXT: Color = Color::from_rgb(242, 240, 232);
const MUTED: Color = Color::from_rgb(164, 165, 162);
const ACCENT: Color = Color::from_rgb(234, 216, 110);
const AQUA: Color = Color::from_rgb(142, 193, 175);
const PINK: Color = Color::from_rgb(231, 153, 163);
const LINE: Color = Color::from_rgb(56, 58, 59);

#[derive(Clone, Copy, PartialEq)]
enum Page {
    Dashboard,
    Macros,
    Statistics,
    Trackers,
    Screen,
    Assistant,
    Connection,
    Profiles,
    Preferences,
}
enum TrayAction {
    Show,
    Dashboard,
    Quit,
}
pub struct Desktop {
    pub shared: Shared,
    pub sender: tokio::sync::mpsc::UnboundedSender<Command>,
    pub runtime: Option<tokio::runtime::Runtime>,
    pub store: DiskStore,
    pub preview: bool,
    pub screenshot: Option<PathBuf>,
    pub frames: usize,
    page: Page,
    name: String,
    code: String,
    backend: String,
    advanced: bool,
    editor: Option<Profile>,
    editor_saving: bool,
    discovered: Vec<Discovered>,
    notice: String,
    startup: bool,
    tray: Option<TrayIcon>,
    tray_events: mpsc::Receiver<TrayAction>,
    quitting: bool,
    background: bool,
    selection: kirk_companion::dashboard::Selection,
    next_refresh: std::time::Instant,
    question: String,
    place: String,
    screenshot_texture: Option<egui::TextureHandle>,
    screenshot_stamp: String,
}
impl Desktop {
    #[allow(clippy::too_many_arguments)] // Window boot dependencies are supplied once by main.
    pub fn new(
        cc: &eframe::CreationContext<'_>,
        shared: Shared,
        sender: tokio::sync::mpsc::UnboundedSender<Command>,
        runtime: Option<tokio::runtime::Runtime>,
        store: DiskStore,
        preview: bool,
        screenshot: Option<PathBuf>,
        background: bool,
    ) -> Self {
        let ctx = &cc.egui_ctx;
        let mut fonts = egui::FontDefinitions::default();
        fonts.font_data.insert(
            "kirk-squares".into(),
            std::sync::Arc::new(egui::FontData::from_static(include_bytes!(
                "../assets/fonts/Squares-Bold.ttf"
            ))),
        );
        fonts
            .families
            .entry(egui::FontFamily::Proportional)
            .or_default()
            .insert(0, "kirk-squares".into());
        ctx.set_fonts(fonts);
        let mut style = egui::Style {
            visuals: egui::Visuals::dark(),
            ..Default::default()
        };
        style.visuals.override_text_color = Some(TEXT);
        style.visuals.panel_fill = BG;
        style.visuals.window_fill = SURFACE;
        style.visuals.text_edit_bg_color = Some(BG);
        style.visuals.window_highlight_topmost = false;
        style.visuals.widgets.noninteractive.bg_stroke = egui::Stroke::new(1., LINE);
        style.visuals.widgets.inactive.bg_fill = Color::from_rgb(43, 45, 46);
        style.visuals.widgets.inactive.weak_bg_fill = Color::from_rgb(43, 45, 46);
        style.visuals.widgets.inactive.fg_stroke.color = TEXT;
        style.visuals.widgets.hovered.bg_fill = Color::from_rgb(56, 58, 59);
        style.visuals.widgets.active.bg_fill = Color::from_rgb(69, 71, 70);
        style.visuals.selection.bg_fill = Color::from_rgb(100, 91, 48);
        style.spacing.item_spacing = Vec2::new(12., 10.);
        style.spacing.button_padding = Vec2::new(15., 10.);
        style.spacing.interact_size.y = 22.;
        style
            .text_styles
            .insert(egui::TextStyle::Body, FontId::proportional(14.));
        style
            .text_styles
            .insert(egui::TextStyle::Button, FontId::proportional(13.));
        style
            .text_styles
            .insert(egui::TextStyle::Small, FontId::proportional(12.));
        ctx.set_theme(egui::Theme::Dark);
        ctx.set_style_of(egui::Theme::Dark, style);
        let snapshot = shared.read().unwrap_or_else(|e| e.into_inner()).clone();
        let (tx, rx) = mpsc::channel();
        let (tray, notice) = if preview {
            (None, String::new())
        } else {
            match create_tray(ctx.clone(), tx) {
                Ok(tray) => (Some(tray), String::new()),
                Err(_) => (
                    None,
                    "System tray is unavailable. Closing the window will quit the companion."
                        .into(),
                ),
            }
        };
        let startup = startup_config()
            .and_then(|s| s.is_enabled().map_err(Into::into))
            .unwrap_or(false);
        Self {
            shared,
            sender,
            runtime,
            store,
            preview,
            screenshot,
            frames: 0,
            page: Page::Dashboard,
            name: snapshot.settings.name,
            backend: snapshot.settings.backend,
            code: String::new(),
            advanced: false,
            editor: None,
            editor_saving: false,
            discovered: launch::discover(),
            notice,
            startup,
            tray,
            tray_events: rx,
            quitting: false,
            background,
            selection: Default::default(),
            next_refresh: std::time::Instant::now(),
            question: String::new(),
            place: "hub".into(),
            screenshot_texture: None,
            screenshot_stamp: String::new(),
        }
    }
    fn send(&mut self, command: Command) {
        self.notice.clear();
        if self.preview {
            self.notice = "Design preview — no launcher or account was changed.".into();
            return;
        }
        let _ = self.sender.send(command);
    }
    fn dashboard(&mut self) {
        if !self.preview
            && let Err(e) = launch::open_dashboard()
        {
            self.notice = e.to_string();
        }
    }
    fn heading(ui: &mut egui::Ui, title: &str, subtitle: &str) {
        ui.vertical(|ui| {
            ui.label(RichText::new(title).size(17.).strong().color(TEXT));
            ui.add_space(2.);
            ui.label(RichText::new(subtitle).size(12.).color(MUTED));
        });
    }
    fn card() -> egui::Frame {
        egui::Frame::new()
            .fill(SURFACE)
            .corner_radius(20)
            .inner_margin(24)
            .shadow(egui::Shadow {
                offset: [0, 5],
                blur: 22,
                spread: 0,
                color: Color::from_black_alpha(6),
            })
    }
    fn button(ui: &mut egui::Ui, text: &str, fill: Color, ink: Color) -> egui::Response {
        ui.add(
            egui::Button::new(RichText::new(text).size(12.).strong().color(ink))
                .fill(fill)
                .corner_radius(11)
                .min_size(Vec2::new(0., 39.)),
        )
    }
    fn icon(ui: &mut egui::Ui, kind: &str, color: Color, size: f32) {
        let tile_size = (size * 1.65).max(28.);
        let (tile, _) = ui.allocate_exact_size(Vec2::splat(tile_size), egui::Sense::hover());
        let background = Color::from_rgb(53, 56, 58);
        let painter = ui.painter();
        painter.rect_filled(tile, (tile_size * 0.26).round(), background);
        let origin = tile.center() - Vec2::splat(size * 0.5);
        let p = |x: f32, y: f32| origin + Vec2::new(x * size, y * size);
        let rect = |x: f32, y: f32, w: f32, h: f32| {
            egui::Rect::from_min_size(p(x, y), Vec2::new(w * size, h * size))
        };
        let polygon = |points: Vec<egui::Pos2>| {
            egui::Shape::convex_polygon(points, color, egui::Stroke::NONE)
        };
        match kind {
            "grid" | "home" => {
                painter.add(polygon(vec![p(0.02, 0.45), p(0.5, 0.05), p(0.98, 0.45)]));
                painter.rect_filled(rect(0.18, 0.39, 0.64, 0.55), size * 0.1, color);
                painter.rect_filled(rect(0.42, 0.64, 0.17, 0.32), size * 0.06, background);
            }
            "monitor" => {
                painter.rect_filled(rect(0.02, 0.1, 0.96, 0.64), size * 0.09, color);
                painter.rect_filled(rect(0.15, 0.22, 0.7, 0.39), size * 0.03, background);
                painter.rect_filled(rect(0.43, 0.7, 0.14, 0.22), size * 0.03, color);
                painter.rect_filled(rect(0.27, 0.87, 0.46, 0.1), size * 0.04, color);
            }
            "chart" => {
                for (x, y, h) in [(0.08, 0.48, 0.43), (0.4, 0.12, 0.79), (0.72, 0.31, 0.6)] {
                    painter.rect_filled(rect(x, y, 0.2, h), size * 0.07, color);
                }
            }
            "chat" => {
                painter.rect_filled(rect(0.02, 0.1, 0.96, 0.68), size * 0.18, color);
                painter.add(polygon(vec![p(0.18, 0.68), p(0.18, 0.97), p(0.48, 0.7)]));
                for x in [0.3, 0.5, 0.7] {
                    painter.circle_filled(p(x, 0.43), size * 0.045, background);
                }
            }
            "settings" => {
                let points = (0..32)
                    .map(|i| {
                        let angle = i as f32 * std::f32::consts::TAU / 32.;
                        let radius = if i % 4 < 2 { 0.49 } else { 0.39 };
                        p(0.5 + angle.cos() * radius, 0.5 + angle.sin() * radius)
                    })
                    .collect();
                painter.add(polygon(points));
                painter.circle_filled(p(0.5, 0.5), size * 0.17, background);
            }
            "link" => {
                let pen = egui::Stroke::new(size * 0.18, color);
                painter.circle_stroke(p(0.3, 0.68), size * 0.21, pen);
                painter.circle_stroke(p(0.7, 0.3), size * 0.21, pen);
                painter.line_segment([p(0.3, 0.68), p(0.7, 0.3)], pen);
            }
            "folder" => {
                painter.rect_filled(rect(0.04, 0.15, 0.42, 0.22), size * 0.06, color);
                painter.rect_filled(rect(0.04, 0.29, 0.92, 0.58), size * 0.1, color);
            }
            _ => {
                painter.add(polygon(vec![p(0.26, 0.06), p(0.94, 0.5), p(0.26, 0.94)]));
            }
        }
    }
    fn badge(ui: &mut egui::Ui, text: &str, tint: Color, foreground: Color) {
        egui::Frame::new()
            .fill(tint)
            .corner_radius(20)
            .inner_margin(egui::Margin::symmetric(11, 5))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    if text.starts_with(['●', '○']) {
                        let (rect, _) =
                            ui.allocate_exact_size(Vec2::splat(7.), egui::Sense::hover());
                        ui.painter().circle_filled(rect.center(), 2.5, foreground);
                    }
                    ui.label(
                        RichText::new(text.trim_start_matches(['●', '○', ' ']))
                            .size(12.)
                            .color(foreground),
                    );
                });
            });
    }
    fn input(ui: &mut egui::Ui, label: &str, value: &mut String, hint: &str) {
        ui.label(RichText::new(label).size(12.).color(MUTED));
        ui.add(
            egui::TextEdit::singleline(value)
                .hint_text(hint)
                .desired_width(f32::INFINITY)
                .margin(Vec2::new(12., 11.)),
        );
    }
    fn header(&mut self, ui: &mut egui::Ui) {
        let header = egui::Frame::new()
            .fill(SURFACE)
            .inner_margin(egui::Margin::symmetric(23, 13))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label(RichText::new("K").size(24.).strong().color(TEXT));
                    ui.label(RichText::new("Kirk").size(17.).strong());
                    ui.label(RichText::new("Companion").size(13.).color(MUTED));
                    ui.with_layout(egui::Layout::right_to_left(Align::Center), |ui| {
                        if ui
                            .add(egui::Button::new(RichText::new("×").size(20.)).frame(false))
                            .clicked()
                        {
                            ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
                        }
                        if ui.add(egui::Button::new("—").frame(false)).clicked() {
                            ui.ctx()
                                .send_viewport_cmd(egui::ViewportCommand::Minimized(true));
                        }
                        if self.preview {
                            Self::badge(ui, "DESIGN PREVIEW", BG, MUTED);
                        }
                        ui.menu_button("Account", |ui| {
                            ui.label(RichText::new("Your Kirk workspace").strong());
                            ui.separator();
                            if ui.button("Connection").clicked() {
                                self.page = Page::Connection;
                                ui.close();
                            }
                            if ui.button("Preferences").clicked() {
                                self.page = Page::Preferences;
                                ui.close();
                            }
                            if ui.button("Open website ↗").clicked() {
                                self.dashboard();
                                ui.close();
                            }
                        });
                    });
                });
            });
        let drag_rect = egui::Rect::from_min_max(
            header.response.rect.min,
            egui::pos2(
                header.response.rect.right() - 350.,
                header.response.rect.bottom(),
            ),
        );
        if ui
            .interact(drag_rect, ui.id().with("window-drag"), egui::Sense::drag())
            .drag_started()
        {
            ui.ctx().send_viewport_cmd(egui::ViewportCommand::StartDrag);
        }
    }
    fn sidebar(&mut self, ui: &mut egui::Ui) {
        egui::Frame::new()
            .fill(BG)
            .inner_margin(egui::Margin {
                left: 14,
                right: 14,
                top: 22,
                bottom: 20,
            })
            .show(ui, |ui| {
                ui.set_width(192.);
                ui.set_min_height(ui.available_height());
                ui.label(RichText::new("WORKSPACE").size(12.).color(MUTED));
                ui.add_space(18.);
                egui::ScrollArea::vertical()
                    .id_salt("navigation")
                    .max_height((ui.available_height() - 75.).max(100.))
                    .show(ui, |ui| {
                        for (page, mark, title) in [
                            (Page::Dashboard, "grid", "Dashboard"),
                            (Page::Macros, "prism", "Macros"),
                            (Page::Statistics, "chart", "Statistics"),
                            (Page::Trackers, "chart", "Trackers"),
                            (Page::Screen, "monitor", "Screen"),
                            (Page::Assistant, "chat", "Kirk assistant"),
                            (Page::Profiles, "folder", "Launch profiles"),
                            (Page::Connection, "link", "Connection"),
                            (Page::Preferences, "settings", "Preferences"),
                        ] {
                            let active = page == self.page;
                            let response = egui::Frame::new()
                                .fill(if active { SURFACE } else { BG })
                                .corner_radius(12)
                                .inner_margin(egui::Margin::symmetric(14, 8))
                                .show(ui, |ui| {
                                    ui.set_width(164.);
                                    ui.horizontal(|ui| {
                                        Self::icon(ui, mark, TEXT, 17.);
                                        ui.label(RichText::new(title).size(12.).color(if active {
                                            TEXT
                                        } else {
                                            MUTED
                                        }));
                                    });
                                })
                                .response
                                .interact(egui::Sense::click())
                                .on_hover_cursor(egui::CursorIcon::PointingHand);
                            if response.clicked() {
                                self.page = page;
                                self.notice.clear();
                                if page == Page::Assistant {
                                    self.remote("conversation", json!({}));
                                }
                            }
                            ui.add_space(3.);
                        }
                    });
                ui.with_layout(egui::Layout::bottom_up(Align::LEFT), |ui| {
                    ui.label(
                        RichText::new("Companion 0.2.0 · MIT")
                            .size(12.)
                            .color(MUTED),
                    );
                    if ui.link("Open website ↗").clicked() {
                        self.dashboard();
                    }
                });
            });
    }
    fn connection(&mut self, ui: &mut egui::Ui, snapshot: &kirk_companion::service::Snapshot) {
        Self::heading(
            ui,
            "Make the connection.",
            "Pair this PC to view your dashboard and control your linked games.",
        );
        ui.add_space(24.);
        Self::card().show(ui, |ui| {
            ui.horizontal(|ui| {
                Self::heading(
                    ui,
                    "Your computer",
                    "Use a code from the website Pair a computer.",
                );
                Self::badge(ui, &snapshot.status, BG, ACCENT);
            });
            ui.add_space(14.);
            ui.label(RichText::new("Pairing allows this app to read your statistics, capture game screenshots, talk to Kirk, and start or stop macros on your linked clients.").size(12.).color(MUTED));
            ui.add_space(20.);
            if snapshot.paired {
                ui.label(RichText::new(&snapshot.settings.name).size(23.).strong());
                ui.add_space(8.);
                ui.label(
                    RichText::new(&snapshot.settings.backend)
                        .color(MUTED)
                        .size(12.),
                );
                ui.add_space(22.);
                if Self::button(ui, "Unpair computer", Color::from_rgb(68, 44, 49), PINK)
                    .clicked()
                {
                    self.send(Command::Unpair);
                }
            } else {
                Self::input(ui, "Computer name", &mut self.name, "Gaming PC");
                ui.add_space(17.);
                Self::input(
                    ui,
                    "Eight-character pairing code",
                    &mut self.code,
                    "ACDE FHJK",
                );
                ui.add_space(21.);
                ui.add_enabled_ui(!snapshot.working, |ui| {
                    if Self::button(ui, "Connect computer", ACCENT, SURFACE).clicked() {
                        self.send(Command::Pair {
                            backend: self.backend.clone(),
                            code: self.code.clone(),
                            name: self.name.clone(),
                        });
                    }
                });
                ui.add_space(16.);
                ui.checkbox(
                    &mut self.advanced,
                    RichText::new("Use a different Kirk server")
                        .size(12.)
                        .color(MUTED),
                );
                if self.advanced {
                    Self::input(ui, "Server URL", &mut self.backend, DEFAULT_BACKEND);
                }
            }
        });
        if snapshot.ephemeral {
            ui.add_space(16.);
            ui.label(
                RichText::new(
                    "No secure credential store is available. This pairing lasts until you quit.",
                )
                .color(PINK)
                .size(12.),
            );
        }
    }
    fn profiles(&mut self, ui: &mut egui::Ui, snapshot: &kirk_companion::service::Snapshot) {
        Self::card().show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.vertical(|ui| Self::heading(ui, "Launch profiles", "Your existing instances, just as you left them."));
                ui.with_layout(egui::Layout::right_to_left(Align::Center), |ui| { if Self::button(ui, "+ Add profile", Color::from_rgb(59, 56, 43), ACCENT).clicked() { self.editor = Some(Profile::default()); } });
            });
            ui.add_space(20.);
            if snapshot.settings.profiles.is_empty() {
                egui::Frame::new().fill(BG).corner_radius(14).inner_margin(24).show(ui, |ui| {
                    ui.set_min_width(ui.available_width());
                    ui.label(RichText::new("Give Kirk a place to start.").size(14.).color(TEXT)); ui.add_space(6.);
                    ui.label(RichText::new("Add a Prism or MultiMC instance, or choose a shortcut from another launcher.").size(12.).color(MUTED));
                });
            }
            for (index, profile) in snapshot.settings.profiles.iter().enumerate() {
                if index > 0 { ui.add_space(10.); ui.separator(); ui.add_space(10.); }
                ui.push_id(profile.id, |ui| {
                    ui.horizontal(|ui| {
                        egui::Frame::new().fill(if index % 2 == 0 { Color::from_rgb(40, 61, 59) } else { Color::from_rgb(65, 44, 49) }).corner_radius(12).inner_margin(14).show(ui, |ui| { Self::icon(ui, "prism", if index % 2 == 0 { Color::from_rgb(64, 168, 182) } else { PINK }, 24.); });
                        ui.vertical(|ui| { ui.label(RichText::new(&profile.name).size(14.).strong()); ui.label(RichText::new(format!("{}{}", profile.kind.name(), if profile.instance.is_empty() { String::new() } else { format!(" / {}", profile.instance) })).size(12.).color(MUTED)); });
                        ui.with_layout(egui::Layout::right_to_left(Align::Center), |ui| {
                            if ui.add(egui::Button::new(RichText::new("Remove").size(12.).color(MUTED)).frame(false)).clicked() { self.send(Command::Remove(profile.id)); }
                            ui.add_enabled_ui(!snapshot.working, |ui| { if Self::button(ui, "Test launch  ↗", BG, ACCENT).clicked() { self.send(Command::Test(profile.id)); } });
                        });
                    });
                });
            }
        });
    }
    fn preferences(&mut self, ui: &mut egui::Ui) {
        Self::heading(
            ui,
            "Make yourself at home.",
            "A little less setup the next time you play.",
        );
        ui.add_space(24.);
        Self::card().show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.vertical(|ui| {
                    Self::heading(
                        ui,
                        "Start on login",
                        "Keep the companion ready when you sign in.",
                    )
                });
                if ui.checkbox(&mut self.startup, "Enabled").changed() && !self.preview {
                    let result = startup_config().and_then(|s| {
                        if self.startup {
                            s.enable()
                        } else {
                            s.disable()
                        }
                        .map_err(Into::into)
                    });
                    if let Err(e) = result {
                        self.notice = format!("Could not change startup settings: {e}");
                        self.startup = !self.startup;
                    }
                }
            });
            ui.add_space(19.);
            ui.separator();
            ui.add_space(16.);
            ui.label(RichText::new("Close the window to keep Kirk in the system tray.").size(12.));
            ui.add_space(6.);
            ui.label(
                RichText::new("Choose Quit from the tray menu to disconnect.")
                    .size(12.)
                    .color(MUTED),
            );
            ui.add_space(23.);
            ui.label(
                RichText::new("Copyright © 2026 Benja (Benjamacs1)")
                    .size(12.)
                    .color(TEXT),
            );
            ui.label(
                RichText::new(
                    "Open source under MIT. Copyright and license notices must be retained.",
                )
                .size(12.)
                .color(MUTED),
            );
            ui.add_space(14.);
            if Self::button(ui, "Open dashboard  ↗", Color::from_rgb(59, 56, 43), ACCENT).clicked()
            {
                self.dashboard();
            }
        });
    }
    fn editor(&mut self, ctx: &egui::Context, snapshot: &kirk_companion::service::Snapshot) {
        let Some(mut profile) = self.editor.take() else {
            return;
        };
        if self.editor_saving
            && snapshot
                .settings
                .profiles
                .iter()
                .any(|p| p.id == profile.id)
        {
            self.editor_saving = false;
            return;
        }
        if self.editor_saving && !snapshot.working {
            self.editor_saving = false;
        }
        let mut open = true;
        let mut cancel = false;
        let mut save = false;
        egui::Window::new("Add launch profile").open(&mut open).anchor(egui::Align2::CENTER_CENTER,Vec2::ZERO).title_bar(false).default_width(510.).resizable(false).collapsible(false).frame(Self::card()).show(ctx, |ui| {
            ui.horizontal(|ui| { ui.with_layout(egui::Layout::right_to_left(Align::Center),|ui| { if ui.button("×").clicked() { cancel = true; } ui.with_layout(egui::Layout::left_to_right(Align::Center),|ui| { Self::heading(ui,"Add launch profile","Choose the instance Kirk should start."); }); }); }); ui.add_space(20.);
            Self::input(ui, "Profile name", &mut profile.name, "My Kirk instance"); ui.add_space(8.);
            egui::ComboBox::from_label("Launch method").selected_text(profile.kind.name()).width(240.).show_ui(ui, |ui| { for kind in Launcher::ALL { ui.selectable_value(&mut profile.kind, kind, kind.name()); } });
            ui.add_space(12.); ui.label(RichText::new("Launcher executable or instance shortcut").color(MUTED).size(12.));
            ui.horizontal(|ui| {
                let text = if profile.target.as_os_str().is_empty() { "Choose a local file…".into() } else { profile.target.display().to_string() };
                ui.with_layout(egui::Layout::right_to_left(Align::Center), |ui| {
                if Self::button(ui, "Browse…", BG, ACCENT).clicked() {
                    let picker = rfd::FileDialog::new().set_title("Choose launcher or instance shortcut");
                    #[cfg(windows)] let picker = picker.add_filter("Launchers and shortcuts", &["exe", "lnk"]);
                    if let Some(path) = picker.pick_file() { profile.target = path; }
                }
                ui.add(egui::Label::new(RichText::new(text).size(12.)).truncate());
                });
            });
            if profile.kind.direct() {
                ui.add_space(12.); Self::input(ui, "Instance folder name", &mut profile.instance, "26.2(kirk)");
                egui::ComboBox::from_label("Detected instances").selected_text("Choose an instance…").width(240.).show_ui(ui, |ui| {
                    for found in &self.discovered { if ui.selectable_label(false, &found.name).clicked() { profile.instance = found.instance.clone(); profile.root = Some(found.root.clone()); profile.kind = found.kind; if profile.name.is_empty() { profile.name = found.name.clone(); } } }
                });
                ui.add_space(12.); ui.label(RichText::new("Launcher data folder (optional, for portable installs)").size(12.).color(MUTED));
                ui.horizontal(|ui| {
                    ui.add(egui::Label::new(RichText::new(profile.root.as_ref().map_or("Default launcher folder".into(), |p| p.display().to_string())).size(12.)).truncate());
                    if Self::button(ui, "Choose…", BG, ACCENT).clicked() && let Some(path) = rfd::FileDialog::new().pick_folder() { profile.root = Some(path); }
                    if ui.small_button("Clear").clicked() { profile.root = None; }
                });
            } else {
                ui.add_space(12.); ui.label(RichText::new(if profile.kind == Launcher::Open { "Opens the launcher. You still choose your instance and press Play." } else { "Choose an exact-instance shortcut created by your launcher, then test it." }).size(12.).color(MUTED));
            }
            ui.add_space(18.); ui.label(RichText::new("Use an instance with Kirk and its required Fabric version installed.").color(MUTED).size(12.));
            ui.add_space(10.);
            ui.add_enabled_ui(!snapshot.working && !self.editor_saving, |ui| { if Self::button(ui, "Save profile", ACCENT, SURFACE).clicked() { save = true; } });
        });
        if cancel {
            open = false;
        }
        if save {
            match profile.validate() {
                Ok(()) => {
                    self.send(Command::Add(profile.clone()));
                    self.editor_saving = !self.preview;
                }
                Err(e) => self.notice = e.to_string(),
            }
        }
        if open {
            self.editor = Some(profile);
        }
    }
}
impl eframe::App for Desktop {
    fn logic(&mut self, ctx: &egui::Context, _: &mut eframe::Frame) {
        ctx.request_repaint_after(Duration::from_millis(300));
        let snapshot = self
            .shared
            .read()
            .unwrap_or_else(|e| e.into_inner())
            .clone();
        if !self.preview
            && snapshot.paired
            && !snapshot.dashboard.refreshing
            && !snapshot.dashboard.acting
            && (std::time::Instant::now() >= self.next_refresh
                || snapshot.dashboard.selection != self.selection)
        {
            self.refresh_dashboard();
        }
        if !snapshot.paired {
            self.screenshot_texture = None;
            self.screenshot_stamp.clear();
        }

        if self.background {
            self.background = false;
            if self.tray.is_some() {
                ctx.send_viewport_cmd(egui::ViewportCommand::Visible(false));
            }
        }
        let wake = self.store.dir.join("wake");
        if !self.preview && wake.exists() {
            let _ = std::fs::remove_file(wake);
            ctx.send_viewport_cmd(egui::ViewportCommand::Visible(true));
            ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
        }
        while let Ok(action) = self.tray_events.try_recv() {
            match action {
                TrayAction::Show => {
                    ctx.send_viewport_cmd(egui::ViewportCommand::Visible(true));
                    ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
                }
                TrayAction::Dashboard => self.dashboard(),
                TrayAction::Quit => {
                    self.quitting = true;
                    ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                }
            }
        }
        if ctx.input(|input| input.viewport().close_requested())
            && self.tray.is_some()
            && !self.quitting
        {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            ctx.send_viewport_cmd(egui::ViewportCommand::Visible(false));
        }
    }
    fn ui(&mut self, ui: &mut egui::Ui, _: &mut eframe::Frame) {
        self.frames += 1;
        let snapshot = self
            .shared
            .read()
            .unwrap_or_else(|e| e.into_inner())
            .clone();
        let ctx = ui.ctx().clone();
        egui::Panel::top("header")
            .frame(egui::Frame::new().fill(SURFACE))
            .show(ui, |ui| self.header(ui));
        egui::Panel::left("sidebar")
            .exact_size(220.)
            .resizable(false)
            .frame(egui::Frame::new().fill(BG))
            .show(ui, |ui| self.sidebar(ui));
        egui::CentralPanel::default()
            .frame(egui::Frame::new().fill(BG).inner_margin(30))
            .show(ui, |ui| {
                egui::ScrollArea::vertical()
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        let notice = if self.notice.is_empty() {
                            &snapshot.notice
                        } else {
                            &self.notice
                        };
                        if !notice.is_empty() {
                            egui::Frame::new()
                                .fill(Color::from_rgb(57, 54, 42))
                                .corner_radius(12)
                                .inner_margin(13)
                                .show(ui, |ui| {
                                    ui.label(RichText::new(notice).size(12.).color(ACCENT));
                                });
                            ui.add_space(17.);
                        }
                        if !snapshot.dashboard.error.is_empty() {
                            egui::Frame::new()
                                .fill(Color::from_rgb(255, 237, 239))
                                .corner_radius(12)
                                .inner_margin(13)
                                .show(ui, |ui| {
                                    ui.label(
                                        RichText::new(&snapshot.dashboard.error)
                                            .size(12.)
                                            .color(Color::from_rgb(163, 65, 86)),
                                    );
                                });
                            ui.add_space(16.);
                        }
                        if self.preview {
                            ui.label(
                                RichText::new("DESIGN PREVIEW · Sample figures")
                                    .size(12.)
                                    .color(MUTED),
                            );
                            ui.add_space(12.);
                        }
                        match self.page {
                            Page::Dashboard => self.dashboard_page(ui, &snapshot),
                            Page::Macros => self.macro_page(ui, &snapshot),
                            Page::Statistics => self.statistics_page(ui, &snapshot),
                            Page::Trackers => self.trackers_page(ui, &snapshot),
                            Page::Screen => self.screen_page(ui, &snapshot),
                            Page::Assistant => self.assistant_page(ui, &snapshot),
                            Page::Connection => self.connection(ui, &snapshot),
                            Page::Profiles => self.profiles(ui, &snapshot),
                            Page::Preferences => self.preferences(ui),
                        }
                    });
            });
        self.editor(&ctx, &snapshot);
        if self.preview && self.frames == 8 && self.screenshot.is_some() {
            ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot(egui::UserData::default()));
        }
        let images = ctx.input(|input| {
            input
                .events
                .iter()
                .filter_map(|event| {
                    if let egui::Event::Screenshot { image, .. } = event {
                        Some(image.clone())
                    } else {
                        None
                    }
                })
                .collect::<Vec<_>>()
        });
        for image in images {
            if let Some(path) = self.screenshot.take() {
                if let Some(parent) = path.parent() {
                    let _ = std::fs::create_dir_all(parent);
                }
                let bytes: Vec<u8> = image.pixels.iter().flat_map(|p| p.to_array()).collect();
                if let Err(e) = image::save_buffer(
                    path,
                    &bytes,
                    image.width() as u32,
                    image.height() as u32,
                    image::ColorType::Rgba8,
                ) {
                    eprintln!("Could not save preview: {e}");
                }
                self.quitting = true;
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
        }
    }
    fn on_exit(&mut self) {
        let _ = self.sender.send(Command::Quit);
        if let Some(runtime) = self.runtime.take() {
            runtime.shutdown_timeout(Duration::from_secs(2));
        }
    }
}

fn startup_config() -> anyhow::Result<auto_launch::AutoLaunch> {
    let exe = std::env::current_exe()?;
    Ok(auto_launch::AutoLaunchBuilder::new()
        .set_app_name("Kirk Companion")
        .set_app_path(&exe.to_string_lossy())
        .set_args(&["--background"])
        .build()?)
}
fn create_tray(ctx: egui::Context, sender: mpsc::Sender<TrayAction>) -> anyhow::Result<TrayIcon> {
    let menu = Menu::new();
    let show = MenuItem::new("Open Kirk Companion", true, None);
    let dashboard = MenuItem::new("Open dashboard", true, None);
    let quit = MenuItem::new("Quit", true, None);
    menu.append_items(&[&show, &dashboard, &PredefinedMenuItem::separator(), &quit])?;
    let show_id = show.id().clone();
    let dashboard_id = dashboard.id().clone();
    let quit_id = quit.id().clone();
    let repaint = ctx.clone();
    let actions = sender.clone();
    MenuEvent::set_event_handler(Some(move |event: MenuEvent| {
        let action = if event.id == show_id {
            Some(TrayAction::Show)
        } else if event.id == dashboard_id {
            Some(TrayAction::Dashboard)
        } else if event.id == quit_id {
            Some(TrayAction::Quit)
        } else {
            None
        };
        if let Some(action) = action {
            let _ = actions.send(action);
            repaint.request_repaint();
        }
    }));
    TrayIconEvent::set_event_handler(Some(move |event| {
        if matches!(
            event,
            TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            }
        ) {
            let _ = sender.send(TrayAction::Show);
            ctx.request_repaint();
        }
    }));
    let icon = image::load_from_memory(include_bytes!("ui/icon.png"))?.into_rgba8();
    let (width, height) = icon.dimensions();
    Ok(TrayIconBuilder::new()
        .with_menu(Box::new(menu))
        .with_tooltip("Kirk Companion")
        .with_icon(tray_icon::Icon::from_rgba(icon.into_raw(), width, height)?)
        .build()?)
}

include!("dashboard_ui.rs");
