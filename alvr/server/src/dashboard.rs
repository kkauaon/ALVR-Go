use crate::{graphics_info, settings_editor, ClientListAction, FILESYSTEM_LAYOUT, SESSION_MANAGER};
use alvr_common::{prelude::*, ALVR_VERSION};
use alvr_session::{EventSeverity, Raw, ServerEvent, Statistics};
use eframe::egui::{self, Color32, RichText};
use parking_lot::Mutex;
use serde_json::{json, Value};
use settings_schema::SchemaNode;
use std::{
    collections::VecDeque, env::consts::OS, net::IpAddr, path::PathBuf, sync::Arc, thread,
    time::Duration,
};
use tokio::sync::broadcast;

const WINDOW_WIDTH: f32 = 900.0;
const WINDOW_HEIGHT: f32 = 650.0;
const LOG_CAPACITY: usize = 2000;
const HISTORY_CAPACITY: usize = 300;
const RELEASES_URL: &str = "https://github.com/alvr-org/ALVR/releases";

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

#[derive(PartialEq, Clone, Copy)]
enum LogFilter {
    Debug,
    Info,
    Warning,
    Error,
}

impl LogFilter {
    fn allows(self, severity: &EventSeverity) -> bool {
        let level = match severity {
            EventSeverity::Debug => 0,
            EventSeverity::Info => 1,
            EventSeverity::Warning => 2,
            EventSeverity::Error => 3,
        };
        let minimum = match self {
            Self::Debug => 0,
            Self::Info => 1,
            Self::Warning => 2,
            Self::Error => 3,
        };
        level >= minimum
    }
}

struct SettingsState {
    schema: SchemaNode,
    draft: Value,
    show_advanced: bool,
    dirty: bool,
    message: Option<String>,
}

impl SettingsState {
    fn load() -> Self {
        let schema = alvr_session::settings_schema(alvr_session::session_settings_default());
        let draft = Self::stored();

        Self {
            schema,
            draft,
            show_advanced: false,
            dirty: false,
            message: None,
        }
    }

    fn stored() -> Value {
        serde_json::to_value(SESSION_MANAGER.lock().get())
            .ok()
            .and_then(|session| session.get("sessionSettings").cloned())
            .unwrap_or(Value::Null)
    }

    fn apply(&mut self) {
        let res = SESSION_MANAGER
            .lock()
            .get_mut()
            .merge_from_json(&json!({ "sessionSettings": self.draft }));

        self.message = Some(match res {
            Ok(()) => {
                self.dirty = false;
                "Settings saved. Some changes apply on the next connection.".to_owned()
            }
            Err(e) => format!("Settings saved with warnings: {e}"),
        });
        // show what was actually stored (invalid values may have been corrected)
        self.draft = Self::stored();
    }
}

struct DashboardApp {
    tab: Tab,
    events: broadcast::Receiver<String>,
    logs: VecDeque<Raw>,
    log_filter: LogFilter,
    stats: Option<Statistics>,
    history: VecDeque<Statistics>,
    client_connected: bool,
    last_notice: Option<String>,

    settings: SettingsState,

    new_client_hostname: String,
    new_client_ip: String,
    installation_message: Arc<Mutex<Option<String>>>,
    drivers: Vec<PathBuf>,
    gpu_names: Vec<String>,
}

impl DashboardApp {
    fn new(cc: &eframe::CreationContext<'_>, events: broadcast::Receiver<String>) -> Self {
        *CONTEXT.lock() = Some(cc.egui_ctx.clone());

        Self {
            tab: Tab::Monitor,
            events,
            logs: VecDeque::new(),
            log_filter: LogFilter::Info,
            stats: None,
            history: VecDeque::new(),
            client_connected: false,
            last_notice: None,
            settings: SettingsState::load(),
            new_client_hostname: String::new(),
            new_client_ip: String::new(),
            installation_message: Arc::new(Mutex::new(None)),
            drivers: alvr_commands::get_registered_drivers().unwrap_or_default(),
            gpu_names: graphics_info::get_gpu_names(),
        }
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
                            if self.logs.len() >= LOG_CAPACITY {
                                self.logs.pop_front();
                            }
                            self.logs.push_back(raw);
                        }
                        ServerEvent::Statistics(stats) => {
                            if self.history.len() >= HISTORY_CAPACITY {
                                self.history.pop_front();
                            }
                            self.history.push_back(stats.clone());
                            self.stats = Some(stats);
                        }
                        ServerEvent::ClientConnected => {
                            self.client_connected = true;
                            self.stats = None;
                            self.history.clear();
                        }
                        ServerEvent::ClientDisconnected => self.client_connected = false,
                        ServerEvent::ClientFoundWrongVersion(version) => {
                            self.last_notice = Some(format!(
                                "A client with an incompatible version ({version}) was found"
                            ));
                        }
                        ServerEvent::SessionSettingsExtrapolationFailed => {
                            self.last_notice =
                                Some("Some settings could not be migrated and were reset".into());
                        }
                        _ => (),
                    }
                }
                Err(broadcast::error::TryRecvError::Lagged(_)) => continue,
                Err(_) => break,
            }
        }
    }

    fn monitor_ui(&mut self, ui: &mut egui::Ui) {
        ui.heading("Connection");
        ui.label(if self.client_connected {
            RichText::new("Client streaming").color(Color32::from_rgb(0x2e, 0x9e, 0x4a))
        } else {
            RichText::new("Waiting for a client").color(Color32::GRAY)
        });

        ui.add_space(8.0);
        self.clients_ui(ui);

        ui.add_space(8.0);
        ui.heading("Statistics");
        match &self.stats {
            Some(stats) => {
                egui::Grid::new("stats").num_columns(2).show(ui, |ui| {
                    let rows = [
                        (
                            "Total latency",
                            format!("{:.0} ms", stats.total_latency * 1000.0),
                        ),
                        (
                            "Encode latency",
                            format!("{:.0} ms", stats.encode_latency * 1000.0),
                        ),
                        (
                            "Transport latency",
                            format!("{:.0} ms", stats.transport_latency * 1000.0),
                        ),
                        (
                            "Decode latency",
                            format!("{:.0} ms", stats.decode_latency * 1000.0),
                        ),
                        ("Server FPS", stats.server_f_p_s.to_string()),
                        ("Client FPS", stats.client_f_p_s.to_string()),
                        ("Bitrate", format!("{:.1} Mbps", stats.sent_rate)),
                        (
                            "Packets lost / s",
                            stats.packets_lost_per_second.to_string(),
                        ),
                        ("FEC failures / s", stats.fec_failure_in_second.to_string()),
                    ];
                    for (name, value) in rows {
                        ui.label(name);
                        ui.label(value);
                        ui.end_row();
                    }
                });
                ui.add_space(4.0);
                let latency = self
                    .history
                    .iter()
                    .map(|s| s.total_latency * 1000.0)
                    .collect::<Vec<_>>();
                sparkline(ui, "Total latency (ms)", &latency);
            }
            None => {
                ui.label("No statistics yet.");
            }
        }

        ui.add_space(8.0);
        ui.horizontal(|ui| {
            ui.heading("Log");
            egui::ComboBox::from_id_salt("log_filter")
                .selected_text(match self.log_filter {
                    LogFilter::Debug => "Debug",
                    LogFilter::Info => "Info",
                    LogFilter::Warning => "Warning",
                    LogFilter::Error => "Error",
                })
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut self.log_filter, LogFilter::Debug, "Debug");
                    ui.selectable_value(&mut self.log_filter, LogFilter::Info, "Info");
                    ui.selectable_value(&mut self.log_filter, LogFilter::Warning, "Warning");
                    ui.selectable_value(&mut self.log_filter, LogFilter::Error, "Error");
                });
            if ui.button("Clear").clicked() {
                self.logs.clear();
            }
        });
        egui::ScrollArea::vertical()
            .id_salt("log")
            .stick_to_bottom(true)
            .max_height(220.0)
            .auto_shrink([false, false])
            .show(ui, |ui| {
                for raw in self
                    .logs
                    .iter()
                    .filter(|r| self.log_filter.allows(&r.severity))
                {
                    let color = match raw.severity {
                        EventSeverity::Error => Color32::from_rgb(0xd0, 0x30, 0x30),
                        EventSeverity::Warning => Color32::from_rgb(0xc0, 0x80, 0x00),
                        EventSeverity::Info => ui.visuals().text_color(),
                        EventSeverity::Debug => Color32::GRAY,
                    };
                    ui.label(
                        RichText::new(format!("{} {}", raw.timestamp, raw.content))
                            .monospace()
                            .color(color),
                    );
                }
            });
    }

    fn clients_ui(&mut self, ui: &mut egui::Ui) {
        ui.heading("Clients");

        let clients = SESSION_MANAGER.lock().get().client_connections.clone();
        if clients.is_empty() {
            ui.label("No clients discovered yet. Start ALVR on the headset.");
        }

        let mut hostnames = clients.keys().cloned().collect::<Vec<_>>();
        hostnames.sort();
        for hostname in hostnames {
            let client = &clients[&hostname];
            ui.horizontal(|ui| {
                ui.label(format!("{} ({hostname})", client.display_name));
                if client.trusted {
                    ui.label(RichText::new("trusted").color(Color32::from_rgb(0x2e, 0x9e, 0x4a)));
                } else if ui.button("Trust").clicked() {
                    crate::update_client_list(
                        hostname.clone(),
                        ClientListAction::TrustAndMaybeAddIp(None),
                    );
                }
                if ui.button("Remove").clicked() {
                    crate::update_client_list(
                        hostname.clone(),
                        ClientListAction::RemoveIpOrEntry(None),
                    );
                }
            });
            for ip in &client.manual_ips {
                ui.horizontal(|ui| {
                    ui.add_space(24.0);
                    ui.label(ip.to_string());
                    if ui.small_button("Remove IP").clicked() {
                        crate::update_client_list(
                            hostname.clone(),
                            ClientListAction::RemoveIpOrEntry(Some(*ip)),
                        );
                    }
                });
            }
        }

        ui.add_space(4.0);
        ui.horizontal(|ui| {
            ui.label("Add client manually:");
            ui.add(
                egui::TextEdit::singleline(&mut self.new_client_hostname)
                    .hint_text("hostname")
                    .desired_width(160.0),
            );
            ui.add(
                egui::TextEdit::singleline(&mut self.new_client_ip)
                    .hint_text("IP address")
                    .desired_width(120.0),
            );
            let ip = self.new_client_ip.trim().parse::<IpAddr>();
            let valid = ip.is_ok() && !self.new_client_hostname.trim().is_empty();
            if ui.add_enabled(valid, egui::Button::new("Add")).clicked() {
                if let Ok(ip) = ip {
                    let hostname = self.new_client_hostname.trim().to_owned();
                    crate::update_client_list(
                        hostname.clone(),
                        ClientListAction::AddIfMissing {
                            display_name: hostname.clone(),
                        },
                    );
                    crate::update_client_list(
                        hostname,
                        ClientListAction::TrustAndMaybeAddIp(Some(ip)),
                    );
                    self.new_client_hostname.clear();
                    self.new_client_ip.clear();
                }
            }
        });
    }

    fn settings_ui(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.checkbox(&mut self.settings.show_advanced, "Show advanced settings");
            if ui
                .add_enabled(self.settings.dirty, egui::Button::new("Save"))
                .clicked()
            {
                self.settings.apply();
            }
            if ui
                .add_enabled(self.settings.dirty, egui::Button::new("Discard"))
                .clicked()
            {
                self.settings.draft = SettingsState::stored();
                self.settings.dirty = false;
            }
        });
        if let Some(message) = &self.settings.message {
            ui.label(message);
        }
        ui.separator();

        let settings = &mut self.settings;
        egui::ScrollArea::vertical()
            .id_salt("settings")
            .auto_shrink([false, false])
            .show(ui, |ui| {
                // the root section is rendered expanded, without its own header
                let SchemaNode::Section { entries } = &settings.schema else {
                    return;
                };
                let Some(fields) = settings.draft.as_object_mut() else {
                    ui.label("Settings are not available.");
                    return;
                };
                for (name, data) in entries {
                    let Some(data) = data else { continue };
                    if data.advanced && !settings.show_advanced {
                        continue;
                    }
                    if let Some(field) = fields.get_mut(name) {
                        settings.dirty |= settings_editor::node_ui(
                            ui,
                            name,
                            name,
                            &data.content,
                            field,
                            settings.show_advanced,
                        );
                    }
                }
            });
    }

    fn installation_ui(&mut self, ui: &mut egui::Ui) {
        ui.heading("SteamVR driver");
        ui.horizontal(|ui| {
            if ui.button("Register ALVR driver").clicked() {
                let res = alvr_commands::driver_registration(
                    &[FILESYSTEM_LAYOUT.openvr_driver_root_dir.clone()],
                    true,
                );
                self.report(res.map(|()| "Driver registered".to_owned()));
                self.drivers = alvr_commands::get_registered_drivers().unwrap_or_default();
            }
            if ui.button("Restart SteamVR").clicked() {
                crate::notify_restart_driver();
            }
        });
        ui.label("Registered drivers:");
        let mut unregister = None;
        for path in &self.drivers {
            ui.horizontal(|ui| {
                ui.label(path.to_string_lossy());
                if ui.small_button("Unregister").clicked() {
                    unregister = Some(path.clone());
                }
            });
        }
        if let Some(path) = unregister {
            let res = alvr_commands::driver_registration(&[path], false);
            self.report(res.map(|()| "Driver unregistered".to_owned()));
            self.drivers = alvr_commands::get_registered_drivers().unwrap_or_default();
        }

        ui.add_space(12.0);
        ui.heading("Firewall");
        ui.label(
            "Allows SteamVR to receive the headset connection. Administrator rights are requested.",
        );
        ui.horizontal(|ui| {
            if ui.button("Add firewall rules").clicked() {
                self.set_firewall_rules(true);
            }
            if ui.button("Remove firewall rules").clicked() {
                self.set_firewall_rules(false);
            }
        });

        if let Some(message) = self.installation_message.lock().as_ref() {
            ui.add_space(8.0);
            ui.label(message);
        }
    }

    fn report(&self, res: StrResult<String>) {
        *self.installation_message.lock() = Some(match res {
            Ok(message) => message,
            Err(e) => format!("Error: {e}"),
        });
    }

    fn set_firewall_rules(&self, add: bool) {
        let message = Arc::clone(&self.installation_message);
        // The elevation prompt blocks until the user answers: keep the UI responsive.
        thread::spawn(move || {
            let text = match alvr_commands::firewall_rules(add) {
                Ok(()) => "Firewall rules updated".to_owned(),
                Err(1) => "Firewall rule is already set".to_owned(),
                Err(126) => "Request dismissed".to_owned(),
                Err(code) => format!("Setting firewall rules failed: code {code}"),
            };
            *message.lock() = Some(text);
        });
    }

    fn about_ui(&mut self, ui: &mut egui::Ui) {
        ui.heading("ALVR");
        egui::Grid::new("about").num_columns(2).show(ui, |ui| {
            ui.label("Version");
            ui.label(ALVR_VERSION.to_string());
            ui.end_row();
            ui.label("Operating system");
            ui.label(OS);
            ui.end_row();
            ui.label("Graphics devices");
            ui.vertical(|ui| {
                for name in &self.gpu_names {
                    ui.label(name);
                }
            });
            ui.end_row();
        });
        ui.add_space(8.0);
        ui.hyperlink_to("Releases", RELEASES_URL);
        ui.label("Updates are installed manually from the releases page.");
    }
}

fn sparkline(ui: &mut egui::Ui, title: &str, values: &[f32]) {
    ui.label(title);
    let (rect, _) =
        ui.allocate_exact_size(egui::vec2(ui.available_width(), 60.0), egui::Sense::hover());
    let painter = ui.painter_at(rect);
    painter.rect_filled(rect, 2.0, ui.visuals().extreme_bg_color);

    if values.len() < 2 {
        return;
    }
    let max = values.iter().cloned().fold(f32::MIN, f32::max).max(1.0);
    let points = values
        .iter()
        .enumerate()
        .map(|(i, v)| {
            egui::pos2(
                rect.left() + rect.width() * i as f32 / (values.len() - 1) as f32,
                rect.bottom() - rect.height() * (v / max).clamp(0.0, 1.0),
            )
        })
        .collect::<Vec<_>>();
    painter.add(egui::Shape::line(
        points,
        egui::Stroke::new(1.5, Color32::from_rgb(0x30, 0x80, 0xd0)),
    ));
    painter.text(
        rect.right_top() + egui::vec2(-4.0, 2.0),
        egui::Align2::RIGHT_TOP,
        format!("max {max:.0}"),
        egui::FontId::proportional(10.0),
        ui.visuals().weak_text_color(),
    );
}

impl eframe::App for DashboardApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.poll_events();

        egui::Panel::top("tabs").show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.selectable_value(&mut self.tab, Tab::Monitor, "Monitor");
                ui.selectable_value(&mut self.tab, Tab::Settings, "Settings");
                ui.selectable_value(&mut self.tab, Tab::Installation, "Installation");
                ui.selectable_value(&mut self.tab, Tab::About, "About");
            });
        });

        if let Some(notice) = self.last_notice.clone() {
            egui::Panel::bottom("notice").show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.colored_label(Color32::from_rgb(0xc0, 0x80, 0x00), notice);
                    if ui.button("Dismiss").clicked() {
                        self.last_notice = None;
                    }
                });
            });
        }

        egui::CentralPanel::default().show(ui, |ui| match self.tab {
            Tab::Monitor => {
                egui::ScrollArea::vertical()
                    .id_salt("monitor")
                    .auto_shrink([false, false])
                    .show(ui, |ui| self.monitor_ui(ui))
                    .inner
            }
            Tab::Settings => self.settings_ui(ui),
            Tab::Installation => self.installation_ui(ui),
            Tab::About => self.about_ui(ui),
        });

        // log and statistics events arrive from other threads
        ui.ctx().request_repaint_after(Duration::from_millis(200));
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
            .with_title("ALVR Dashboard")
            .with_inner_size([WINDOW_WIDTH, WINDOW_HEIGHT]),
        centered: true,
        event_loop_builder: Some(event_loop_hook()),
        ..Default::default()
    };

    trace_err!(eframe::run_native(
        "ALVR Dashboard",
        options,
        Box::new(move |cc| Ok(Box::new(DashboardApp::new(cc, events)))),
    ))?;

    // the window was closed: prevent a second close request
    *CONTEXT.lock() = None;
    crate::shutdown_runtime();

    unsafe { crate::ShutdownSteamvr() };

    Ok(())
}
