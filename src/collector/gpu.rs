use super::pdh::{Counter, PdhError, Query};
use std::collections::HashMap;
use windows::Win32::Graphics::Dxgi::{
    CreateDXGIFactory1, DXGI_ADAPTER_FLAG_SOFTWARE, IDXGIAdapter1, IDXGIFactory1,
};

/// アダプタの LUID (HighPart, LowPart)。
pub type Luid = (u32, u32);

/// グラフに表示するエンジン種別（タスクマネージャーの既定表示に合わせる）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum EngineKind {
    ThreeD,
    Copy,
    VideoEncode,
    VideoDecode,
    VideoCodec,
}

impl EngineKind {
    /// PDH の engtype（`3D`, `VideoDecode`, `Video Codec 0` など）から分類する。
    fn classify(engtype: &str) -> Option<Self> {
        let t = engtype.to_ascii_lowercase();
        if t == "3d" {
            Some(Self::ThreeD)
        } else if t == "copy" {
            Some(Self::Copy)
        } else if t.contains("encode") {
            Some(Self::VideoEncode)
        } else if t.contains("decode") {
            Some(Self::VideoDecode)
        } else if t.contains("codec") {
            Some(Self::VideoCodec)
        } else {
            None
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::ThreeD => "3D",
            Self::Copy => "Copy",
            Self::VideoEncode => "Enc",
            Self::VideoDecode => "Dec",
            Self::VideoCodec => "Codec",
        }
    }
}

#[derive(Debug, Clone)]
pub struct GpuSample {
    pub luid: Luid,
    /// DXGI の列挙順の番号（ソフトウェアアダプタを除く）。
    pub index: u32,
    pub name: String,
    /// 全体の使用率 (0〜100)。全エンジンの最大値。
    pub usage: f32,
    /// エンジン種別ごとの使用率 (0〜100)。存在する種別のみ、`EngineKind` 順。
    pub engines: Vec<(EngineKind, f32)>,
    pub dedicated_used: u64,
    pub dedicated_total: u64,
    pub shared_used: u64,
    pub shared_total: u64,
}

struct Adapter {
    luid: Luid,
    name: String,
    dedicated_total: u64,
    shared_total: u64,
}

pub struct GpuCollector {
    query: Query,
    utilization: Counter,
    dedicated: Counter,
    shared: Counter,
    adapters: Vec<Adapter>,
}

impl GpuCollector {
    pub fn new() -> Result<Self, Box<dyn std::error::Error>> {
        let adapters = enumerate_adapters()?;
        let query = Query::new()?;
        let utilization = query.add(r"\GPU Engine(*)\Utilization Percentage")?;
        let dedicated = query.add(r"\GPU Adapter Memory(*)\Dedicated Usage")?;
        let shared = query.add(r"\GPU Adapter Memory(*)\Shared Usage")?;
        query.collect().map_err(|e: PdhError| Box::new(e) as Box<dyn std::error::Error>)?;
        Ok(Self { query, utilization, dedicated, shared, adapters })
    }

    pub fn sample(&mut self) -> Vec<GpuSample> {
        if self.query.collect().is_err() {
            return Vec::new();
        }

        // エンジン（LUID, phys, eng）ごとに全プロセスの使用率を合計する
        let mut per_engine: HashMap<(Luid, u32, u32), (Option<EngineKind>, f64)> = HashMap::new();
        for (name, value) in self.utilization.values() {
            let Some(engine) = parse_engine_instance(&name) else { continue };
            let entry = per_engine
                .entry((engine.luid, engine.phys, engine.eng))
                .or_insert((EngineKind::classify(engine.engtype), 0.0));
            entry.1 += value.max(0.0);
        }

        let dedicated = sum_by_luid(self.dedicated.values());
        let shared = sum_by_luid(self.shared.values());

        self.adapters
            .iter()
            .enumerate()
            .map(|(index, adapter)| {
                let mut usage = 0.0f64;
                let mut engines: HashMap<EngineKind, f64> = HashMap::new();
                for (&(luid, _, _), &(kind, value)) in &per_engine {
                    if luid != adapter.luid {
                        continue;
                    }
                    let value = value.min(100.0);
                    usage = usage.max(value);
                    if let Some(kind) = kind {
                        let e = engines.entry(kind).or_default();
                        *e = e.max(value);
                    }
                }
                let mut engines: Vec<_> =
                    engines.into_iter().map(|(k, v)| (k, v as f32)).collect();
                engines.sort_by_key(|(k, _)| *k);

                GpuSample {
                    luid: adapter.luid,
                    index: index as u32,
                    name: adapter.name.clone(),
                    usage: usage as f32,
                    engines,
                    dedicated_used: dedicated.get(&adapter.luid).copied().unwrap_or(0),
                    dedicated_total: adapter.dedicated_total,
                    shared_used: shared.get(&adapter.luid).copied().unwrap_or(0),
                    shared_total: adapter.shared_total,
                }
            })
            .collect()
    }
}

fn sum_by_luid(values: Vec<(String, f64)>) -> HashMap<Luid, u64> {
    let mut map = HashMap::new();
    for (name, value) in values {
        if let Some((luid, _)) = parse_luid(&name) {
            *map.entry(luid).or_default() += value.max(0.0) as u64;
        }
    }
    map
}

/// ハードウェア GPU を DXGI の列挙順に取得する（Microsoft Basic Render Driver などは除外）。
fn enumerate_adapters() -> windows::core::Result<Vec<Adapter>> {
    let factory: IDXGIFactory1 = unsafe { CreateDXGIFactory1()? };
    let mut adapters = Vec::new();
    for i in 0.. {
        let Ok(adapter): windows::core::Result<IDXGIAdapter1> = (unsafe { factory.EnumAdapters1(i) })
        else {
            break;
        };
        let desc = unsafe { adapter.GetDesc1()? };
        if desc.Flags & DXGI_ADAPTER_FLAG_SOFTWARE.0 as u32 != 0 {
            continue;
        }
        let len = desc.Description.iter().position(|&c| c == 0).unwrap_or(desc.Description.len());
        let luid = (desc.AdapterLuid.HighPart as u32, desc.AdapterLuid.LowPart);
        if adapters.iter().any(|a: &Adapter| a.luid == luid) {
            continue;
        }
        adapters.push(Adapter {
            luid,
            name: String::from_utf16_lossy(&desc.Description[..len]).trim().to_string(),
            dedicated_total: desc.DedicatedVideoMemory as u64,
            shared_total: desc.SharedSystemMemory as u64,
        });
    }
    Ok(adapters)
}

/// `"luid_0x00000000_0x0001306E_phys_0..."` の LUID と、それ以降の文字列。
fn parse_luid(name: &str) -> Option<(Luid, &str)> {
    let rest = &name[name.find("luid_")? + 5..];
    let mut parts = rest.splitn(3, '_');
    let high = u32::from_str_radix(parts.next()?.strip_prefix("0x")?, 16).ok()?;
    let low = u32::from_str_radix(parts.next()?.strip_prefix("0x")?, 16).ok()?;
    Some(((high, low), parts.next().unwrap_or("")))
}

#[derive(Debug, PartialEq)]
struct EngineInstance<'a> {
    luid: Luid,
    phys: u32,
    eng: u32,
    engtype: &'a str,
}

/// `"pid_1234_luid_0x00000000_0x0001306E_phys_0_eng_3_engtype_VideoDecode"` を分解する。
fn parse_engine_instance(name: &str) -> Option<EngineInstance<'_>> {
    let (luid, rest) = parse_luid(name)?;
    let mut parts = rest.splitn(5, '_');
    (parts.next()? == "phys").then_some(())?;
    let phys = parts.next()?.parse().ok()?;
    (parts.next()? == "eng").then_some(())?;
    let eng = parts.next()?.parse().ok()?;
    let engtype = parts.next()?.strip_prefix("engtype_")?;
    Some(EngineInstance { luid, phys, eng, engtype })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn engine_instance() {
        assert_eq!(
            parse_engine_instance("pid_10660_luid_0x00000000_0x0001306E_phys_0_eng_12_engtype_Video Decode 1"),
            Some(EngineInstance {
                luid: (0, 0x1306e),
                phys: 0,
                eng: 12,
                engtype: "Video Decode 1"
            })
        );
        assert_eq!(
            parse_engine_instance("pid_4_luid_0x00000001_0x0000ABCD_phys_1_eng_0_engtype_Security_1")
                .map(|e| (e.luid, e.phys, e.engtype)),
            Some(((1, 0xabcd), 1, "Security_1"))
        );
        assert_eq!(parse_engine_instance("_Total"), None);
    }

    #[test]
    fn memory_instance() {
        assert_eq!(
            parse_luid("luid_0x00000000_0x00011b45_phys_0").map(|(l, _)| l),
            Some((0, 0x11b45))
        );
    }

    #[test]
    fn engine_kinds() {
        assert_eq!(EngineKind::classify("3D"), Some(EngineKind::ThreeD));
        assert_eq!(EngineKind::classify("High Priority 3D"), None);
        assert_eq!(EngineKind::classify("Copy"), Some(EngineKind::Copy));
        assert_eq!(EngineKind::classify("VideoEncode"), Some(EngineKind::VideoEncode));
        assert_eq!(EngineKind::classify("Video Decode 1"), Some(EngineKind::VideoDecode));
        assert_eq!(EngineKind::classify("Video Codec 0"), Some(EngineKind::VideoCodec));
        assert_eq!(EngineKind::classify("Compute_0"), None);
    }
}
