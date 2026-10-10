use super::pdh::{Counter, PdhError, Query};
use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct DiskSample {
    /// 物理ディスク番号。
    pub index: u32,
    /// 含まれるドライブレター（`"C: D:"`）。無ければ空。
    pub drives: String,
    /// アクティブな時間 (0〜100)。
    pub active: f32,
    /// 読み取り速度 (バイト/秒)。
    pub read: f64,
    /// 書き込み速度 (バイト/秒)。
    pub write: f64,
}

pub struct DiskCollector {
    query: Query,
    idle: Counter,
    read: Counter,
    write: Counter,
}

impl DiskCollector {
    pub fn new() -> Result<Self, PdhError> {
        let query = Query::new()?;
        let idle = query.add(r"\PhysicalDisk(*)\% Idle Time")?;
        let read = query.add(r"\PhysicalDisk(*)\Disk Read Bytes/sec")?;
        let write = query.add(r"\PhysicalDisk(*)\Disk Write Bytes/sec")?;
        query.collect()?;
        Ok(Self { query, idle, read, write })
    }

    pub fn sample(&mut self) -> Vec<DiskSample> {
        if self.query.collect().is_err() {
            return Vec::new();
        }
        let read: HashMap<_, _> = self.read.values().into_iter().collect();
        let write: HashMap<_, _> = self.write.values().into_iter().collect();

        let mut disks: Vec<DiskSample> = self
            .idle
            .values()
            .into_iter()
            .filter_map(|(name, idle)| {
                let (index, drives) = parse_instance(&name)?;
                Some(DiskSample {
                    index,
                    drives,
                    // タスクマネージャーの「アクティブな時間」は 100 - アイドル時間
                    active: (100.0 - idle).clamp(0.0, 100.0) as f32,
                    read: read.get(&name).copied().unwrap_or(0.0).max(0.0),
                    write: write.get(&name).copied().unwrap_or(0.0).max(0.0),
                })
            })
            .collect();
        disks.sort_by_key(|d| d.index);
        disks
    }
}

/// `"0 C: D:"` → `(0, "C: D:")`。`"_Total"` は None。
fn parse_instance(name: &str) -> Option<(u32, String)> {
    let mut parts = name.split_whitespace();
    let index = parts.next()?.parse().ok()?;
    Some((index, parts.collect::<Vec<_>>().join(" ")))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn instance_names() {
        assert_eq!(parse_instance("0 C:"), Some((0, "C:".into())));
        assert_eq!(parse_instance("1 D: E:"), Some((1, "D: E:".into())));
        assert_eq!(parse_instance("2"), Some((2, String::new())));
        assert_eq!(parse_instance("_Total"), None);
    }
}
