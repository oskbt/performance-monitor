use super::registry;
use std::collections::{HashMap, HashSet};
use std::time::{Duration, Instant};
use windows::Win32::NetworkManagement::IpHelper::{
    FreeMibTable, GetIfTable2, IF_TYPE_SOFTWARE_LOOPBACK, IF_TYPE_TUNNEL, MIB_IF_ROW2,
    MIB_IF_TABLE2,
};
use windows::Win32::NetworkManagement::Ndis::IfOperStatusUp;

/// `MIB_IF_ROW2::InterfaceAndOperStatusFlags` の FilterInterface ビット。
const FILTER_INTERFACE: u8 = 1 << 1;
/// ネットワークアダプタのデバイスクラス。
const NET_CLASS_KEY: &str =
    r"SYSTEM\CurrentControlSet\Control\Class\{4d36e972-e325-11ce-bfc1-08002be10318}";
/// ドライバの Characteristics の NCF_HIDDEN（WAN Miniport など OS 内部用のアダプタ）。
const NCF_HIDDEN: u32 = 0x8;
/// 未知のアダプタが現れたときにレジストリを読み直す最短間隔。
const RELOAD_INTERVAL: Duration = Duration::from_secs(10);

#[derive(Debug, Clone)]
pub struct NetworkSample {
    /// インターフェース GUID。
    pub id: String,
    /// 接続名（`イーサネット`, `Wi-Fi` など）。
    pub name: String,
    /// アダプタの説明（製品名）。
    pub description: String,
    /// 送信 (ビット/秒)。
    pub sent: f64,
    /// 受信 (ビット/秒)。
    pub received: f64,
    /// リンク速度 (ビット/秒)。
    pub link_speed: u64,
}

pub struct NetworkCollector {
    /// 前回のオクテット数 (受信, 送信)。
    previous: HashMap<String, (u64, u64)>,
    previous_time: Instant,
    adapters: AdapterRegistry,
}

impl NetworkCollector {
    pub fn new() -> Self {
        Self {
            previous: HashMap::new(),
            previous_time: Instant::now(),
            adapters: AdapterRegistry::load(),
        }
    }

    pub fn sample(&mut self) -> Vec<NetworkSample> {
        let now = Instant::now();
        let elapsed = now.duration_since(self.previous_time).as_secs_f64().max(1e-3);
        self.previous_time = now;

        let mut current = HashMap::new();
        let mut samples = Vec::new();
        for row in interfaces() {
            if !is_target(&row) {
                continue;
            }
            let id = format!("{:?}", row.InterfaceGuid);
            if self.adapters.is_hidden(&id) {
                continue;
            }
            let (sent, received) = match self.previous.get(&id) {
                Some(&(prev_in, prev_out)) => (
                    row.OutOctets.saturating_sub(prev_out) as f64 * 8.0 / elapsed,
                    row.InOctets.saturating_sub(prev_in) as f64 * 8.0 / elapsed,
                ),
                None => (0.0, 0.0),
            };
            current.insert(id.clone(), (row.InOctets, row.OutOctets));
            samples.push(NetworkSample {
                id,
                name: wide_to_string(&row.Alias),
                description: wide_to_string(&row.Description),
                sent,
                received,
                link_speed: row.ReceiveLinkSpeed.max(row.TransmitLinkSpeed),
            });
        }
        self.previous = current;
        samples
    }
}

/// 表示対象のインターフェースか。接続中の物理・仮想アダプタを対象とし、
/// ループバック・トンネル疑似インターフェース（Teredo 等）・フィルタードライバは除く。
fn is_target(row: &MIB_IF_ROW2) -> bool {
    row.OperStatus == IfOperStatusUp
        && row.Type != IF_TYPE_SOFTWARE_LOOPBACK
        && row.Type != IF_TYPE_TUNNEL
        && row.InterfaceAndOperStatusFlags._bitfield & FILTER_INTERFACE == 0
}

/// レジストリから読んだアダプタのドライバ情報。
struct AdapterRegistry {
    /// 登録済みアダプタの GUID（大文字・波括弧なし）。
    known: HashSet<String>,
    hidden: HashSet<String>,
    loaded_at: Instant,
}

impl AdapterRegistry {
    fn load() -> Self {
        let mut known = HashSet::new();
        let mut hidden = HashSet::new();
        for name in registry::subkeys(NET_CLASS_KEY) {
            let path = format!(r"{NET_CLASS_KEY}\{name}");
            let Some(id) = registry::read_string(&path, "NetCfgInstanceId") else { continue };
            let id = normalize_guid(&id);
            if registry::read_dword(&path, "Characteristics").is_some_and(|c| c & NCF_HIDDEN != 0) {
                hidden.insert(id.clone());
            }
            known.insert(id);
        }
        Self { known, hidden, loaded_at: Instant::now() }
    }

    fn is_hidden(&mut self, id: &str) -> bool {
        let id = normalize_guid(id);
        if !self.known.contains(&id) && self.loaded_at.elapsed() >= RELOAD_INTERVAL {
            *self = Self::load();
        }
        self.hidden.contains(&id)
    }
}

/// `{2f1a...}` → `2F1A...`
fn normalize_guid(id: &str) -> String {
    id.trim_matches(|c| c == '{' || c == '}').to_ascii_uppercase()
}

fn interfaces() -> Vec<MIB_IF_ROW2> {
    let mut table: *mut MIB_IF_TABLE2 = std::ptr::null_mut();
    unsafe {
        if GetIfTable2(&mut table).is_err() || table.is_null() {
            return Vec::new();
        }
        let count = (*table).NumEntries as usize;
        let rows = std::slice::from_raw_parts((*table).Table.as_ptr(), count).to_vec();
        FreeMibTable(table as *const _);
        rows
    }
}

fn wide_to_string(s: &[u16]) -> String {
    let len = s.iter().position(|&c| c == 0).unwrap_or(s.len());
    String::from_utf16_lossy(&s[..len])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn guid_normalization() {
        let guid = windows::core::GUID::from_u128(0x2f1a0b3c_1234_5678_9abc_def012345678);
        assert_eq!(
            normalize_guid(&format!("{guid:?}")),
            normalize_guid("{2f1a0b3c-1234-5678-9abc-def012345678}")
        );
    }
}
