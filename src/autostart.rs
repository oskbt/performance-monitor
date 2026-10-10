//! Windows 起動時の自動起動（HKCU の Run キーへの登録）。管理者権限は不要。

use windows::Win32::Foundation::ERROR_FILE_NOT_FOUND;
use windows::Win32::System::Registry::{
    HKEY_CURRENT_USER, REG_SZ, RRF_RT_REG_BINARY, RRF_RT_REG_SZ, RegDeleteKeyValueW,
    RegGetValueW, RegSetKeyValueW,
};
use windows::core::HSTRING;

const RUN_KEY: &str = r"Software\Microsoft\Windows\CurrentVersion\Run";
/// タスクマネージャーの「スタートアップ アプリ」で無効にした状態が記録されるキー。
const APPROVED_KEY: &str =
    r"Software\Microsoft\Windows\CurrentVersion\Explorer\StartupApproved\Run";
const VALUE_NAME: &str = "PerformanceMonitor";

/// 自動起動が有効か。Run キーに登録済みで、タスクマネージャーで無効化されていなければ true。
pub fn is_enabled() -> bool {
    read_run_value().is_some() && !disabled_in_task_manager()
}

/// 自動起動を有効・無効にする。有効にするときは現在の実行ファイルのパスを登録する。
pub fn set_enabled(enabled: bool) -> Result<(), Box<dyn std::error::Error>> {
    if enabled {
        let exe = std::env::current_exe()?;
        let command = command_line(&exe.to_string_lossy());
        let data: Vec<u16> = command.encode_utf16().chain(Some(0)).collect();
        unsafe {
            RegSetKeyValueW(
                HKEY_CURRENT_USER,
                &HSTRING::from(RUN_KEY),
                &HSTRING::from(VALUE_NAME),
                REG_SZ.0,
                Some(data.as_ptr().cast()),
                (data.len() * size_of::<u16>()) as u32,
            )
        }
        .ok()?;
        // タスクマネージャーで無効にされていた場合も、明示的に有効にしたので解除する
        delete_value(APPROVED_KEY)
    } else {
        delete_value(RUN_KEY)
    }
}

/// Run キーに登録するコマンドライン（パスに空白があっても動くよう引用符で囲む）。
fn command_line(exe: &str) -> String {
    format!("\"{exe}\"")
}

/// 値を削除する。もともと無い場合は成功扱い。
fn delete_value(key: &str) -> Result<(), Box<dyn std::error::Error>> {
    let status = unsafe {
        RegDeleteKeyValueW(HKEY_CURRENT_USER, &HSTRING::from(key), &HSTRING::from(VALUE_NAME))
    };
    if status == ERROR_FILE_NOT_FOUND {
        return Ok(());
    }
    Ok(status.ok()?)
}

fn read_run_value() -> Option<String> {
    let mut buf = [0u16; 1024];
    let mut size = size_of_val(&buf) as u32;
    let status = unsafe {
        RegGetValueW(
            HKEY_CURRENT_USER,
            &HSTRING::from(RUN_KEY),
            &HSTRING::from(VALUE_NAME),
            RRF_RT_REG_SZ,
            None,
            Some(buf.as_mut_ptr().cast()),
            Some(&mut size),
        )
    };
    if status.is_err() {
        return None;
    }
    let len = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
    Some(String::from_utf16_lossy(&buf[..len]))
}

/// StartupApproved の値の先頭バイトが奇数（0x03, 0x07 など）なら無効化されている。
fn disabled_in_task_manager() -> bool {
    let mut data = [0u8; 12];
    let mut size = data.len() as u32;
    let status = unsafe {
        RegGetValueW(
            HKEY_CURRENT_USER,
            &HSTRING::from(APPROVED_KEY),
            &HSTRING::from(VALUE_NAME),
            RRF_RT_REG_BINARY,
            None,
            Some(data.as_mut_ptr().cast()),
            Some(&mut size),
        )
    };
    status.is_ok() && size > 0 && data[0] & 1 == 1
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 実際のレジストリを書き換えるため通常は実行しない（`cargo test -- --ignored` で実行）。
    /// 終了時は元の状態に戻す。
    #[test]
    #[ignore]
    fn toggle_registry() {
        let original = is_enabled();
        set_enabled(true).unwrap();
        assert!(is_enabled());
        assert!(read_run_value().unwrap().contains(".exe"));
        set_enabled(false).unwrap();
        assert!(!is_enabled());
        set_enabled(original).unwrap();
    }

    #[test]
    fn quotes_path() {
        assert_eq!(
            command_line(r"C:\Program Files\pm\performance-monitor.exe"),
            r#""C:\Program Files\pm\performance-monitor.exe""#
        );
    }
}
