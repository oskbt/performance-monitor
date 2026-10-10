//! 数値を表示用の文字列に変換する。

const KIB: f64 = 1024.0;
const MIB: f64 = KIB * 1024.0;
const GIB: f64 = MIB * 1024.0;

pub fn percent(v: f32) -> String {
    format!("{v:.0}%")
}

pub fn gb(bytes: u64) -> String {
    format!("{:.1} GB", bytes as f64 / GIB)
}

/// 容量の組 `"12.3 / 31.8 GB"`。
pub fn gb_pair(used: u64, total: u64) -> String {
    format!("{:.1} / {:.1} GB", used as f64 / GIB, total as f64 / GIB)
}

pub fn ghz(mhz: f32) -> String {
    format!("{:.2} GHz", mhz / 1000.0)
}

/// バイト毎秒（1024 単位、タスクマネージャーのディスク表示と同じ）。
pub fn bytes_per_sec(v: f64) -> String {
    if v < MIB {
        format!("{:.0} KB/s", v / KIB)
    } else if v < GIB {
        format!("{:.1} MB/s", v / MIB)
    } else {
        format!("{:.2} GB/s", v / GIB)
    }
}

/// ビット毎秒（1000 単位、タスクマネージャーのネットワーク表示と同じ）。
pub fn bits_per_sec(v: f64) -> String {
    if v < 1e6 {
        format!("{:.0} Kbps", v / 1e3)
    } else if v < 1e9 {
        format!("{:.1} Mbps", v / 1e6)
    } else {
        format!("{:.2} Gbps", v / 1e9)
    }
}

/// `v` 以上で最小の「きりのいい数」（1, 2, 5 × 10^n）。グラフの縦軸上限に使う。
pub fn nice_ceiling(v: f64) -> f64 {
    if !(v > 0.0) || !v.is_finite() {
        return 1.0;
    }
    let exp = v.log10().floor();
    let base = 10f64.powf(exp);
    for m in [1.0, 2.0, 5.0, 10.0] {
        // 浮動小数点誤差で 1 段上がらないよう少し余裕を持たせる
        if v <= m * base * (1.0 + 1e-9) {
            return m * base;
        }
    }
    10.0 * base
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bytes_units() {
        assert_eq!(bytes_per_sec(0.0), "0 KB/s");
        assert_eq!(bytes_per_sec(512.0 * KIB), "512 KB/s");
        assert_eq!(bytes_per_sec(1.5 * MIB), "1.5 MB/s");
        assert_eq!(bytes_per_sec(2.0 * GIB), "2.00 GB/s");
    }

    #[test]
    fn bits_units() {
        assert_eq!(bits_per_sec(0.0), "0 Kbps");
        assert_eq!(bits_per_sec(8_000.0), "8 Kbps");
        assert_eq!(bits_per_sec(35_200_000.0), "35.2 Mbps");
        assert_eq!(bits_per_sec(1_000_000_000.0), "1.00 Gbps");
    }

    #[test]
    fn gb_values() {
        assert_eq!(gb((12.3 * GIB) as u64), "12.3 GB");
        assert_eq!(gb_pair((1.5 * GIB) as u64, (32.0 * GIB) as u64), "1.5 / 32.0 GB");
    }

    #[test]
    fn nice() {
        assert_eq!(nice_ceiling(0.0), 1.0);
        assert_eq!(nice_ceiling(f64::NAN), 1.0);
        assert_eq!(nice_ceiling(1.0), 1.0);
        assert_eq!(nice_ceiling(1.1), 2.0);
        assert_eq!(nice_ceiling(3.0), 5.0);
        assert_eq!(nice_ceiling(7.0), 10.0);
        assert_eq!(nice_ceiling(100.0), 100.0);
        assert_eq!(nice_ceiling(120_000.0), 200_000.0);
        assert_eq!(nice_ceiling(0.03), 0.05);
    }
}
