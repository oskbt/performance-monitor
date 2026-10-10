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

const NO_DATA: &str = "—";

/// 表示する全タイル（非表示設定は適用しない）。
pub fn build_tiles(history: &History, per_core_cpu: bool) -> Vec<Tile> {
    vec![cpu_tile(history, per_core_cpu), memory_tile(history)]
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
        graph,
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
        graph: Graph::Lines {
            series: vec![Series::filled(
                history.series(|s| s.memory.as_ref().map(percent)),
                MEMORY_COLOR,
            )],
            max: 100.0,
            scale: None,
        },
        legend: latest
            .map(|m| vec![(None, format!("使用中 {}", format::gb_pair(m.used, m.total)))])
            .unwrap_or_default(),
        details,
    }
}
