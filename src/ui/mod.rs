//! 収集した履歴からタイルを組み立てる。

pub mod format;
pub mod graph;
pub mod tile;

use crate::collector::EngineKind;
use crate::config::Config;
use crate::history::History;
use eframe::egui::Color32;
use graph::Series;
use tile::{Graph, Tile};

pub const CPU_COLOR: Color32 = Color32::from_rgb(0x3d, 0xa5, 0xf4);
pub const MEMORY_COLOR: Color32 = Color32::from_rgb(0xb0, 0x6c, 0xf0);
const PAGE_OUT_COLOR: Color32 = Color32::from_rgb(0xe6, 0xb8, 0xf5);
pub const DISK_COLOR: Color32 = Color32::from_rgb(0x6c, 0xc0, 0x4a);
const DISK_WRITE_COLOR: Color32 = Color32::from_rgb(0xd4, 0xe1, 0x57);

pub const NETWORK_COLOR: Color32 = Color32::from_rgb(0xe0, 0x8e, 0x3c);
const NETWORK_SEND_COLOR: Color32 = Color32::from_rgb(0xf5, 0xd0, 0x8a);

pub const GPU_COLOR: Color32 = Color32::from_rgb(0x40, 0xc8, 0xd8);

/// GPU のエンジン種別ごとの線の色。
fn engine_color(kind: EngineKind) -> Color32 {
    match kind {
        EngineKind::ThreeD => GPU_COLOR,
        EngineKind::Copy => Color32::from_rgb(0xe8, 0xc5, 0x4a),
        EngineKind::VideoEncode => Color32::from_rgb(0xe8, 0x6a, 0x9a),
        EngineKind::VideoDecode => Color32::from_rgb(0xa0, 0x8c, 0xf0),
        EngineKind::VideoCodec => Color32::from_rgb(0x7c, 0xd6, 0x9c),
    }
}

/// ディスク速度グラフの縦軸の最小上限（1 MB/s）。
const DISK_MIN_SCALE: f64 = 1024.0 * 1024.0;
/// ネットワークグラフの縦軸の最小上限（100 Kbps）。
const NETWORK_MIN_SCALE: f64 = 100_000.0;

const NO_DATA: &str = "—";

/// 列ごとのタイル。左から CPU・メモリ・GPU・ディスク・ネットワーク（非表示設定は適用しない）。
pub fn build_columns(history: &History, config: &Config) -> Vec<Vec<Tile>> {
    let mut columns = vec![
        vec![cpu_tile(history, config.show_per_core_cpu)],
        vec![memory_tile(history), paging_tile(history)],
    ];
    let temp_thresholds = (config.gpu_temp_warning, config.gpu_temp_danger);
    match history.latest() {
        Some(latest) => {
            columns.push(
                latest.gpus.iter().map(|g| gpu_tile(history, g.luid, temp_thresholds)).collect(),
            );
            columns.push(latest.disks.iter().map(|d| disk_tile(history, d.index)).collect());
            columns.push(latest.networks.iter().map(|n| network_tile(history, &n.id)).collect());
        }
        None => columns.extend([Vec::new(), Vec::new(), Vec::new()]),
    }
    columns
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

fn cpu_tile(history: &History, per_thread: bool) -> Tile {
    let latest = history.latest().and_then(|s| s.cpu.as_ref());

    let mut graphs = vec![Graph::Lines {
        series: vec![Series::filled(history.series(|s| s.cpu.as_ref().map(|c| c.usage)), CPU_COLOR)],
        max: 100.0,
        scale: None,
    }];
    let threads = latest.map_or(0, |c| c.per_core.len());
    if per_thread && threads > 0 {
        graphs.push(Graph::Cells(
            (0..threads)
                .map(|i| history.series(|s| s.cpu.as_ref()?.per_core.get(i).copied()))
                .collect(),
        ));
    }

    Tile {
        id: "cpu".into(),
        title: "CPU".into(),
        value: latest.map_or(NO_DATA.into(), |c| format::percent(c.usage)),
        color: CPU_COLOR,
        graphs,
        legend: latest
            .and_then(|c| c.frequency_mhz)
            .map(|mhz| vec![(None, format!("速度 {}", format::ghz(mhz)))])
            .unwrap_or_default(),
        details: latest.map(|c| vec![c.name.to_string()]).unwrap_or_default(),
        alert: false,
        slim: false,
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
        alert: false,
        slim: false,
    }
}

fn paging_tile(history: &History) -> Tile {
    let latest = history.latest().and_then(|s| s.paging.as_ref());

    let usage = Graph::Lines {
        series: vec![Series::filled(
            history.series(|s| s.paging.as_ref()?.usage),
            MEMORY_COLOR,
        )],
        max: 100.0,
        scale: None,
    };
    let rates = rate_graph(
        history.series(|s| s.paging.as_ref().map(|p| p.input as f32)),
        history.series(|s| s.paging.as_ref().map(|p| p.output as f32)),
        (MEMORY_COLOR, PAGE_OUT_COLOR),
        DISK_MIN_SCALE,
        format::nice_bytes_ceiling,
        format::bytes_per_sec,
    );

    let mut details = Vec::new();
    if let Some(p) = latest {
        details.push(match p.usage {
            Some(usage) if p.pagefile_total > 0 => format!(
                "ページファイル {}",
                format::gb_pair(
                    (p.pagefile_total as f64 * usage as f64 / 100.0) as u64,
                    p.pagefile_total
                )
            ),
            _ => "ページファイルなし".into(),
        });
    }

    Tile {
        id: "paging".into(),
        title: "ページング".into(),
        value: latest.and_then(|p| p.usage).map_or(NO_DATA.into(), format::percent),
        color: MEMORY_COLOR,
        graphs: vec![usage, rates],
        legend: latest
            .map(|p| {
                vec![
                    (Some(MEMORY_COLOR), format!("イン {}", format::bytes_per_sec(p.input))),
                    (Some(PAGE_OUT_COLOR), format!("アウト {}", format::bytes_per_sec(p.output))),
                ]
            })
            .unwrap_or_default(),
        details,
        alert: false,
        slim: true,
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
        alert: false,
        slim: true,
    }
}

fn network_tile(history: &History, id: &str) -> Tile {
    let find = |s: &crate::collector::Snapshot| s.networks.iter().find(|n| n.id == id).cloned();
    let latest = history.latest().and_then(|s| find(s));

    let graph = rate_graph(
        history.series(|s| find(s).map(|n| n.received as f32)),
        history.series(|s| find(s).map(|n| n.sent as f32)),
        (NETWORK_COLOR, NETWORK_SEND_COLOR),
        NETWORK_MIN_SCALE,
        format::nice_ceiling,
        format::bits_per_sec,
    );

    let mut details = Vec::new();
    if let Some(n) = &latest {
        details.push(n.description.clone());
        if n.link_speed > 0 {
            details.push(format!("リンク速度 {}", format::bits_per_sec(n.link_speed as f64)));
        }
    }

    Tile {
        id: format!("net:{id}"),
        title: latest.as_ref().map_or_else(|| "ネットワーク".into(), |n| n.name.clone()),
        value: String::new(),
        color: NETWORK_COLOR,
        graphs: vec![graph],
        legend: latest
            .map(|n| {
                vec![
                    (Some(NETWORK_COLOR), format!("受信 {}", format::bits_per_sec(n.received))),
                    (Some(NETWORK_SEND_COLOR), format!("送信 {}", format::bits_per_sec(n.sent))),
                ]
            })
            .unwrap_or_default(),
        details,
        alert: false,
        slim: false,
    }
}

/// GPU 温度メーターの上限 (℃)。
const GPU_TEMP_MAX: f32 = 100.0;
/// ドライバが最大回転数を報告しない場合のファンメーターの上限 (RPM)。
const GPU_FAN_MAX_DEFAULT: f32 = 3000.0;

fn gpu_tile(history: &History, luid: (u32, u32), temp_thresholds: (f32, f32)) -> Tile {
    let find = |s: &crate::collector::Snapshot| s.gpus.iter().find(|g| g.luid == luid).cloned();
    let latest = history.latest().and_then(|s| find(s));
    let kinds: Vec<EngineKind> =
        latest.as_ref().map(|g| g.engines.iter().map(|(k, _)| *k).collect()).unwrap_or_default();

    let series = kinds
        .iter()
        .enumerate()
        .map(|(i, &kind)| {
            let values = history.series(|s| {
                find(s)?.engines.iter().find(|(k, _)| *k == kind).map(|(_, v)| *v)
            });
            if i == 0 {
                Series::filled(values, engine_color(kind))
            } else {
                Series::line(values, engine_color(kind))
            }
        })
        .collect();

    let mut details = Vec::new();
    if let Some(g) = &latest {
        if g.dedicated_total > 0 {
            details.push(format!(
                "専用メモリ {}",
                format::gb_pair(g.dedicated_used, g.dedicated_total)
            ));
        }
        details.push(format!("共有メモリ {}", format::gb_pair(g.shared_used, g.shared_total)));
    }

    Tile {
        id: format!("gpu:{:08x}_{:08x}", luid.0, luid.1),
        title: latest
            .as_ref()
            .map_or_else(|| "GPU".into(), |g| format!("GPU {} ({})", g.index, g.name)),
        value: latest.as_ref().map_or(NO_DATA.into(), |g| format::percent(g.usage)),
        color: GPU_COLOR,
        graphs: vec![
            Graph::Lines { series, max: 100.0, scale: None },
            Graph::Meter {
                label: "温度".into(),
                value: latest.as_ref().and_then(|g| g.temperature),
                max: GPU_TEMP_MAX,
                thresholds: Some(temp_thresholds),
                unit: "℃",
            },
            Graph::Meter {
                label: "ファン".into(),
                value: latest.as_ref().and_then(|g| g.fan_rpm).map(|rpm| rpm as f32),
                max: latest
                    .as_ref()
                    .and_then(|g| g.fan_max_rpm)
                    .map_or(GPU_FAN_MAX_DEFAULT, |max| max as f32),
                thresholds: None,
                unit: " RPM",
            },
        ],
        slim: false,
        alert: latest
            .as_ref()
            .and_then(|g| g.temperature)
            .is_some_and(|t| t >= temp_thresholds.1),
        legend: latest
            .map(|g| {
                g.engines
                    .iter()
                    .map(|&(k, v)| (Some(engine_color(k)), format!("{} {}", k.label(), format::percent(v))))
                    .collect()
            })
            .unwrap_or_default(),
        details,
    }
}
