//! eframe::App の実装。

use crate::collector;
use crate::config::{Config, UPDATE_INTERVALS_MS};
use crate::history::History;
use crate::ui::{self, tile::TEXT_DIM};
use crate::winutil;
use eframe::egui::{self, Color32, Rect, vec2};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use windows::Win32::Foundation::HWND;

/// 背景色（ダークテーマ固定）。
const BG: Color32 = Color32::from_rgb(0x16, 0x18, 0x1c);
const MIN_TILE_W: f32 = 220.0;
const TILE_GAP: f32 = 6.0;

pub struct MonitorApp {
    config: Config,
    hwnd: Option<HWND>,
    history: Arc<Mutex<History>>,
    interval_ms: Arc<AtomicU64>,
    /// 全タイルの (ID, タイトル)。「表示項目」メニュー用。
    all_tiles: Vec<(String, String)>,
}

impl MonitorApp {
    pub fn new(cc: &eframe::CreationContext<'_>, config: Config) -> Self {
        cc.egui_ctx.set_theme(egui::Theme::Dark);
        setup_fonts(&cc.egui_ctx);

        let hwnd = winutil::hwnd_of(cc);
        if let (Some(hwnd), Some(rect)) = (hwnd, config.window) {
            winutil::restore_window_rect(hwnd, rect);
        }

        let history = Arc::new(Mutex::new(History::new(config.history_len())));
        let interval_ms = Arc::new(AtomicU64::new(config.update_interval_ms));
        collector::spawn(cc.egui_ctx.clone(), history.clone(), interval_ms.clone());

        Self { config, hwnd, history, interval_ms, all_tiles: Vec::new() }
    }

    /// 右クリックメニュー。`tile` はメニューを開いたタイルの ID。
    fn context_menu(&mut self, ui: &mut egui::Ui, tile: Option<&str>) {
        let before = self.config.clone();

        if let Some(id) = tile {
            if ui.button("このタイルを非表示").clicked() {
                self.config.hidden.insert(id.to_owned());
                ui.close();
            }
            ui.separator();
        }

        ui.menu_button("表示項目", |ui| {
            for (id, title) in &self.all_tiles {
                let mut shown = !self.config.hidden.contains(id);
                if ui.checkbox(&mut shown, title).changed() {
                    if shown {
                        self.config.hidden.remove(id);
                    } else {
                        self.config.hidden.insert(id.clone());
                    }
                }
            }
            ui.separator();
            if ui.button("すべて表示").clicked() {
                self.config.hidden.clear();
            }
        });
        ui.checkbox(&mut self.config.show_per_core_cpu, "CPU を論理プロセッサごとに表示");
        ui.checkbox(&mut self.config.compact, "コンパクト表示");
        ui.separator();

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
        self.interval_ms.store(self.config.update_interval_ms, Ordering::Relaxed);

        let (tiles, capacity) = {
            let mut history = self.history.lock().unwrap();
            history.set_capacity(self.config.history_len());
            (ui::build_tiles(&history, self.config.show_per_core_cpu), history.capacity())
        };
        self.all_tiles = tiles.iter().map(|t| (t.id.clone(), t.title.clone())).collect();
        let visible: Vec<_> =
            tiles.into_iter().filter(|t| !self.config.hidden.contains(&t.id)).collect();

        let compact = self.config.compact;
        let bg = BG.gamma_multiply(self.config.opacity);
        egui::CentralPanel::default()
            .frame(egui::Frame::NONE.fill(bg).inner_margin(TILE_GAP))
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
                bg_response.context_menu(|ui| self.context_menu(ui, None));

                if visible.is_empty() {
                    ui.centered_and_justified(|ui| {
                        ui.colored_label(TEXT_DIM, "表示する項目がありません（右クリックで設定）");
                    });
                    return;
                }

                egui::ScrollArea::vertical()
                    .auto_shrink(false)
                    .scroll_source(egui::scroll_area::ScrollSource {
                        drag: false,
                        ..Default::default()
                    })
                    .show(ui, |ui| {
                        ui.spacing_mut().item_spacing = vec2(TILE_GAP, TILE_GAP);
                        let avail = ui.available_width();
                        let cols = (((avail + TILE_GAP) / (MIN_TILE_W + TILE_GAP)).floor() as usize)
                            .max(1);
                        let tile_w = (avail - TILE_GAP * (cols - 1) as f32) / cols as f32;

                        for row in visible.chunks(cols) {
                            let row_h = row
                                .iter()
                                .map(|t| t.height(ui.painter(), compact, tile_w))
                                .fold(0.0, f32::max);
                            let (row_rect, _) =
                                ui.allocate_exact_size(vec2(avail, row_h), egui::Sense::hover());
                            for (i, tile) in row.iter().enumerate() {
                                let rect = Rect::from_min_size(
                                    row_rect.min + vec2(i as f32 * (tile_w + TILE_GAP), 0.0),
                                    vec2(tile_w, row_h),
                                );
                                let response = tile.draw(ui, rect, compact, capacity);
                                if response.drag_started() && self.config.borderless {
                                    ctx.send_viewport_cmd(egui::ViewportCommand::StartDrag);
                                }
                                response.context_menu(|ui| self.context_menu(ui, Some(&tile.id)));
                            }
                        }
                    });
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
