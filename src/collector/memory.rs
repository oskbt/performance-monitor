use windows::Win32::System::ProcessStatus::{GetPerformanceInfo, PERFORMANCE_INFORMATION};
use windows::Win32::System::SystemInformation::{GlobalMemoryStatusEx, MEMORYSTATUSEX};

#[derive(Debug, Clone)]
pub struct MemorySample {
    /// 使用中 (バイト)。
    pub used: u64,
    /// 搭載量 (バイト、OS が使える量)。
    pub total: u64,
    pub commit: Option<u64>,
    pub commit_limit: Option<u64>,
}

pub fn sample() -> Option<MemorySample> {
    let mut status =
        MEMORYSTATUSEX { dwLength: size_of::<MEMORYSTATUSEX>() as u32, ..Default::default() };
    unsafe { GlobalMemoryStatusEx(&mut status) }.ok()?;

    let mut perf = PERFORMANCE_INFORMATION::default();
    let commit = unsafe { GetPerformanceInfo(&mut perf, size_of::<PERFORMANCE_INFORMATION>() as u32) }
        .ok()
        .map(|_| {
            let page = perf.PageSize as u64;
            (perf.CommitTotal as u64 * page, perf.CommitLimit as u64 * page)
        });

    Some(MemorySample {
        used: status.ullTotalPhys - status.ullAvailPhys,
        total: status.ullTotalPhys,
        commit: commit.map(|c| c.0),
        commit_limit: commit.map(|c| c.1),
    })
}
