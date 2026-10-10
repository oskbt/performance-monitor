//! 時系列の折れ線グラフ描画。

use eframe::egui::{Color32, Mesh, Painter, Pos2, Rect, Shape, Stroke, StrokeKind, pos2};

pub const GRAPH_BG: Color32 = Color32::from_rgb(0x0e, 0x10, 0x13);
const GRID: Color32 = Color32::from_rgb(0x26, 0x2a, 0x30);

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
    painter.rect_filled(rect, 0.0, GRAPH_BG);
    draw_grid(painter, rect);

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

/// 小さなグラフを格子状に並べる（論理プロセッサ別 CPU など）。
pub fn draw_cells(painter: &Painter, rect: Rect, cells: &[Vec<f32>], capacity: usize, color: Color32) {
    let n = cells.len().max(1);
    let aspect = (rect.width() / rect.height().max(1.0)).max(0.1);
    let cols = ((n as f32 * aspect).sqrt().ceil() as usize).clamp(1, n);
    let rows = n.div_ceil(cols);
    let gap = 2.0;
    let w = (rect.width() - gap * (cols - 1) as f32) / cols as f32;
    let h = (rect.height() - gap * (rows - 1) as f32) / rows as f32;

    for (i, values) in cells.iter().enumerate() {
        let (r, c) = (i / cols, i % cols);
        let min = rect.min + eframe::egui::vec2(c as f32 * (w + gap), r as f32 * (h + gap));
        let cell = Rect::from_min_size(min, eframe::egui::vec2(w, h));
        draw(
            painter,
            cell,
            &[Series::filled(values.clone(), color)],
            100.0,
            capacity,
            color.gamma_multiply(0.4),
        );
    }
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
