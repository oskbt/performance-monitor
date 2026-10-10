#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod autostart;
mod collector;
mod history;
mod icon;
mod tray;
mod ui;
mod config;
mod fonts;
mod winutil;

use eframe::egui;

fn main() -> eframe::Result {
    env_logger::init();
    let config = config::Config::load();

    let viewport = egui::ViewportBuilder::default()
        .with_title("Performance Monitor")
        .with_inner_size([1100.0, 640.0])
        .with_min_inner_size([240.0, 120.0])
        .with_transparent(true)
        // ヘッダーレス。移動は背景のドラッグ、終了は右クリックメニューかタスクトレイから
        .with_decorations(false)
        // タスクバーには出さず、タスクトレイに常駐する
        .with_taskbar(false)
        .with_icon(egui::IconData { rgba: icon::rgba(64), width: 64, height: 64 })
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
