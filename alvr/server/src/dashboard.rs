use crate::{
    graphics_info,
    settings_editor::SettingsEditor,
    theme::{self, Palette},
    ClientListAction, FILESYSTEM_LAYOUT, SESSION_MANAGER,
};
use alvr_common::{prelude::*, ALVR_VERSION};
use alvr_session::{EventSeverity, Raw, ServerEvent, Statistics};
use eframe::egui::{
    self, Align, Align2, Color32, FontFamily, FontId, Frame, Layout, Margin, RichText, Sense,
    Stroke, Vec2,
};
use parking_lot::Mutex;
use serde_json::{json, Value};
use settings_schema::SchemaNode;
use std::{
    collections::VecDeque,
    net::Ipv4Addr,
    path::PathBuf,
    thread,
    time::{Duration, Instant},
};
use tokio::sync::broadcast;

const WINDOW_WIDTH: f32 = 800.0;
const WINDOW_HEIGHT: f32 = 600.0;
const LOG_CAPACITY: usize = 500;
const HISTORY_CAPACITY: usize = 600;
const STATISTICS_TIMEOUT: Duration = Duration::from_secs(2);
const TOAST_DURATION: Duration = Duration::from_secs(5);

// Used by shutdown_runtime() to close the window from another thread.
static CONTEXT: Mutex<Option<egui::Context>> = Mutex::new(None);

pub fn close_window() {
    if let Some(context) = CONTEXT.lock().take() {
        context.send_viewport_cmd(egui::ViewportCommand::Close);
        context.request_repaint();
    }
}

#[derive(PartialEq, Clone, Copy)]
enum Tab {
    Monitor,
    Settings,
    Installation,
    About,
}

#[derive(Clone, Copy, PartialEq)]
enum ToastKind {
    Error,
    Warning,
    Info,
    Success,
    Default,
}

struct Toast {
    text: String,
    kind: ToastKind,
    until: Instant,
}

struct SettingsState {
    schema: SchemaNode,
    draft: Value,
    editor: SettingsEditor,
    dirty: bool,
}

impl SettingsState {
    fn load() -> Self {
        Self {
            schema: alvr_session::settings_schema(alvr_session::session_settings_default()),
            draft: Self::stored(),
            editor: SettingsEditor::new(),
            dirty: false,
        }
    }

    fn stored() -> Value {
        serde_json::to_value(SESSION_MANAGER.lock().get())
            .ok()
            .and_then(|session| session.get("sessionSettings").cloned())
            .unwrap_or(Value::Null)
    }

    /// Settings are stored as soon as they change, like the web dashboard did.
    fn apply(&mut self) -> Option<String> {
        self.dirty = false;
        let res = SESSION_MANAGER
            .lock()
            .get_mut()
            .merge_from_json(&json!({ "sessionSettings": self.draft }));

        match res {
            Ok(()) => None,
            Err(e) => {
                // show what was actually stored (invalid values may have been corrected)
                self.draft = Self::stored();
                Some(format!("Settings could not be fully stored: {e}"))
            }
        }
    }

    fn notification_levels(&self) -> &'static [EventLevel] {
        match self
            .draft
            .pointer("/extra/notificationLevel/variant")
            .and_then(Value::as_str)
        {
            Some("error") => &[EventLevel::Error],
            Some("warning") => &[EventLevel::Error, EventLevel::Warning],
            Some("info") => &[EventLevel::Error, EventLevel::Warning, EventLevel::Info],
            Some("debug") => &[
                EventLevel::Error,
                EventLevel::Warning,
                EventLevel::Info,
                EventLevel::Debug,
            ],
            _ => &[],
        }
    }

    fn flag(&self, pointer: &str) -> bool {
        self.draft
            .pointer(pointer)
            .and_then(Value::as_bool)
            .unwrap_or(false)
    }
}

#[derive(PartialEq, Clone, Copy)]
enum EventLevel {
    Error,
    Warning,
    Info,
    Debug,
}

impl EventLevel {
    fn of(severity: &EventSeverity) -> Self {
        match severity {
            EventSeverity::Error => Self::Error,
            EventSeverity::Warning => Self::Warning,
            EventSeverity::Info => Self::Info,
            EventSeverity::Debug => Self::Debug,
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::Error => "[ERROR]",
            Self::Warning => "[WARN]",
            Self::Info => "[INFO]",
            Self::Debug => "[DEBUG]",
        }
    }
}

struct AddClientForm {
    device_name: String,
    hostname: String,
    ip: String,
}

struct DashboardApp {
    tab: Tab,
    monitor_tab: usize,
    events: broadcast::Receiver<String>,
    palette: Palette,
    logs: VecDeque<Raw>,
    stats: Statistics,
    history: VecDeque<Statistics>,
    last_statistics: Option<Instant>,
    toasts: Vec<Toast>,

    settings: SettingsState,

    add_client: Option<AddClientForm>,
    configure_client: Option<(String, String)>, // hostname, new ip text
    confirm_restart: bool,
    drivers: Vec<PathBuf>,
    gpu_names: Vec<String>,
}

impl DashboardApp {
    fn new(cc: &eframe::CreationContext<'_>, events: broadcast::Receiver<String>) -> Self {
        *CONTEXT.lock() = Some(cc.egui_ctx.clone());

        let settings = SettingsState::load();
        let palette = Palette::darkly();
        theme::apply(&cc.egui_ctx, &palette);

        Self {
            tab: Tab::Monitor,
            monitor_tab: 0,
            events,
            palette,
            logs: VecDeque::new(),
            stats: default_statistics(),
            history: VecDeque::new(),
            last_statistics: None,
            toasts: Vec::new(),
            settings,
            add_client: None,
            configure_client: None,
            confirm_restart: false,
            drivers: alvr_commands::get_registered_drivers().unwrap_or_default(),
            gpu_names: graphics_info::get_gpu_names(),
        }
    }

    fn toast(&mut self, kind: ToastKind, text: impl Into<String>) {
        self.toasts.push(Toast {
            text: text.into(),
            kind,
            until: Instant::now() + TOAST_DURATION,
        });
    }

    fn poll_events(&mut self) {
        loop {
            match self.events.try_recv() {
                Ok(line) => {
                    let Ok(event) = serde_json::from_str::<ServerEvent>(&line) else {
                        continue;
                    };
                    match event {
                        ServerEvent::Raw(raw) => {
                            let level = EventLevel::of(&raw.severity);
                            if self.settings.notification_levels().contains(&level)
                                && self.toasts.len() < 2
                            {
                                let kind = match level {
                                    EventLevel::Error => ToastKind::Error,
                                    EventLevel::Warning => ToastKind::Warning,
                                    EventLevel::Info => ToastKind::Info,
                                    EventLevel::Debug => ToastKind::Default,
                                };
                                self.toast(kind, raw.content.clone());
                            }
                            if self.logs.len() >= LOG_CAPACITY {
                                self.logs.pop_front();
                            }
                            self.logs.push_back(raw);
                        }
                        ServerEvent::Statistics(stats) => {
                            if self.last_statistics.is_none() {
                                // a client started streaming: show the statistics
                                self.monitor_tab = 1;
                                self.history.clear();
                            }
                            self.last_statistics = Some(Instant::now());
                            if self.history.len() >= HISTORY_CAPACITY {
                                self.history.pop_front();
                            }
                            self.history.push_back(stats.clone());
                            self.stats = stats;
                        }
                        ServerEvent::ClientFoundWrongVersion(version) => {
                            self.toast(
                                ToastKind::Warning,
                                format!(
                                    "A client with an incompatible version ({version}) was found"
                                ),
                            );
                        }
                        ServerEvent::SessionSettingsExtrapolationFailed => {
                            self.toast(
                                ToastKind::Warning,
                                "Some settings could not be migrated and were reset",
                            );
                        }
                        _ => (),
                    }
                }
                Err(broadcast::error::TryRecvError::Lagged(_)) => continue,
                Err(_) => break,
            }
        }

        if let Some(last) = self.last_statistics {
            if last.elapsed() > STATISTICS_TIMEOUT {
                // the client went away: back to the connection page
                self.last_statistics = None;
                self.monitor_tab = 0;
            }
        }

        self.toasts.retain(|toast| toast.until > Instant::now());
    }

    fn sidebar(&mut self, ui: &mut egui::Ui) {
        let p = self.palette;
        egui::Panel::left("sidebar")
            .exact_size(180.0)
            .resizable(false)
            .show_separator_line(false)
            .frame(Frame::NONE.fill(p.sidebar_bg))
            .show(ui, |ui| {
                ui.spacing_mut().item_spacing = Vec2::ZERO;
                ui.add_space(0.0);
                for (tab, icon, label) in [
                    (Tab::Monitor, "\u{23F1}", "Monitor"),
                    (Tab::Settings, "\u{2699}", "Settings"),
                    (Tab::Installation, "\u{1F4BE}", "Installation"),
                    (Tab::About, "\u{2139}", "About"),
                ] {
                    if theme::sidebar_item(ui, &p, icon, label).clicked() {
                        self.tab = tab;
                    }
                }

                let remaining = ui.available_height();
                ui.add_space((remaining - 36.0).max(0.0));
                if theme::sidebar_item(ui, &p, "\u{27F3}", "Restart SteamVR").clicked() {
                    if self.settings.flag("/extra/restartConfirmDialog/enabled")
                        || self.settings.flag("/extra/restartConfirmDialog")
                    {
                        self.confirm_restart = true;
                    } else {
                        crate::notify_restart_driver();
                    }
                }
            });
    }

    fn monitor_ui(&mut self, ui: &mut egui::Ui) {
        let p = self.palette;
        if let Some(index) = theme::nav_tabs(
            ui,
            &p,
            &["Connection", "Statistics", "Logging"],
            self.monitor_tab,
        ) {
            self.monitor_tab = index;
        }
        ui.add_space(20.0);

        egui::ScrollArea::vertical()
            .id_salt("monitor_scroll")
            .auto_shrink([false, false])
            .show(ui, |ui| {
                ui.set_max_width(ui.available_width() - 8.0);
                match self.monitor_tab {
                    0 => self.connection_ui(ui),
                    1 => self.statistics_ui(ui),
                    _ => self.logging_ui(ui),
                }
            });
    }

    fn connection_ui(&mut self, ui: &mut egui::Ui) {
        let p = self.palette;
        let clients = SESSION_MANAGER.lock().get().client_connections.clone();
        let mut hostnames = clients.keys().cloned().collect::<Vec<_>>();
        hostnames.sort();

        theme::card(ui, &p, "New Clients", |ui| {
            for hostname in &hostnames {
                let client = &clients[hostname];
                if client.trusted {
                    continue;
                }
                ui.horizontal(|ui| {
                    ui.label(format!("{} ({hostname})", client.display_name));
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if theme::small_button(ui, "Trust").clicked() {
                            crate::update_client_list(
                                hostname.clone(),
                                ClientListAction::TrustAndMaybeAddIp(None),
                            );
                        }
                    });
                });
            }
        });
        ui.add_space(20.0);

        theme::card(ui, &p, "Trusted Clients", |ui| {
            for hostname in &hostnames {
                let client = &clients[hostname];
                if !client.trusted {
                    continue;
                }
                ui.horizontal(|ui| {
                    ui.label(format!("{} ({hostname})", client.display_name));
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if theme::small_button(ui, "Remove").clicked() {
                            crate::update_client_list(
                                hostname.clone(),
                                ClientListAction::RemoveIpOrEntry(None),
                            );
                        }
                        if theme::small_button(ui, "Configure").clicked() {
                            self.configure_client = Some((hostname.clone(), String::new()));
                        }
                    });
                });
            }
        });

        ui.add_space(10.0);
        if ui.button("Add client manually").clicked() {
            self.add_client = Some(AddClientForm {
                device_name: String::new(),
                hostname: String::new(),
                ip: String::new(),
            });
        }

        ui.add_space(20.0);
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing.x = 0.0;
            ui.label("Is your headset not showing up? Visit our troubleshooting page ");
            ui.hyperlink_to(
                "here",
                "https://github.com/alvr-org/ALVR/wiki/Troubleshooting#alvr-cant-see-my-headset",
            );
        });
    }

    fn statistics_ui(&mut self, ui: &mut egui::Ui) {
        let p = self.palette;

        ui.horizontal(|ui| {
            ui.heading(RichText::new("Latency").size(18.0));
            theme::badge(ui, "Network", rgb(0x7f, 0x7f, 0x7f));
            theme::badge(ui, "Render", rgb(0xd6, 0x27, 0x28));
            theme::badge(ui, "Idle", rgb(0xff, 0x7f, 0x0e));
            theme::badge(ui, "Transcode", rgb(0x1f, 0x77, 0xb4));
        });
        let network = self
            .history
            .iter()
            .map(|s| s.transport_latency)
            .collect::<Vec<_>>();
        let render = vec![0.0; self.history.len()];
        let transcode = self
            .history
            .iter()
            .map(|s| s.encode_latency + s.decode_latency)
            .collect::<Vec<_>>();
        let idle = self
            .history
            .iter()
            .zip(network.iter().zip(&transcode))
            .map(|(s, (n, t))| (s.total_latency - n - t).max(0.0))
            .collect::<Vec<_>>();
        draw_graph(
            ui,
            &p,
            160.0,
            true,
            &[
                (rgb(0x7f, 0x7f, 0x7f), network),
                (rgb(0xd6, 0x27, 0x28), render),
                (rgb(0xff, 0x7f, 0x0e), idle),
                (rgb(0x1f, 0x77, 0xb4), transcode),
            ],
        );

        ui.add_space(12.0);
        ui.horizontal(|ui| {
            ui.heading(RichText::new("Framerate").size(18.0));
            theme::badge(ui, "Server FPS", rgb(0x1f, 0x77, 0xb4));
            theme::badge(ui, "Client FPS", rgb(0xff, 0x7f, 0x0e));
        });
        draw_graph(
            ui,
            &p,
            100.0,
            false,
            &[
                (
                    rgb(0x1f, 0x77, 0xb4),
                    self.history.iter().map(|s| s.server_f_p_s as f32).collect(),
                ),
                (
                    rgb(0xff, 0x7f, 0x0e),
                    self.history.iter().map(|s| s.client_f_p_s as f32).collect(),
                ),
            ],
        );

        ui.add_space(12.0);
        let s = &self.stats;
        let rows: [(&str, String, Option<String>); 12] = [
            (
                "Total Packets",
                format!("{} Packets", s.total_packets),
                Some(format!("{} Packets / s", s.packet_rate)),
            ),
            (
                "Total packets lost",
                format!("{} Packets", s.packets_lost_total),
                Some(format!("{} Packets / s", s.packets_lost_per_second)),
            ),
            (
                "Total sent",
                format!("{} MB", s.total_sent),
                Some(format!("{} Mbps", s.sent_rate)),
            ),
            ("Total latency", format!("{} ms", s.total_latency), None),
            ("Encoder Latency", format!("{} ms", s.encode_latency), None),
            (
                "Transport latency",
                format!("{} ms", s.transport_latency),
                None,
            ),
            ("Decoder latency", format!("{} ms", s.decode_latency), None),
            ("Fec percentage", format!("{} %", s.fec_percentage), None),
            (
                "Fec failure total",
                format!("{} Packets", s.fec_failure_total),
                Some(format!("{} Packets / s", s.fec_failure_in_second)),
            ),
            ("Client FPS", format!("{} fps", s.client_f_p_s), None),
            ("Server FPS", format!("{} fps", s.server_f_p_s), None),
            (
                "Encode latency max",
                format!("{} ms", s.encode_latency_max),
                None,
            ),
        ];
        theme::card_body(ui, &p, |ui| {
            egui::Grid::new("statistics_table")
                .num_columns(3)
                .min_col_width(ui.available_width() / 3.0 - 8.0)
                .show(ui, |ui| {
                    for (name, value, rate) in rows {
                        ui.label(format!("{name}:"));
                        ui.label(value);
                        ui.label(rate.unwrap_or_default());
                        ui.end_row();
                    }
                });
        });
    }

    fn logging_ui(&mut self, ui: &mut egui::Ui) {
        let p = self.palette;
        let font = FontId::new(12.0, FontFamily::Proportional);
        let width = ui.available_width();

        for (index, raw) in self.logs.iter().enumerate() {
            let level = EventLevel::of(&raw.severity);
            let color = match level {
                EventLevel::Error => rgb(0xd0, 0x30, 0x30),
                EventLevel::Warning => rgb(0xc0, 0x80, 0x00),
                EventLevel::Info => p.text,
                EventLevel::Debug => p.muted,
            };
            let fill = if index % 2 == 1 {
                p.table_stripe
            } else {
                Color32::TRANSPARENT
            };

            // like the web table: fixed time and level columns, the message wraps
            Frame::new()
                .fill(fill)
                .inner_margin(Margin::symmetric(4, 1))
                .show(ui, |ui| {
                    ui.set_width(width - 8.0);
                    ui.horizontal_top(|ui| {
                        ui.spacing_mut().item_spacing.x = 0.0;
                        ui.add_sized(
                            Vec2::new(120.0, 16.0),
                            egui::Label::new(
                                RichText::new(&raw.timestamp)
                                    .font(font.clone())
                                    .color(p.text),
                            ),
                        );
                        ui.add_sized(
                            Vec2::new(60.0, 16.0),
                            egui::Label::new(
                                RichText::new(level.label()).font(font.clone()).color(color),
                            ),
                        );
                        ui.add(
                            egui::Label::new(
                                RichText::new(raw.content.trim())
                                    .font(font.clone())
                                    .color(p.text),
                            )
                            .wrap(),
                        );
                    });
                });
        }
    }

    fn installation_ui(&mut self, ui: &mut egui::Ui) {
        let p = self.palette;
        ui.add_space(20.0);
        ui.horizontal(|ui| {
            if ui.button("Add firewall rules").clicked() {
                self.set_firewall_rules(true);
            }
            if ui.button("Remove firewall rules").clicked() {
                self.set_firewall_rules(false);
            }
        });

        ui.add_space(40.0);
        let header = if self.drivers.is_empty() {
            "No Drivers registered"
        } else {
            "Registered Drivers"
        };
        let mut unregister = None;
        theme::card(ui, &p, header, |ui| {
            for path in &self.drivers {
                ui.horizontal(|ui| {
                    ui.label(path.to_string_lossy());
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if ui
                            .add_sized(Vec2::new(80.0, 23.0), egui::Button::new("Remove"))
                            .clicked()
                        {
                            unregister = Some(path.clone());
                        }
                    });
                });
            }
        });
        if let Some(path) = unregister {
            match alvr_commands::driver_registration(&[path], false) {
                Ok(()) => self.toast(ToastKind::Success, "Driver successfully unregistred"),
                Err(_) => self.toast(ToastKind::Error, "Unregistering driver failed"),
            }
            self.drivers = alvr_commands::get_registered_drivers().unwrap_or_default();
        }

        ui.add_space(10.0);
        if ui.button("Register ALVR driver").clicked() {
            let res = alvr_commands::driver_registration(
                &[FILESYSTEM_LAYOUT.openvr_driver_root_dir.clone()],
                true,
            );
            match res {
                Ok(()) => self.toast(ToastKind::Success, "Driver successfully registered"),
                Err(e) => self.toast(ToastKind::Error, format!("Registering driver failed: {e}")),
            }
            self.drivers = alvr_commands::get_registered_drivers().unwrap_or_default();
        }

        ui.add_space(10.0);
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing.x = 0.0;
            ui.label(
                "IMPORTANT: Registering the driver is generally discouraged. Now the driver \
                 registration is needed only when you want to launch SteamVR by itself or when \
                 you want to use ALVR together with other SteamVR drivers, like ",
            );
            ui.hyperlink_to("Driver4VR", "https://www.driver4vr.com/");
            ui.label(" for full body tracking.");
        });
    }

    fn set_firewall_rules(&self, add: bool) {
        // The elevation prompt blocks until the user answers: keep the UI responsive. The result
        // is reported through the log, which feeds the notifications.
        thread::spawn(move || match alvr_commands::firewall_rules(add) {
            Ok(()) => info!("Firewall rules successfully set"),
            Err(1) => warn!("Firewall rule is already set"),
            Err(126) => warn!("Firewall request dismissed"),
            Err(code) => error!("Setting firewall rules failed: code {code}"),
        });
    }

    fn about_ui(&mut self, ui: &mut egui::Ui) {
        ui.add_space(16.0);
        ui.heading(RichText::new(format!("ALVR server {}", *ALVR_VERSION)).size(28.0));
        ui.add_space(8.0);
        ui.label("Stream VR games from your PC to your headset via Wi-Fi.");
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing.x = 0.0;
            ui.label("ALVR uses technologies like ");
            ui.hyperlink_to(
                "Asynchronous TimeWarp (ATW)",
                "https://developer.oculus.com/documentation/native/android/mobile-timewarp-overview/",
            );
            ui.label(" and ");
            ui.hyperlink_to(
                "Fixed Foveated Rendering (FFR)",
                "https://developer.oculus.com/documentation/native/android/mobile-ffr/",
            );
            ui.label(" for a smoother experience.");
        });
        ui.label("All games that work with an Oculus Rift(s) should work with ALVR.");
        ui.label("This is a fork of ALVR that works with Oculus Quest and Go.");
        ui.add_space(14.0);
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing.x = 0.0;
            ui.label("Visit us on GitHub ");
            ui.hyperlink("https://github.com/alvr-org/ALVR/");
        });
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing.x = 0.0;
            ui.label("Join us on Discord: ");
            ui.hyperlink("https://discord.gg/ALVR");
        });
        ui.add_space(14.0);
        if !self.gpu_names.is_empty() {
            ui.label(format!("Graphics devices: {}", self.gpu_names.join(", ")));
            ui.add_space(14.0);
        }
    }

    fn modals(&mut self, ctx: &egui::Context) {
        let p = self.palette;

        if let Some(form) = &mut self.add_client {
            let mut open = true;
            let mut submit = false;
            egui::Window::new("Add client")
                .open(&mut open)
                .collapsible(false)
                .resizable(false)
                .anchor(Align2::CENTER_CENTER, [0.0, 0.0])
                .show(ctx, |ui| {
                    ui.set_width(380.0);
                    form_field(
                        ui,
                        &p,
                        "Device name",
                        &mut form.device_name,
                        "Give your headset a name",
                    );
                    form_field(
                        ui,
                        &p,
                        "Hostname",
                        &mut form.hostname,
                        "As displayed when opening the ALVR client on your device",
                    );
                    form_field(
                        ui,
                        &p,
                        "IP address",
                        &mut form.ip,
                        "Enter the remote IP address of the headset",
                    );
                    ui.add_space(6.0);
                    if ui.button("Add client").clicked() {
                        submit = true;
                    }
                });

            if submit {
                let hostname = form.hostname.trim().to_owned();
                let device_name = form.device_name.trim().to_owned();
                let duplicated = SESSION_MANAGER
                    .lock()
                    .get()
                    .client_connections
                    .contains_key(&hostname);
                if duplicated || hostname.is_empty() {
                    self.toast(
                        ToastKind::Error,
                        "A device with this hostname is already registered",
                    );
                } else if let Ok(ip) = form.ip.trim().parse::<Ipv4Addr>() {
                    crate::update_client_list(
                        hostname.clone(),
                        ClientListAction::AddIfMissing {
                            display_name: device_name,
                        },
                    );
                    crate::update_client_list(
                        hostname,
                        ClientListAction::TrustAndMaybeAddIp(Some(ip.into())),
                    );
                    open = false;
                } else {
                    self.toast(ToastKind::Error, "Not a valid IPv4 formatted address");
                }
            }
            if !open {
                self.add_client = None;
            }
        }

        if let Some((hostname, new_ip)) = &mut self.configure_client {
            let mut open = true;
            let mut add_ip = false;
            let mut remove_ip = None;
            let known_ips = SESSION_MANAGER
                .lock()
                .get()
                .client_connections
                .get(hostname.as_str())
                .map(|client| {
                    let mut ips = client.manual_ips.iter().cloned().collect::<Vec<_>>();
                    ips.sort();
                    ips
                })
                .unwrap_or_default();

            egui::Window::new("Configure client")
                .open(&mut open)
                .collapsible(false)
                .resizable(false)
                .anchor(Align2::CENTER_CENTER, [0.0, 0.0])
                .show(ctx, |ui| {
                    ui.set_width(380.0);
                    for ip in &known_ips {
                        ui.horizontal(|ui| {
                            ui.label(ip.to_string());
                            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                if ui.small_button("Remove").clicked() {
                                    remove_ip = Some(*ip);
                                }
                            });
                        });
                    }
                    ui.add_space(6.0);
                    ui.horizontal(|ui| {
                        theme::text_input(ui, &p, new_ip, 200.0);
                        if ui.button("Add new IP").clicked() {
                            add_ip = true;
                        }
                    });
                });

            let hostname = hostname.clone();
            let typed = new_ip.trim().to_owned();
            if let Some(ip) = remove_ip {
                crate::update_client_list(
                    hostname.clone(),
                    ClientListAction::RemoveIpOrEntry(Some(ip)),
                );
            }
            if add_ip {
                match typed.parse::<Ipv4Addr>() {
                    Ok(ip) if known_ips.contains(&ip.into()) => self.toast(
                        ToastKind::Error,
                        "This IP address is already registed on this device",
                    ),
                    Ok(ip) => {
                        crate::update_client_list(
                            hostname,
                            ClientListAction::TrustAndMaybeAddIp(Some(ip.into())),
                        );
                        if let Some((_, text)) = &mut self.configure_client {
                            text.clear();
                        }
                    }
                    Err(_) => self.toast(ToastKind::Error, "Not a valid IPv4 formatted address"),
                }
            }
            if !open {
                self.configure_client = None;
            }
        }

        if self.confirm_restart {
            egui::Window::new("Restart SteamVR")
                .collapsible(false)
                .resizable(false)
                .anchor(Align2::CENTER_CENTER, [0.0, 0.0])
                .show(ctx, |ui| {
                    ui.label("Are you sure you want to restart SteamVR?");
                    ui.add_space(6.0);
                    ui.horizontal(|ui| {
                        if ui.button("Restart").clicked() {
                            self.confirm_restart = false;
                            crate::notify_restart_driver();
                        }
                        if ui.button("Cancel").clicked() {
                            self.confirm_restart = false;
                        }
                    });
                });
        }
    }

    fn toasts_ui(&self, ctx: &egui::Context) {
        #[allow(deprecated)]
        let area = egui::Area::new(egui::Id::new("toasts"));
        area.anchor(Align2::LEFT_BOTTOM, [10.0, -10.0])
            .interactable(false)
            .show(ctx, |ui| {
                for toast in &self.toasts {
                    let (accent, fill) = match toast.kind {
                        ToastKind::Error => (rgb(0xd9, 0x53, 0x4f), rgb(0x5a, 0x2a, 0x29)),
                        ToastKind::Warning => (rgb(0xf0, 0xad, 0x4e), rgb(0x5a, 0x45, 0x25)),
                        ToastKind::Info => (rgb(0x5b, 0xc0, 0xde), rgb(0x24, 0x4a, 0x57)),
                        ToastKind::Success => (rgb(0x5c, 0xb8, 0x5c), rgb(0x2a, 0x4d, 0x2a)),
                        ToastKind::Default => (rgb(0x99, 0x99, 0x99), rgb(0x44, 0x44, 0x44)),
                    };
                    Frame::new()
                        .fill(fill)
                        .stroke(Stroke::new(1.0, accent))
                        .corner_radius(egui::CornerRadius::same(10))
                        .inner_margin(Margin::symmetric(14, 8))
                        .show(ui, |ui| {
                            ui.set_max_width(320.0);
                            ui.label(RichText::new(&toast.text).color(Color32::WHITE).size(13.0));
                        });
                    ui.add_space(6.0);
                }
            });
    }
}

fn form_field(ui: &mut egui::Ui, p: &Palette, label: &str, text: &mut String, help: &str) {
    ui.label(label);
    theme::text_input(ui, p, text, 360.0);
    ui.label(RichText::new(help).size(12.0).color(p.muted));
    ui.add_space(4.0);
}

const fn rgb(r: u8, g: u8, b: u8) -> Color32 {
    Color32::from_rgb(r, g, b)
}

fn default_statistics() -> Statistics {
    Statistics {
        total_packets: 0,
        packet_rate: 0,
        packets_lost_total: 0,
        packets_lost_per_second: 0,
        total_sent: 0,
        sent_rate: 0.0,
        total_latency: 0.0,
        encode_latency: 0.0,
        encode_latency_max: 0.0,
        transport_latency: 0.0,
        decode_latency: 0.0,
        fec_percentage: 0,
        fec_failure_total: 0,
        fec_failure_in_second: 0,
        client_f_p_s: 0,
        server_f_p_s: 0,
    }
}

/// Graphs shown in the statistics page. When `stacked`, each series is drawn on top of the
/// previous ones as a filled band; otherwise each series is a line.
fn draw_graph(
    ui: &mut egui::Ui,
    p: &Palette,
    height: f32,
    stacked: bool,
    series: &[(Color32, Vec<f32>)],
) {
    let (rect, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), height), Sense::hover());
    let painter = ui.painter_at(rect);
    let grid_color = p.tab_border;
    let font = FontId::new(11.0, FontFamily::Proportional);

    let plot = egui::Rect::from_min_max(
        egui::pos2(rect.left() + 38.0, rect.top() + 4.0),
        egui::pos2(rect.right() - 4.0, rect.bottom() - 4.0),
    );

    let samples = series.iter().map(|(_, v)| v.len()).max().unwrap_or(0);
    let totals = (0..samples)
        .map(|i| {
            if stacked {
                series
                    .iter()
                    .map(|(_, v)| v.get(i).copied().unwrap_or(0.0))
                    .sum::<f32>()
            } else {
                series
                    .iter()
                    .map(|(_, v)| v.get(i).copied().unwrap_or(0.0))
                    .fold(0.0, f32::max)
            }
        })
        .collect::<Vec<_>>();
    let peak = totals.iter().cloned().fold(0.0, f32::max).max(1.0);
    let top = nice_ceiling(peak);

    for step in 0..=4 {
        let fraction = step as f32 / 4.0;
        let y = plot.bottom() - plot.height() * fraction;
        painter.hline(plot.x_range(), y, Stroke::new(1.0, grid_color));
        painter.text(
            egui::pos2(plot.left() - 6.0, y),
            Align2::RIGHT_CENTER,
            format!("{}", (top * fraction).round()),
            font.clone(),
            p.text,
        );
    }

    if samples < 2 {
        return;
    }
    let x_of = |i: usize| plot.left() + plot.width() * i as f32 / (samples - 1) as f32;
    let y_of = |v: f32| plot.bottom() - plot.height() * (v / top).clamp(0.0, 1.0);

    if stacked {
        let mut lower = vec![0.0f32; samples];
        for (color, values) in series {
            let upper = (0..samples)
                .map(|i| lower[i] + values.get(i).copied().unwrap_or(0.0))
                .collect::<Vec<_>>();
            for i in 0..samples - 1 {
                painter.add(egui::Shape::convex_polygon(
                    vec![
                        egui::pos2(x_of(i), y_of(lower[i])),
                        egui::pos2(x_of(i + 1), y_of(lower[i + 1])),
                        egui::pos2(x_of(i + 1), y_of(upper[i + 1])),
                        egui::pos2(x_of(i), y_of(upper[i])),
                    ],
                    color.gamma_multiply(0.85),
                    Stroke::NONE,
                ));
            }
            lower = upper;
        }
    } else {
        for (color, values) in series {
            let points = (0..samples)
                .map(|i| egui::pos2(x_of(i), y_of(values.get(i).copied().unwrap_or(0.0))))
                .collect::<Vec<_>>();
            painter.add(egui::Shape::line(points, Stroke::new(1.5, *color)));
        }
    }
}

fn nice_ceiling(value: f32) -> f32 {
    let magnitude = 10f32.powf(value.log10().floor());
    let normalized = value / magnitude;
    let step = if normalized <= 1.0 {
        1.0
    } else if normalized <= 2.0 {
        2.0
    } else if normalized <= 5.0 {
        5.0
    } else {
        10.0
    };
    step * magnitude
}

impl eframe::App for DashboardApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.poll_events();

        let p = self.palette;

        self.sidebar(ui);

        egui::CentralPanel::default()
            .frame(Frame::NONE.fill(p.bg).inner_margin(Margin {
                left: 30,
                right: 15,
                top: 0,
                bottom: 0,
            }))
            .show(ui, |ui| match self.tab {
                Tab::Monitor => self.monitor_ui(ui),
                Tab::Settings => {
                    let settings = &mut self.settings;
                    if settings
                        .editor
                        .ui(ui, &p, &settings.schema, &mut settings.draft)
                    {
                        settings.dirty = true;
                    }
                }
                Tab::Installation => self.installation_ui(ui),
                Tab::About => self.about_ui(ui),
            });

        // store once the user is done dragging a slider
        if self.settings.dirty && !ui.input(|i| i.pointer.any_down()) {
            if let Some(message) = self.settings.apply() {
                self.toast(ToastKind::Warning, message);
            }
        }

        self.modals(ui.ctx());
        self.toasts_ui(ui.ctx());

        // log and statistics events arrive from other threads
        ui.ctx().request_repaint_after(Duration::from_millis(100));
    }
}

fn event_loop_hook() -> eframe::EventLoopBuilderHook {
    // The window lives on a thread owned by SteamVR's process, not on the main thread.
    Box::new(|builder| {
        #[cfg(windows)]
        {
            use winit::platform::windows::EventLoopBuilderExtWindows;
            builder.with_any_thread(true);
        }
        #[cfg(all(unix, not(target_os = "macos")))]
        {
            use winit::platform::{
                wayland::EventLoopBuilderExtWayland, x11::EventLoopBuilderExtX11,
            };
            EventLoopBuilderExtX11::with_any_thread(builder, true);
            EventLoopBuilderExtWayland::with_any_thread(builder, true);
        }
        #[cfg(not(any(windows, all(unix, not(target_os = "macos")))))]
        let _ = builder;
    })
}

// this thread gets interrupted when SteamVR closes
// todo: handle this in a better way
pub fn ui_thread(events_sender: broadcast::Sender<String>) -> StrResult {
    let events = events_sender.subscribe();

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("ALVR dashboard")
            .with_inner_size([WINDOW_WIDTH, WINDOW_HEIGHT])
            .with_min_inner_size([600.0, 400.0]),
        centered: true,
        event_loop_builder: Some(event_loop_hook()),
        ..Default::default()
    };

    trace_err!(eframe::run_native(
        "ALVR dashboard",
        options,
        Box::new(move |cc| Ok(Box::new(DashboardApp::new(cc, events)))),
    ))?;

    // the window was closed: prevent a second close request
    *CONTEXT.lock() = None;
    crate::shutdown_runtime();

    unsafe { crate::ShutdownSteamvr() };

    Ok(())
}
