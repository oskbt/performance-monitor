//! 収集した履歴からタイルを組み立てる。

pub mod format;
pub mod graph;
pub mod tile;

use crate::history::History;
use eframe::egui::Color32;
use graph::Series;
use tile::{Graph, Tile};

pub const CPU_COLOR: Color32 = Color32::from_rgb(0x3d, 0xa5, 0xf4);
pub const MEMORY_COLOR: Color32 = Color32::from_rgb(0xb0, 0x6c, 0xf0);
pub const DISK_COLOR: Color32 = Color32::from_rgb(0x6c, 0xc0, 0x4a);
const DISK_WRITE_COLOR: Color32 = Color32::from_rgb(0xd4, 0xe1, 0x57);

/// ディスク速度グラフの縦軸の最小上限（1 MB/s）。
const DISK_MIN_SCALE: f64 = 1024.0 * 1024.0;

const NO_DATA: &str = "—";

/// 表示する全タイル（非表示設定は適用しない）。
pub fn build_tiles(history: &History, per_core_cpu: bool) -> Vec<Tile> {
    let mut tiles = vec![cpu_tile(history, per_core_cpu), memory_tile(history)];
    if let Some(latest) = history.latest() {
        tiles.extend(latest.disks.iter().map(|d| disk_tile(history, d.index)));
    }
    tiles
}

/// 2 系列の速度グラフ（1 系列目を塗りつぶし、2 系列目を破線）。
/// 縦軸は表示期間中の最大値に合わせて自動調整する。
fn rate_graph(
    first: Vec<f32>,
    second: Vec<f32>,
    colors: (Color32, Color32),
    min_scale: f64,
    ceiling: fn(f64) -> f64,
    label: fn(f64) -> String,
) -> Graph {
    let peak = first
        .iter()
        .chain(&second)
        .filter(|v| v.is_finite())
        .fold(0.0f32, |a, &b| a.max(b));
    let max = ceiling((peak as f64).max(min_scale));
    Graph::Lines {
        series: vec![Series::filled(first, colors.0), Series::dashed(second, colors.1)],
        max: max as f32,
        scale: Some(label(max)),
    }
}

fn cpu_tile(history: &History, per_core: bool) -> Tile {
    let latest = history.latest().and_then(|s| s.cpu.as_ref());

    let cores = latest.map_or(0, |c| c.per_core.len());
    let graph = if per_core && cores > 0 {
        Graph::Cells(
            (0..cores)
                .map(|i| history.series(|s| s.cpu.as_ref()?.per_core.get(i).copied()))
                .collect(),
        )
    } else {
        Graph::Lines {
            series: vec![Series::filled(
                history.series(|s| s.cpu.as_ref().map(|c| c.usage)),
                CPU_COLOR,
            )],
            max: 100.0,
            scale: None,
        }
    };

    Tile {
        id: "cpu".into(),
        title: "CPU".into(),
        value: latest.map_or(NO_DATA.into(), |c| format::percent(c.usage)),
        color: CPU_COLOR,
        graphs: vec![graph],
        legend: latest
            .and_then(|c| c.frequency_mhz)
            .map(|mhz| vec![(None, format!("速度 {}", format::ghz(mhz)))])
            .unwrap_or_default(),
        details: latest.map(|c| vec![c.name.to_string()]).unwrap_or_default(),
    }
}

fn memory_tile(history: &History) -> Tile {
    let latest = history.latest().and_then(|s| s.memory.as_ref());
    let percent = |m: &crate::collector::MemorySample| {
        (m.used as f64 / m.total.max(1) as f64 * 100.0) as f32
    };

    let mut details = Vec::new();
    if let Some((commit, limit)) = latest.and_then(|m| Some((m.commit?, m.commit_limit?))) {
        details.push(format!("コミット済み {}", format::gb_pair(commit, limit)));
    }

    Tile {
        id: "memory".into(),
        title: "メモリ".into(),
        value: latest.map_or(NO_DATA.into(), |m| format::percent(percent(m))),
        color: MEMORY_COLOR,
        graphs: vec![Graph::Lines {
            series: vec![Series::filled(
                history.series(|s| s.memory.as_ref().map(percent)),
                MEMORY_COLOR,
            )],
            max: 100.0,
            scale: None,
        }],
        legend: latest
            .map(|m| vec![(None, format!("使用中 {}", format::gb_pair(m.used, m.total)))])
            .unwrap_or_default(),
        details,
    }
}

fn disk_tile(history: &History, index: u32) -> Tile {
    let find = move |s: &crate::collector::Snapshot| s.disks.iter().find(|d| d.index == index).cloned();
    let latest = history.latest().and_then(|s| find(s));

    let title = match latest.as_ref().map(|d| d.drives.as_str()) {
        Some(drives) if !drives.is_empty() => format!("ディスク {index} ({drives})"),
        _ => format!("ディスク {index}"),
    };
    let active = Graph::Lines {
        series: vec![Series::filled(history.series(|s| find(s).map(|d| d.active)), DISK_COLOR)],
        max: 100.0,
        scale: None,
    };
    let rates = rate_graph(
        history.series(|s| find(s).map(|d| d.read as f32)),
        history.series(|s| find(s).map(|d| d.write as f32)),
        (DISK_COLOR, DISK_WRITE_COLOR),
        DISK_MIN_SCALE,
        format::nice_bytes_ceiling,
        format::bytes_per_sec,
    );

    Tile {
        id: format!("disk:{index}"),
        title,
        value: latest.as_ref().map_or(NO_DATA.into(), |d| format::percent(d.active)),
        color: DISK_COLOR,
        graphs: vec![active, rates],
        legend: latest
            .map(|d| {
                vec![
                    (Some(DISK_COLOR), format!("読み {}", format::bytes_per_sec(d.read))),
                    (Some(DISK_WRITE_COLOR), format!("書き {}", format::bytes_per_sec(d.write))),
                ]
            })
            .unwrap_or_default(),
        details: Vec::new(),
    }
}
