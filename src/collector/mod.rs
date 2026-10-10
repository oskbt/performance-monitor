//! 各種パフォーマンス値の収集。UI とは別スレッドで動く。

mod cpu;
mod disk;
mod memory;
mod pdh;

use crate::history::History;
use eframe::egui;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

pub use cpu::CpuSample;
pub use disk::DiskSample;
pub use memory::MemorySample;

/// ある時点の全項目の値。
#[derive(Debug, Default)]
pub struct Snapshot {
    pub cpu: Option<CpuSample>,
    pub memory: Option<MemorySample>,
    pub disks: Vec<DiskSample>,
}

struct Collectors {
    cpu: Option<cpu::CpuCollector>,
    disk: Option<disk::DiskCollector>,
}

impl Collectors {
    fn new() -> Self {
        Self {
            cpu: cpu::CpuCollector::new()
                .inspect_err(|e| log::error!("CPU 情報を取得できません: {e}"))
                .ok(),
            disk: disk::DiskCollector::new()
                .inspect_err(|e| log::error!("ディスク情報を取得できません: {e}"))
                .ok(),
        }
    }

    fn sample(&mut self) -> Snapshot {
        Snapshot {
            cpu: self.cpu.as_mut().and_then(|c| c.sample()),
            memory: memory::sample(),
            disks: self.disk.as_mut().map(|c| c.sample()).unwrap_or_default(),
        }
    }
}

/// 収集スレッドを起動する。`interval_ms` は実行中に変更できる。
pub fn spawn(ctx: egui::Context, history: Arc<Mutex<History>>, interval_ms: Arc<AtomicU64>) {
    std::thread::Builder::new()
        .name("collector".into())
        .spawn(move || {
            let mut collectors = Collectors::new();
            // レート系カウンタは 2 回目以降の取得で値が出るため、最初に 1 回空読みしておく。
            collectors.sample();
            let mut next = Instant::now();
            loop {
                next += Duration::from_millis(interval_ms.load(Ordering::Relaxed));
                let now = Instant::now();
                if next > now {
                    std::thread::sleep(next - now);
                } else {
                    // 大きく遅れた（スリープ復帰など）場合は基準を取り直す
                    next = now;
                }

                let snapshot = Arc::new(collectors.sample());
                history.lock().unwrap().push(snapshot);
                ctx.request_repaint();
            }
        })
        .expect("収集スレッドを起動できませんでした");
}
