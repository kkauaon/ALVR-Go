#![windows_subsystem = "windows"]

mod commands;

use alvr_common::prelude::*;
use alvr_filesystem as afs;
use eframe::egui;
use std::{
    env,
    net::{Ipv4Addr, SocketAddr, TcpStream},
    sync::Arc,
    thread,
    time::Duration,
};

// Default value of the `web_server_port` setting, now used only as the driver readiness beacon
const DRIVER_READY_PORT: u16 = 8082;

const WINDOW_WIDTH: f32 = 500.0;
const WINDOW_HEIGHT: f32 = 300.0;

#[derive(Clone, PartialEq)]
enum View {
    RequirementsCheck { steamvr: String },
    Launching { resetting: bool },
    Closing,
}

type SharedView = Arc<parking_lot::Mutex<View>>;

fn set_view(view: &SharedView, ctx: &egui::Context, new_view: View) {
    *view.lock() = new_view;
    ctx.request_repaint();
}

fn launcher_lifecycle(view: SharedView, ctx: egui::Context) {
    loop {
        let steamvr_ok = commands::check_steamvr_installation();

        if steamvr_ok {
            break;
        } else {
            let steamvr_string =
                "SteamVR not installed: make sure you launched it at least once, then close it.";

            set_view(
                &view,
                &ctx,
                View::RequirementsCheck {
                    steamvr: steamvr_string.to_owned(),
                },
            );

            thread::sleep(Duration::from_millis(500));
        }
    }

    set_view(&view, &ctx, View::Launching { resetting: false });

    let mut tried_steamvr_launch = false;
    loop {
        // the driver accepts connections on this loopback port once its dashboard is running
        let driver_ready = TcpStream::connect_timeout(
            &SocketAddr::new(Ipv4Addr::LOCALHOST.into(), DRIVER_READY_PORT),
            Duration::from_millis(100),
        )
        .is_ok();
        if driver_ready {
            set_view(&view, &ctx, View::Closing);
            break;
        }

        // try to launch SteamVR only one time automatically
        if !tried_steamvr_launch {
            if alvr_common::show_err(commands::maybe_register_alvr_driver()).is_some() {
                if commands::is_steamvr_running() {
                    commands::kill_steamvr();
                    thread::sleep(Duration::from_secs(2))
                }
                commands::maybe_launch_steamvr();
            }
            tried_steamvr_launch = true;
        }

        thread::sleep(Duration::from_millis(500));
    }
}

fn reset_and_retry(view: SharedView, ctx: egui::Context) {
    thread::spawn(move || {
        set_view(&view, &ctx, View::Launching { resetting: true });

        commands::kill_steamvr();

        commands::fix_steamvr();

        commands::restart_steamvr();

        thread::sleep(Duration::from_secs(2));

        set_view(&view, &ctx, View::Launching { resetting: false });
    });
}

struct LauncherApp {
    view: SharedView,
}

impl LauncherApp {
    fn new(cc: &eframe::CreationContext<'_>) -> Self {
        let mut style = (*cc.egui_ctx.global_style()).clone();
        style.visuals = egui::Visuals::light();
        cc.egui_ctx.set_global_style(style);

        let view: SharedView = Arc::new(parking_lot::Mutex::new(View::RequirementsCheck {
            steamvr: "".into(),
        }));

        let lifecycle_view = Arc::clone(&view);
        let lifecycle_ctx = cc.egui_ctx.clone();
        thread::spawn(move || launcher_lifecycle(lifecycle_view, lifecycle_ctx));

        Self { view }
    }
}

impl eframe::App for LauncherApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let view = self.view.lock().clone();

        match view {
            View::RequirementsCheck { steamvr } => {
                ui.add_space(60.0);
                ui.vertical_centered(|ui| {
                    ui.add(egui::Label::new(steamvr).wrap());
                });
            }
            View::Launching { resetting } => {
                ui.add_space(60.0);
                ui.vertical_centered(|ui| {
                    ui.label(egui::RichText::new("Waiting for server to load...").size(25.0));
                    ui.add_space(10.0);
                    if !resetting {
                        if ui.button("Reset drivers and retry").clicked() {
                            reset_and_retry(Arc::clone(&self.view), ui.ctx().clone());
                        }
                    } else {
                        ui.label("Please wait for multiple restarts");
                    }
                });
            }
            View::Closing => {
                ui.send_viewport_cmd(egui::ViewportCommand::Close);
            }
        }
    }
}

fn make_window() -> StrResult {
    let instance_mutex = trace_err!(single_instance::SingleInstance::new("alvr_launcher_mutex"))?;
    if instance_mutex.is_single() {
        let driver_dir = afs::filesystem_layout_from_launcher_exe(&env::current_exe().unwrap())
            .openvr_driver_root_dir;

        if driver_dir.to_str().filter(|s| s.is_ascii()).is_none() {
            alvr_common::show_e_blocking(format!(
                "The path of this folder ({}) contains non ASCII characters. {}",
                driver_dir.to_string_lossy(),
                "Please move it somewhere else (for example in C:\\Users\\Public\\Documents).",
            ));
            return Ok(());
        }

        let options = eframe::NativeOptions {
            viewport: egui::ViewportBuilder::default()
                .with_title("ALVR Launcher")
                .with_inner_size([WINDOW_WIDTH, WINDOW_HEIGHT])
                .with_resizable(false),
            centered: true,
            ..Default::default()
        };

        trace_err!(eframe::run_native(
            "ALVR Launcher",
            options,
            Box::new(|cc| Ok(Box::new(LauncherApp::new(cc)))),
        ))?;
    }
    Ok(())
}

fn main() {
    let args = env::args().collect::<Vec<_>>();
    match args.get(1) {
        Some(flag) if flag == "--restart-steamvr" => commands::restart_steamvr(),
        Some(flag) if flag == "--update" => commands::invoke_installer(),
        Some(_) | None => {
            alvr_common::show_err_blocking(make_window());
        }
    }
}
