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
/// スレッド別などの小グラフ 1 つ分の高さ。
const CELL_H: f32 = 40.0;
const CELL_H_COMPACT: f32 = 26.0;
const METER_H: f32 = 16.0;
const LINE_H: f32 = 16.0;
const GAP: f32 = 4.0;
const MARKER_W: f32 = 12.0;
const LEGEND_SPACING: f32 = 12.0;

fn legend_font() -> FontId {
    FontId::proportional(12.0)
}

pub enum Graph {
    /// 1 つのグラフに複数系列を重ねる。`scale` は縦軸上限のラベル。
    Lines { series: Vec<Series>, max: f32, scale: Option<String> },
    /// 系列ごとに小さなグラフを格子状に並べる（0〜100%）。各セルに番号と現在値を表示する。
    Cells(Vec<Vec<f32>>),
    /// 横棒のメーター（温度など）。コンパクト表示でも表示する。
    Meter {
        label: String,
        value: Option<f32>,
        max: f32,
        /// (警告, 危険) のしきい値。
        thresholds: (f32, f32),
        unit: &'static str,
    },
}

pub struct Tile {
    /// 非表示設定に使う ID（`"cpu"`, `"disk:0"` など）。
    pub id: String,
    pub title: String,
    /// 右上に大きく表示する現在値。
    pub value: String,
    pub color: Color32,
    /// 上から順に描くグラフ。2 つ目以降の折れ線グラフは小さめに描き、コンパクト表示では省略する。
    pub graphs: Vec<Graph>,
    /// グラフ下の 1 行（凡例・主要な数値）。コンパクト表示でも表示する。
    pub legend: Vec<(Option<Color32>, String)>,
    /// 補助情報。コンパクト表示では省略する。
    pub details: Vec<String>,
    /// 危険な状態（温度超過など）。枠を赤く強調する。
    pub alert: bool,
}

impl Tile {
    /// 幅 `width` で描いたときに必要な高さ。
    pub fn height(&self, painter: &egui::Painter, compact: bool, width: f32) -> f32 {
        let graphs_h: f32 =
            self.graph_layout(compact, width - 2.0 * PAD).iter().map(|(_, h)| h + GAP).sum();
        let mut h = PAD + HEADER_H + graphs_h - GAP + PAD;
        if let Some(rows) = self.legend_layout(painter, width - 2.0 * PAD).last().map(|e| e.0 + 1) {
            h += GAP + rows as f32 * LINE_H;
        }
        if !compact && !self.details.is_empty() {
            h += self.details.len() as f32 * LINE_H;
        }
        h
    }

    /// 凡例の各項目の配置 (行, x)。幅に収まらない項目は次の行へ折り返す。
    fn legend_layout(&self, painter: &egui::Painter, width: f32) -> Vec<(usize, f32)> {
        let mut row = 0;
        let mut x = 0.0;
        self.legend
            .iter()
            .map(|(color, text)| {
                let w = if color.is_some() { MARKER_W } else { 0.0 }
                    + painter.layout_no_wrap(text.clone(), legend_font(), TEXT).size().x;
                if x > 0.0 && x + w > width {
                    row += 1;
                    x = 0.0;
                }
                let pos = (row, x);
                x += w + LEGEND_SPACING;
                pos
            })
            .collect()
    }

    /// 描くグラフとその高さ。`width` はタイル内側の幅。
    fn graph_layout(&self, compact: bool, width: f32) -> Vec<(&Graph, f32)> {
        let base = if compact { GRAPH_H_COMPACT } else { GRAPH_H };
        let cell_h = if compact { CELL_H_COMPACT } else { CELL_H };
        self.graphs
            .iter()
            .enumerate()
            .filter_map(|(i, g)| {
                let h = match g {
                    Graph::Cells(cells) => {
                        let (_, rows) = graph::cell_grid(cells.len(), width);
                        rows as f32 * (cell_h + graph::CELL_GAP) - graph::CELL_GAP
                    }
                    Graph::Meter { .. } => METER_H,
                    Graph::Lines { .. } if i == 0 => base,
                    Graph::Lines { .. } if compact => return None,
                    Graph::Lines { .. } => base * 0.6,
                };
                Some((g, h))
            })
            .collect()
    }

    pub fn draw(&self, ui: &mut egui::Ui, rect: Rect, compact: bool, capacity: usize) -> Response {
        let response = ui.interact(rect, ui.id().with(&self.id), Sense::click_and_drag());
        let painter = ui.painter_at(rect);

        let border = if self.alert {
            Stroke::new(2.0, graph::METER_DANGER)
        } else {
            Stroke::new(1.0, self.color.gamma_multiply(0.35))
        };
        painter.rect(rect, CornerRadius::same(4), TILE_BG, border, StrokeKind::Inside);

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

        // グラフ（割り当てられた高さが必要より大きい場合、余りは 1 つ目のグラフに回す）
        let mut layout = self.graph_layout(compact, inner.width());
        if let Some((_, first)) = layout.first_mut() {
            *first += (rect.height() - self.height(&painter, compact, rect.width())).max(0.0);
        }
        let mut y = inner.top() + HEADER_H;
        for (graph, h) in layout {
            let graph_rect = Rect::from_min_size(egui::pos2(inner.left(), y), vec2(inner.width(), h));
            self.draw_graph(&painter, graph_rect, graph, capacity);
            y += h + GAP;
        }

        // 凡例
        let layout = self.legend_layout(&painter, inner.width());
        for ((color, text), &(row, x)) in self.legend.iter().zip(&layout) {
            let mut pos = egui::pos2(inner.left() + x, y + row as f32 * LINE_H);
            if let Some(color) = color {
                let marker = Rect::from_min_size(pos + vec2(0.0, 4.0), vec2(8.0, 8.0));
                painter.rect_filled(marker, 1.0, *color);
                pos.x += MARKER_W;
            }
            let galley = truncated(&painter, text, legend_font(), TEXT, inner.right() - pos.x);
            painter.galley(pos, galley, TEXT);
        }
        if let Some(&(rows, _)) = layout.last() {
            y += (rows + 1) as f32 * LINE_H;
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

    fn draw_graph(&self, painter: &egui::Painter, rect: Rect, graph: &Graph, capacity: usize) {
        match graph {
            Graph::Lines { series, max, scale } => {
                graph::draw(painter, rect, series, *max, capacity, self.color.gamma_multiply(0.6));
                if let Some(scale) = scale {
                    painter.text(
                        rect.left_top() + vec2(3.0, 1.0),
                        Align2::LEFT_TOP,
                        scale,
                        FontId::proportional(10.0),
                        TEXT_DIM,
                    );
                }
            }
            Graph::Cells(cells) => {
                let (cols, _) = graph::cell_grid(cells.len(), rect.width());
                graph::draw_cells(painter, rect, cells, cols, capacity, self.color);
            }
            Graph::Meter { label, value, max, thresholds, unit } => {
                graph::draw_meter(painter, rect, label, *value, *max, *thresholds, unit);
            }
        }
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
