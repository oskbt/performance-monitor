use super::pdh::{Counter, PdhError, Query};
use windows::Win32::System::ProcessStatus::{GetPerformanceInfo, PERFORMANCE_INFORMATION};

#[derive(Debug, Clone)]
pub struct PagingSample {
    /// ページファイルの使用率 (0〜100)。ページファイルが無い場合は None。
    pub usage: Option<f32>,
    /// ページファイルの合計サイズ (バイト)。
    pub pagefile_total: u64,
    /// ページイン（ディスクからの読み込み、バイト/秒）。
    pub input: f64,
    /// ページアウト（ディスクへの書き出し、バイト/秒）。
    pub output: f64,
}

pub struct PagingCollector {
    query: Query,
    usage: Counter,
    input: Counter,
    output: Counter,
}

impl PagingCollector {
    pub fn new() -> Result<Self, PdhError> {
        let query = Query::new()?;
        let usage = query.add(r"\Paging File(_Total)\% Usage")?;
        let input = query.add(r"\Memory\Pages Input/sec")?;
        let output = query.add(r"\Memory\Pages Output/sec")?;
        query.collect()?;
        Ok(Self { query, usage, input, output })
    }

    pub fn sample(&mut self) -> Option<PagingSample> {
        self.query.collect().ok()?;

        let mut perf = PERFORMANCE_INFORMATION::default();
        unsafe { GetPerformanceInfo(&mut perf, size_of::<PERFORMANCE_INFORMATION>() as u32) }
            .ok()?;
        let page = perf.PageSize as u64;
        // コミット上限 = 物理メモリ + ページファイル
        let pagefile_total = (perf.CommitLimit as u64).saturating_sub(perf.PhysicalTotal as u64) * page;

        Some(PagingSample {
            usage: self.usage.value().map(|v| v.clamp(0.0, 100.0) as f32),
            pagefile_total,
            input: self.input.value().unwrap_or(0.0).max(0.0) * page as f64,
            output: self.output.value().unwrap_or(0.0).max(0.0) * page as f64,
        })
    }
}
