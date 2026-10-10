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
/// 列の最小幅。ウィンドウがこれより狭いと横スクロールになる。
const MIN_COLUMN_W: f32 = 190.0;
const TILE_GAP: f32 = 6.0;
/// 枠なし時にリサイズ操作を受け付けるウィンドウ端の幅。
const RESIZE_MARGIN: f32 = 6.0;

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
        ui.checkbox(&mut self.config.show_per_core_cpu, "CPU をスレッドごとに表示");
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

        let (columns, capacity) = {
            let mut history = self.history.lock().unwrap();
            history.set_capacity(self.config.history_len());
            (ui::build_columns(&history, self.config.show_per_core_cpu), history.capacity())
        };
        self.all_tiles =
            columns.iter().flatten().map(|t| (t.id.clone(), t.title.clone())).collect();
        let visible: Vec<Vec<_>> = columns
            .into_iter()
            .map(|column| {
                column.into_iter().filter(|t| !self.config.hidden.contains(&t.id)).collect::<Vec<_>>()
            })
            .filter(|column| !column.is_empty())
            .collect();

        let resizing = self.config.borderless && handle_borderless_resize(ctx);
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
                if bg_response.drag_started() && self.config.borderless && !resizing {
                    ctx.send_viewport_cmd(egui::ViewportCommand::StartDrag);
                }
                bg_response.context_menu(|ui| self.context_menu(ui, None));

                if visible.is_empty() {
                    ui.centered_and_justified(|ui| {
                        ui.colored_label(TEXT_DIM, "表示する項目がありません（右クリックで設定）");
                    });
                    return;
                }

                egui::ScrollArea::both()
                    .auto_shrink(false)
                    .scroll_source(egui::scroll_area::ScrollSource {
                        drag: false,
                        ..Default::default()
                    })
                    .show(ui, |ui| {
                        let n = visible.len();
                        let avail = ui.available_width();
                        let column_w = ((avail - TILE_GAP * (n - 1) as f32) / n as f32)
                            .max(MIN_COLUMN_W);

                        // 列ごとにタイルを縦に積む
                        let heights: Vec<Vec<f32>> = visible
                            .iter()
                            .map(|column| {
                                column
                                    .iter()
                                    .map(|t| t.height(ui.painter(), compact, column_w))
                                    .collect()
                            })
                            .collect();
                        let total_h = heights
                            .iter()
                            .map(|hs| hs.iter().sum::<f32>() + TILE_GAP * (hs.len() - 1) as f32)
                            .fold(0.0, f32::max);
                        let total_w = column_w * n as f32 + TILE_GAP * (n - 1) as f32;
                        let (area, _) =
                            ui.allocate_exact_size(vec2(total_w, total_h), egui::Sense::hover());

                        for (i, (column, hs)) in visible.iter().zip(&heights).enumerate() {
                            let x = area.left() + i as f32 * (column_w + TILE_GAP);
                            let mut y = area.top();
                            for (tile, &h) in column.iter().zip(hs) {
                                let rect = Rect::from_min_size(egui::pos2(x, y), vec2(column_w, h));
                                y += h + TILE_GAP;
                                let response = tile.draw(ui, rect, compact, capacity);
                                if response.drag_started() && self.config.borderless && !resizing {
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

/// 枠なしウィンドウの端でのリサイズ。端でマウスを押したら true を返す。
fn handle_borderless_resize(ctx: &egui::Context) -> bool {
    use egui::{CursorIcon, ResizeDirection as D};

    let (pos, rect, pressed) =
        ctx.input(|i| (i.pointer.hover_pos(), i.viewport_rect(), i.pointer.primary_pressed()));
    let Some(pos) = pos else { return false };

    let left = pos.x - rect.left() < RESIZE_MARGIN;
    let right = rect.right() - pos.x < RESIZE_MARGIN;
    let top = pos.y - rect.top() < RESIZE_MARGIN;
    let bottom = rect.bottom() - pos.y < RESIZE_MARGIN;
    let (direction, cursor) = match (left, right, top, bottom) {
        (true, _, true, _) => (D::NorthWest, CursorIcon::ResizeNorthWest),
        (_, true, true, _) => (D::NorthEast, CursorIcon::ResizeNorthEast),
        (true, _, _, true) => (D::SouthWest, CursorIcon::ResizeSouthWest),
        (_, true, _, true) => (D::SouthEast, CursorIcon::ResizeSouthEast),
        (true, ..) => (D::West, CursorIcon::ResizeWest),
        (_, true, ..) => (D::East, CursorIcon::ResizeEast),
        (_, _, true, _) => (D::North, CursorIcon::ResizeNorth),
        (.., true) => (D::South, CursorIcon::ResizeSouth),
        _ => return false,
    };

    ctx.set_cursor_icon(cursor);
    if pressed {
        ctx.send_viewport_cmd(egui::ViewportCommand::BeginResize(direction));
    }
    pressed
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
