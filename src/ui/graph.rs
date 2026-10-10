//! 時系列の折れ線グラフ描画。

use eframe::egui::{
    Align2, Color32, FontId, Mesh, Painter, Pos2, Rect, Shape, Stroke, StrokeKind, pos2, vec2,
};

pub const GRAPH_BG: Color32 = Color32::from_rgb(0x0e, 0x10, 0x13);
const GRID: Color32 = Color32::from_rgb(0x26, 0x2a, 0x30);
const LABEL: Color32 = Color32::from_rgb(0x9a, 0xa0, 0xa8);

pub const CELL_GAP: f32 = 3.0;

pub struct Series {
    /// 古い順の値。NaN は欠損（線を途切れさせる）。
    pub values: Vec<f32>,
    pub color: Color32,
    pub fill: bool,
    pub dashed: bool,
}

impl Series {
    pub fn filled(values: Vec<f32>, color: Color32) -> Self {
        Self { values, color, fill: true, dashed: false }
    }

    pub fn line(values: Vec<f32>, color: Color32) -> Self {
        Self { values, color, fill: false, dashed: false }
    }

    pub fn dashed(values: Vec<f32>, color: Color32) -> Self {
        Self { values, color, fill: false, dashed: true }
    }
}

/// `rect` にグラフを描く。最新の値が右端に来て、`capacity` 個で全幅になる。
pub fn draw(painter: &Painter, rect: Rect, series: &[Series], max: f32, capacity: usize, border: Color32) {
    draw_with_grid(painter, rect, series, max, capacity, border, true);
}

fn draw_with_grid(
    painter: &Painter,
    rect: Rect,
    series: &[Series],
    max: f32,
    capacity: usize,
    border: Color32,
    grid: bool,
) {
    painter.rect_filled(rect, 0.0, GRAPH_BG);
    if grid {
        draw_grid(painter, rect);
    }

    let painter = painter.with_clip_rect(rect);
    let step = rect.width() / (capacity.max(2) - 1) as f32;
    let max = if max > 0.0 { max } else { 1.0 };

    for s in series {
        let n = s.values.len();
        let mut segment: Vec<Pos2> = Vec::with_capacity(n);
        for (i, v) in s.values.iter().enumerate() {
            if v.is_nan() {
                draw_segment(&painter, rect, &segment, s);
                segment.clear();
                continue;
            }
            let x = rect.right() - (n - 1 - i) as f32 * step;
            let y = rect.bottom() - (v / max).clamp(0.0, 1.0) * rect.height();
            segment.push(pos2(x, y));
        }
        draw_segment(&painter, rect, &segment, s);
    }

    painter.rect_stroke(rect, 0.0, Stroke::new(1.0, border), StrokeKind::Inside);
}

/// 小さなグラフを縦 1 列に並べる（スレッド別 CPU など）。
/// 各セルの左上に番号、右上に現在値を表示する。
pub fn draw_cells(painter: &Painter, rect: Rect, cells: &[Vec<f32>], capacity: usize, color: Color32) {
    let n = cells.len().max(1);
    let h = (rect.height() - CELL_GAP * (n - 1) as f32) / n as f32;
    let font = FontId::proportional(10.0);

    for (i, values) in cells.iter().enumerate() {
        let min = rect.min + vec2(0.0, i as f32 * (h + CELL_GAP));
        let cell = Rect::from_min_size(min, vec2(rect.width(), h));
        draw_with_grid(
            painter,
            cell,
            &[Series::filled(values.clone(), color)],
            100.0,
            capacity,
            color.gamma_multiply(0.4),
            false,
        );
        painter.text(cell.left_top() + vec2(3.0, 1.0), Align2::LEFT_TOP, i.to_string(), font.clone(), LABEL);
        if let Some(v) = values.last().filter(|v| v.is_finite()) {
            painter.text(
                cell.right_top() + vec2(-3.0, 1.0),
                Align2::RIGHT_TOP,
                format!("{v:.0}%"),
                font.clone(),
                LABEL,
            );
        }
    }
}

pub const METER_OK: Color32 = Color32::from_rgb(0x5c, 0xb8, 0x5c);
pub const METER_WARNING: Color32 = Color32::from_rgb(0xf0, 0xad, 0x4e);
pub const METER_DANGER: Color32 = Color32::from_rgb(0xe5, 0x53, 0x4b);

/// しきい値に応じたメーターの色。
pub fn meter_color(value: f32, warning: f32, danger: f32) -> Color32 {
    if value >= danger {
        METER_DANGER
    } else if value >= warning {
        METER_WARNING
    } else {
        METER_OK
    }
}

/// 横棒のメーター。`warning` / `danger` の位置に目盛りを描き、値に応じて色を変える。
/// `value` が None なら「取得不可」と表示する。
pub fn draw_meter(
    painter: &Painter,
    rect: Rect,
    label: &str,
    value: Option<f32>,
    max: f32,
    (warning, danger): (f32, f32),
    unit: &str,
) {
    let font = FontId::proportional(11.0);
    let text = Color32::from_rgb(0xf0, 0xf2, 0xf4);

    // 数値はバーの右外側に置き、しきい値の目盛りと重ならないようにする
    let (value_text, value_color) = match value {
        Some(v) => (format!("{v:.0}{unit}"), meter_color(v, warning, danger)),
        None => ("取得不可".to_string(), LABEL),
    };
    let value_galley = painter.layout_no_wrap(value_text, font.clone(), value_color);
    let bar = Rect::from_min_max(
        rect.min,
        pos2(rect.right() - value_galley.size().x - 6.0, rect.bottom()),
    );
    painter.galley(
        pos2(rect.right() - value_galley.size().x, rect.center().y - value_galley.size().y / 2.0),
        value_galley,
        value_color,
    );

    painter.rect_filled(bar, 2.0, GRAPH_BG);
    let x_at = |v: f32| bar.left() + bar.width() * (v / max).clamp(0.0, 1.0);
    if let Some(v) = value {
        let fill = Rect::from_min_max(bar.min, pos2(x_at(v), bar.bottom()));
        painter.rect_filled(fill, 2.0, meter_color(v, warning, danger).gamma_multiply(0.75));
    }
    for (threshold, color) in [(warning, METER_WARNING), (danger, METER_DANGER)] {
        painter.vline(x_at(threshold), bar.y_range(), Stroke::new(1.0, color));
    }
    painter.text(bar.left_center() + vec2(4.0, 0.0), Align2::LEFT_CENTER, label, font, text);
}

fn draw_grid(painter: &Painter, rect: Rect) {
    let stroke = Stroke::new(1.0, GRID);
    for i in 1..4 {
        let y = rect.top() + rect.height() * i as f32 / 4.0;
        painter.hline(rect.x_range(), y, stroke);
    }
    for i in 1..6 {
        let x = rect.left() + rect.width() * i as f32 / 6.0;
        painter.vline(x, rect.y_range(), stroke);
    }
}

fn draw_segment(painter: &Painter, rect: Rect, points: &[Pos2], series: &Series) {
    if points.is_empty() {
        return;
    }
    if series.fill && points.len() >= 2 {
        let fill = series.color.gamma_multiply(0.25);
        let mut mesh = Mesh::default();
        for (i, p) in points.iter().enumerate() {
            mesh.colored_vertex(*p, fill);
            mesh.colored_vertex(pos2(p.x, rect.bottom()), fill);
            if i > 0 {
                let b = (i * 2) as u32;
                mesh.add_triangle(b - 2, b - 1, b);
                mesh.add_triangle(b - 1, b, b + 1);
            }
        }
        painter.add(Shape::mesh(mesh));
    }

    let stroke = Stroke::new(1.5, series.color);
    if points.len() == 1 {
        painter.circle_filled(points[0], 1.0, series.color);
    } else if series.dashed {
        painter.extend(Shape::dashed_line(points, stroke, 4.0, 3.0));
    } else {
        painter.add(Shape::line(points.to_vec(), stroke));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn meter_thresholds() {
        assert_eq!(meter_color(50.0, 80.0, 90.0), METER_OK);
        assert_eq!(meter_color(80.0, 80.0, 90.0), METER_WARNING);
        assert_eq!(meter_color(95.0, 80.0, 90.0), METER_DANGER);
    }
}
