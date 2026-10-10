#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod collector;
mod history;
mod ui;
mod config;
mod winutil;

use eframe::egui;

fn main() -> eframe::Result {
    env_logger::init();
    let config = config::Config::load();

    let viewport = egui::ViewportBuilder::default()
        .with_title("Performance Monitor")
        .with_inner_size([720.0, 480.0])
        .with_min_inner_size([240.0, 120.0])
        .with_transparent(true)
        .with_decorations(!config.borderless)
        .with_window_level(if config.always_on_top {
            egui::WindowLevel::AlwaysOnTop
        } else {
            egui::WindowLevel::Normal
        });

    let options = eframe::NativeOptions {
        viewport,
        centered: true,
        ..Default::default()
    };

    eframe::run_native(
        "Performance Monitor",
        options,
        Box::new(|cc| Ok(Box::new(app::MonitorApp::new(cc, config)))),
    )
}
