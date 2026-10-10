use super::pdh::{Counter, PdhError, Query};
use super::registry;
use std::sync::Arc;

#[derive(Debug, Clone)]
pub struct CpuSample {
    pub name: Arc<str>,
    /// 全体の使用率 (0〜100)。
    pub usage: f32,
    /// 論理プロセッサごとの使用率 (0〜100)。
    pub per_core: Vec<f32>,
    /// 現在のクロック (MHz)。
    pub frequency_mhz: Option<f32>,
}

pub struct CpuCollector {
    query: Query,
    name: Arc<str>,
    total: Counter,
    per_core: Counter,
    base_frequency: Option<Counter>,
    performance: Option<Counter>,
}

impl CpuCollector {
    pub fn new() -> Result<Self, PdhError> {
        let query = Query::new()?;
        // タスクマネージャーと同じ "% Processor Utility" を使う。古い OS では "% Processor Time" で代用。
        let (total, per_core) = match query.add(r"\Processor Information(_Total)\% Processor Utility")
        {
            Ok(total) => (total, query.add(r"\Processor Information(*)\% Processor Utility")?),
            Err(_) => (
                query.add(r"\Processor Information(_Total)\% Processor Time")?,
                query.add(r"\Processor Information(*)\% Processor Time")?,
            ),
        };
        let base_frequency = query.add(r"\Processor Information(_Total)\Processor Frequency").ok();
        let performance =
            query.add(r"\Processor Information(_Total)\% Processor Performance").ok();
        query.collect()?;

        Ok(Self {
            query,
            name: processor_name().unwrap_or_else(|| "CPU".into()).into(),
            total,
            per_core,
            base_frequency,
            performance,
        })
    }

    pub fn sample(&mut self) -> Option<CpuSample> {
        self.query.collect().ok()?;
        let usage = self.total.value()?.clamp(0.0, 100.0) as f32;

        let mut per_core: Vec<((u32, u32), f32)> = self
            .per_core
            .values()
            .into_iter()
            .filter_map(|(name, v)| Some((parse_core_instance(&name)?, v.clamp(0.0, 100.0) as f32)))
            .collect();
        per_core.sort_by_key(|(key, _)| *key);

        let frequency_mhz = match (&self.base_frequency, &self.performance) {
            (Some(base), Some(perf)) => Some((base.value()? * perf.value()? / 100.0) as f32),
            _ => None,
        };

        Some(CpuSample {
            name: self.name.clone(),
            usage,
            per_core: per_core.into_iter().map(|(_, v)| v).collect(),
            frequency_mhz,
        })
    }
}

/// `"0,3"` → `(0, 3)`。`"_Total"` や `"0,_Total"` は None。
fn parse_core_instance(name: &str) -> Option<(u32, u32)> {
    let (group, index) = name.split_once(',')?;
    Some((group.parse().ok()?, index.parse().ok()?))
}

fn processor_name() -> Option<String> {
    let name = registry::read_string(
        r"HARDWARE\DESCRIPTION\System\CentralProcessor\0",
        "ProcessorNameString",
    )?;
    let name = name.trim();
    (!name.is_empty()).then(|| name.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn core_instance() {
        assert_eq!(parse_core_instance("0,3"), Some((0, 3)));
        assert_eq!(parse_core_instance("1,12"), Some((1, 12)));
        assert_eq!(parse_core_instance("0,_Total"), None);
        assert_eq!(parse_core_instance("_Total"), None);
    }
}
