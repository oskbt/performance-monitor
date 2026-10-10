//! eframe::App の実装。

use crate::config::{Config, UPDATE_INTERVALS_MS};
use crate::winutil;
use eframe::egui::{self, Color32};
use windows::Win32::Foundation::HWND;

/// 背景色（ダークテーマ固定）。
const BG: Color32 = Color32::from_rgb(0x16, 0x18, 0x1c);

pub struct MonitorApp {
    config: Config,
    hwnd: Option<HWND>,
}

impl MonitorApp {
    pub fn new(cc: &eframe::CreationContext<'_>, config: Config) -> Self {
        cc.egui_ctx.set_theme(egui::Theme::Dark);
        setup_fonts(&cc.egui_ctx);

        let hwnd = winutil::hwnd_of(cc);
        if let (Some(hwnd), Some(rect)) = (hwnd, config.window) {
            winutil::restore_window_rect(hwnd, rect);
        }

        Self { config, hwnd }
    }

    fn context_menu(&mut self, ui: &mut egui::Ui) {
        let before = self.config.clone();

        if ui.checkbox(&mut self.config.always_on_top, "常に最前面").changed() {
            ui.ctx().send_viewport_cmd(egui::ViewportCommand::WindowLevel(
                if self.config.always_on_top {
                    egui::WindowLevel::AlwaysOnTop
                } else {
                    egui::WindowLevel::Normal
                },
            ));
        }
        if ui.checkbox(&mut self.config.borderless, "枠なし").changed() {
            ui.ctx()
                .send_viewport_cmd(egui::ViewportCommand::Decorations(!self.config.borderless));
        }
        ui.checkbox(&mut self.config.compact, "コンパクト表示");
        ui.horizontal(|ui| {
            ui.label("不透明度");
            ui.add(egui::Slider::new(&mut self.config.opacity, 0.3..=1.0).show_value(false));
        });
        ui.menu_button("更新間隔", |ui| {
            for ms in UPDATE_INTERVALS_MS {
                ui.radio_value(
                    &mut self.config.update_interval_ms,
                    ms,
                    format!("{} 秒", ms as f64 / 1000.0),
                );
            }
        });
        ui.separator();
        if ui.button("終了").clicked() {
            ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
        }

        if self.config != before {
            self.config.save();
        }
    }
}

impl eframe::App for MonitorApp {
    fn clear_color(&self, _visuals: &egui::Visuals) -> [f32; 4] {
        [0.0; 4]
    }

    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        if let Some(rect) = self.hwnd.and_then(winutil::window_rect) {
            self.config.window = Some(rect);
        }

        let bg = BG.gamma_multiply(self.config.opacity);
        egui::CentralPanel::default()
            .frame(egui::Frame::NONE.fill(bg).inner_margin(6.0))
            .show(ctx, |ui| {
                ui.set_opacity(self.config.opacity);

                // 背景: 枠なし時のドラッグ移動と右クリックメニュー
                let bg_response = ui.interact(
                    ui.max_rect(),
                    ui.id().with("background"),
                    egui::Sense::click_and_drag(),
                );
                if bg_response.drag_started() && self.config.borderless {
                    ctx.send_viewport_cmd(egui::ViewportCommand::StartDrag);
                }
                bg_response.context_menu(|ui| self.context_menu(ui));

                ui.label("右クリックで設定メニュー");
            });
    }

    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        self.config.save();
    }
}

/// 日本語を表示できるよう Windows のシステムフォントをフォールバックとして追加する。
fn setup_fonts(ctx: &egui::Context) {
    const CANDIDATES: [&str; 3] = ["meiryo.ttc", "YuGothM.ttc", "msgothic.ttc"];
    let windir = std::env::var("WINDIR").unwrap_or_else(|_| r"C:\Windows".into());

    let Some(data) = CANDIDATES
        .iter()
        .find_map(|name| std::fs::read(format!(r"{windir}\Fonts\{name}")).ok())
    else {
        log::warn!("日本語フォントが見つかりませんでした");
        return;
    };

    let mut fonts = egui::FontDefinitions::default();
    fonts
        .font_data
        .insert("jp".into(), egui::FontData::from_owned(data).into());
    for family in [egui::FontFamily::Proportional, egui::FontFamily::Monospace] {
        fonts.families.entry(family).or_default().push("jp".into());
    }
    ctx.set_fonts(fonts);
}
