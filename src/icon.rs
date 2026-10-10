//! アプリのアイコン（暗い角丸四角に水色の折れ線グラフ）をプログラムで描く。

/// 背景色。
const BG: [f32; 3] = [0x1f as f32, 0x22 as f32, 0x27 as f32];
/// 折れ線と枠の色。
const LINE: [f32; 3] = [0x40 as f32, 0xc8 as f32, 0xd8 as f32];
/// 折れ線の頂点（0〜1 の相対座標、y は下向き）。
const POINTS: [(f32, f32); 6] =
    [(0.16, 0.72), (0.32, 0.56), (0.45, 0.64), (0.60, 0.30), (0.72, 0.46), (0.85, 0.24)];

/// `size`×`size` の RGBA（ストレートアルファ）画像。
pub fn rgba(size: u32) -> Vec<u8> {
    let s = size as f32;
    let radius = s * 0.22;
    let line_w = (s * 0.09).max(1.5);
    let points: Vec<(f32, f32)> = POINTS.iter().map(|&(x, y)| (x * s, y * s)).collect();

    let mut out = Vec::with_capacity((size * size * 4) as usize);
    for py in 0..size {
        for px in 0..size {
            let (x, y) = (px as f32 + 0.5, py as f32 + 0.5);

            // 角丸四角（外側は透明）と、その内側 1px の枠
            let d = rounded_rect_distance(x, y, s, radius);
            let shape = (0.5 - d).clamp(0.0, 1.0);
            let border = (0.5 - (d + 1.5).abs() + 0.5).clamp(0.0, 1.0) * 0.6;
            let mut color = mix(BG, LINE, border);

            // 折れ線
            let line_d = points
                .windows(2)
                .map(|seg| segment_distance(x, y, seg[0], seg[1]))
                .fold(f32::MAX, f32::min);
            let line = (line_w / 2.0 + 0.5 - line_d).clamp(0.0, 1.0);
            color = mix(color, LINE, line);

            out.extend(color.iter().map(|c| c.round() as u8));
            out.push((shape * 255.0).round() as u8);
        }
    }
    out
}

fn mix(a: [f32; 3], b: [f32; 3], t: f32) -> [f32; 3] {
    [0, 1, 2].map(|i| a[i] + (b[i] - a[i]) * t)
}

/// 角丸四角の符号付き距離（内側が負）。
fn rounded_rect_distance(x: f32, y: f32, size: f32, radius: f32) -> f32 {
    let half = size / 2.0;
    let qx = (x - half).abs() - (half - radius);
    let qy = (y - half).abs() - (half - radius);
    let outside = (qx.max(0.0).powi(2) + qy.max(0.0).powi(2)).sqrt();
    outside + qx.max(qy).min(0.0) - radius
}

fn segment_distance(x: f32, y: f32, a: (f32, f32), b: (f32, f32)) -> f32 {
    let (dx, dy) = (b.0 - a.0, b.1 - a.1);
    let t = (((x - a.0) * dx + (y - a.1) * dy) / (dx * dx + dy * dy)).clamp(0.0, 1.0);
    ((x - a.0 - t * dx).powi(2) + (y - a.1 - t * dy).powi(2)).sqrt()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn size_and_transparent_corners() {
        let img = rgba(32);
        assert_eq!(img.len(), 32 * 32 * 4);
        // 角は透明、中央は不透明
        assert_eq!(img[3], 0);
        let center = ((16 * 32 + 16) * 4 + 3) as usize;
        assert_eq!(img[center], 255);
    }
}
