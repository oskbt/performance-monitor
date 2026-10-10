//! 設定ファイル（%APPDATA%\performance-monitor\config.toml）の読み書き。

use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::path::PathBuf;

/// 選択可能な更新間隔（ミリ秒）。
pub const UPDATE_INTERVALS_MS: [u64; 4] = [500, 1000, 2000, 5000];

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    pub update_interval_ms: u64,
    pub history_seconds: u64,
    pub always_on_top: bool,
    /// ウィンドウのサイズと位置を固定する（ドラッグでの移動・リサイズを無効にする）。
    pub locked: bool,
    pub opacity: f32,
    pub compact: bool,
    pub show_per_core_cpu: bool,
    /// GPU 温度の警告しきい値 (℃)。メーターが黄色になる。
    pub gpu_temp_warning: f32,
    /// GPU 温度の危険しきい値 (℃)。メーターとタイルの枠が赤になる。
    pub gpu_temp_danger: f32,
    /// 非表示にしたタイルの ID。
    pub hidden: BTreeSet<String>,
    /// 前回終了時のウィンドウ位置・サイズ（物理ピクセル）。
    pub window: Option<WindowRect>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct WindowRect {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            update_interval_ms: 1000,
            history_seconds: 60,
            always_on_top: true,
            locked: false,
            opacity: 1.0,
            compact: false,
            show_per_core_cpu: true,
            gpu_temp_warning: 80.0,
            gpu_temp_danger: 90.0,
            hidden: BTreeSet::new(),
            window: None,
        }
    }
}

impl Config {
    fn path() -> Option<PathBuf> {
        let appdata = std::env::var_os("APPDATA")?;
        Some(PathBuf::from(appdata).join("performance-monitor").join("config.toml"))
    }

    /// 設定を読み込む。存在しない・壊れている場合は既定値を返す。
    pub fn load() -> Self {
        let Some(path) = Self::path() else {
            return Self::default();
        };
        let mut config = match std::fs::read_to_string(&path) {
            Ok(text) => toml::from_str(&text).unwrap_or_else(|e| {
                log::warn!("設定ファイルの解析に失敗しました ({}): {e}", path.display());
                Self::default()
            }),
            Err(_) => Self::default(),
        };
        config.sanitize();
        config
    }

    pub fn save(&self) {
        let Some(path) = Self::path() else { return };
        let result = (|| -> Result<(), Box<dyn std::error::Error>> {
            if let Some(dir) = path.parent() {
                std::fs::create_dir_all(dir)?;
            }
            std::fs::write(&path, toml::to_string_pretty(self)?)?;
            Ok(())
        })();
        if let Err(e) = result {
            log::warn!("設定ファイルの保存に失敗しました ({}): {e}", path.display());
        }
    }

    /// 範囲外の値を補正する。
    fn sanitize(&mut self) {
        if !UPDATE_INTERVALS_MS.contains(&self.update_interval_ms) {
            self.update_interval_ms = Self::default().update_interval_ms;
        }
        self.history_seconds = self.history_seconds.clamp(10, 600);
        self.opacity = self.opacity.clamp(0.3, 1.0);
        self.gpu_temp_danger = self.gpu_temp_danger.clamp(30.0, 150.0);
        self.gpu_temp_warning = self.gpu_temp_warning.clamp(30.0, self.gpu_temp_danger);
    }

    /// グラフに保持するサンプル数。
    pub fn history_len(&self) -> usize {
        ((self.history_seconds * 1000) / self.update_interval_ms).max(2) as usize
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip() {
        let mut config = Config::default();
        config.hidden.insert("disk:1".into());
        config.window = Some(WindowRect { x: -10, y: 20, width: 700, height: 400 });
        let text = toml::to_string_pretty(&config).unwrap();
        assert_eq!(toml::from_str::<Config>(&text).unwrap(), config);
    }

    #[test]
    fn missing_fields_use_defaults() {
        let config: Config = toml::from_str("compact = true").unwrap();
        assert!(config.compact);
        assert_eq!(config.update_interval_ms, 1000);
    }

    #[test]
    fn sanitize_out_of_range() {
        let mut config: Config =
            toml::from_str("update_interval_ms = 123\nopacity = 0.0").unwrap();
        config.sanitize();
        assert_eq!(config.update_interval_ms, 1000);
        assert_eq!(config.opacity, 0.3);

        let mut config: Config =
            toml::from_str("gpu_temp_warning = 95.0\ngpu_temp_danger = 85.0").unwrap();
        config.sanitize();
        assert_eq!(config.gpu_temp_warning, 85.0);
    }

    #[test]
    fn history_len() {
        let config = Config { update_interval_ms: 500, history_seconds: 60, ..Default::default() };
        assert_eq!(config.history_len(), 120);
    }
}
