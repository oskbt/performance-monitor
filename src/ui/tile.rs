//! タイル（1 項目分のカード）の描画。

use super::graph::{self, Series};
use eframe::egui::{
    self, Align2, Color32, CornerRadius, FontId, Rect, Response, Sense, Stroke, StrokeKind,
    text::{LayoutJob, TextFormat, TextWrapping},
    vec2,
};

pub const TILE_BG: Color32 = Color32::from_rgb(0x1f, 0x22, 0x27);
pub const TEXT: Color32 = Color32::from_rgb(0xe6, 0xe8, 0xeb);
pub const TEXT_DIM: Color32 = Color32::from_rgb(0x9a, 0xa0, 0xa8);

const PAD: f32 = 8.0;
const HEADER_H: f32 = 22.0;
const GRAPH_H: f32 = 70.0;
const GRAPH_H_COMPACT: f32 = 34.0;
const LINE_H: f32 = 16.0;
const GAP: f32 = 4.0;

pub enum Graph {
    /// 1 つのグラフに複数系列を重ねる。`scale` は縦軸上限のラベル。
    Lines { series: Vec<Series>, max: f32, scale: Option<String> },
    /// 系列ごとに小さなグラフを格子状に並べる（0〜100%）。
    Cells(Vec<Vec<f32>>),
}

pub struct Tile {
    /// 非表示設定に使う ID（`"cpu"`, `"disk:0"` など）。
    pub id: String,
    pub title: String,
    /// 右上に大きく表示する現在値。
    pub value: String,
    pub color: Color32,
    pub graph: Graph,
    /// グラフ下の 1 行（凡例・主要な数値）。コンパクト表示でも表示する。
    pub legend: Vec<(Option<Color32>, String)>,
    /// 補助情報。コンパクト表示では省略する。
    pub details: Vec<String>,
}

impl Tile {
    pub fn height(&self, compact: bool) -> f32 {
        let mut graph_h = if compact { GRAPH_H_COMPACT } else { GRAPH_H };
        if matches!(self.graph, Graph::Cells(_)) {
            graph_h *= 1.6;
        }
        let mut h = PAD + HEADER_H + graph_h + PAD;
        if !self.legend.is_empty() {
            h += GAP + LINE_H;
        }
        if !compact && !self.details.is_empty() {
            h += self.details.len() as f32 * LINE_H;
        }
        h
    }

    pub fn draw(&self, ui: &mut egui::Ui, rect: Rect, compact: bool, capacity: usize) -> Response {
        let response = ui.interact(rect, ui.id().with(&self.id), Sense::click_and_drag());
        let painter = ui.painter_at(rect);

        painter.rect(
            rect,
            CornerRadius::same(4),
            TILE_BG,
            Stroke::new(1.0, self.color.gamma_multiply(0.35)),
            StrokeKind::Inside,
        );

        let inner = rect.shrink(PAD);

        // ヘッダー: タイトル（左）と現在値（右）
        let value = painter.text(
            inner.right_top() + vec2(0.0, HEADER_H / 2.0 - 2.0),
            Align2::RIGHT_CENTER,
            &self.value,
            FontId::proportional(17.0),
            self.color,
        );
        let title_w = (value.left() - inner.left() - 8.0).max(10.0);
        let title = truncated(&painter, &self.title, FontId::proportional(13.0), TEXT, title_w);
        painter.galley(
            egui::pos2(inner.left(), inner.top() + (HEADER_H - title.size().y) / 2.0 - 2.0),
            title,
            TEXT,
        );

        // グラフ
        let footer_h = if self.legend.is_empty() { 0.0 } else { GAP + LINE_H }
            + if compact { 0.0 } else { self.details.len() as f32 * LINE_H };
        let graph_rect = Rect::from_min_max(
            inner.left_top() + vec2(0.0, HEADER_H),
            egui::pos2(inner.right(), inner.bottom() - footer_h),
        );
        match &self.graph {
            Graph::Lines { series, max, scale } => {
                graph::draw(&painter, graph_rect, series, *max, capacity, self.color.gamma_multiply(0.6));
                if let Some(scale) = scale {
                    painter.text(
                        graph_rect.left_top() + vec2(3.0, 1.0),
                        Align2::LEFT_TOP,
                        scale,
                        FontId::proportional(10.0),
                        TEXT_DIM,
                    );
                }
            }
            Graph::Cells(cells) => graph::draw_cells(&painter, graph_rect, cells, capacity, self.color),
        }

        // 凡例
        let mut y = graph_rect.bottom() + GAP;
        if !self.legend.is_empty() {
            let mut x = inner.left();
            for (color, text) in &self.legend {
                if let Some(color) = color {
                    let marker = Rect::from_min_size(egui::pos2(x, y + 4.0), vec2(8.0, 8.0));
                    painter.rect_filled(marker, 1.0, *color);
                    x += 12.0;
                }
                let galley =
                    truncated(&painter, text, FontId::proportional(12.0), TEXT, inner.right() - x);
                let w = galley.size().x;
                painter.galley(egui::pos2(x, y), galley, TEXT);
                x += w + 12.0;
                if x >= inner.right() {
                    break;
                }
            }
            y += LINE_H;
        }

        // 補助情報
        if !compact {
            for line in &self.details {
                let galley =
                    truncated(&painter, line, FontId::proportional(11.5), TEXT_DIM, inner.width());
                painter.galley(egui::pos2(inner.left(), y), galley, TEXT_DIM);
                y += LINE_H;
            }
        }

        response
    }
}

/// 1 行に収まらない部分を「…」で省略したテキスト。
fn truncated(
    painter: &egui::Painter,
    text: &str,
    font: FontId,
    color: Color32,
    max_width: f32,
) -> std::sync::Arc<egui::Galley> {
    let mut job = LayoutJob::single_section(text.to_owned(), TextFormat::simple(font, color));
    job.wrap = TextWrapping::truncate_at_width(max_width.max(1.0));
    painter.layout_job(job)
}
