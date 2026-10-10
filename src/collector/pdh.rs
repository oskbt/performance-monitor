//! PDH（Performance Data Helper）の薄いラッパー。
//!
//! カウンタパスは英語名で指定する（`PdhAddEnglishCounterW`）ため、OS の表示言語に依存しない。

use windows::Win32::System::Performance::{
    PDH_CSTATUS_NEW_DATA, PDH_CSTATUS_VALID_DATA, PDH_FMT, PDH_FMT_COUNTERVALUE,
    PDH_FMT_COUNTERVALUE_ITEM_W, PDH_FMT_DOUBLE, PDH_HCOUNTER, PDH_HQUERY, PDH_MORE_DATA,
    PdhAddEnglishCounterW, PdhCloseQuery, PdhCollectQueryData, PdhGetFormattedCounterArrayW,
    PdhGetFormattedCounterValue, PdhOpenQueryW,
};
use windows::core::{HSTRING, PCWSTR};

/// 100% を超える値もそのまま返す（`% Processor Utility` などはターボ時に 100 を超える）。
const PDH_FMT_NOCAP100: u32 = 0x8000;
const FORMAT: PDH_FMT = PDH_FMT(PDH_FMT_DOUBLE.0 | PDH_FMT_NOCAP100);

#[derive(Debug)]
pub struct PdhError(pub u32);

impl std::fmt::Display for PdhError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "PDH error 0x{:08X}", self.0)
    }
}

impl std::error::Error for PdhError {}

fn check(status: u32) -> Result<(), PdhError> {
    if status == 0 { Ok(()) } else { Err(PdhError(status)) }
}

fn is_valid(cstatus: u32) -> bool {
    cstatus == PDH_CSTATUS_VALID_DATA || cstatus == PDH_CSTATUS_NEW_DATA
}

pub struct Query {
    handle: PDH_HQUERY,
}

impl Query {
    pub fn new() -> Result<Self, PdhError> {
        let mut handle = PDH_HQUERY::default();
        check(unsafe { PdhOpenQueryW(PCWSTR::null(), 0, &mut handle) })?;
        Ok(Self { handle })
    }

    /// カウンタを追加する。ワイルドカード（`(*)`）を含むパスも指定できる。
    pub fn add(&self, path: &str) -> Result<Counter, PdhError> {
        let mut handle = PDH_HCOUNTER::default();
        let path = HSTRING::from(path);
        check(unsafe { PdhAddEnglishCounterW(self.handle, &path, 0, &mut handle) })?;
        Ok(Counter { handle })
    }

    pub fn collect(&self) -> Result<(), PdhError> {
        check(unsafe { PdhCollectQueryData(self.handle) })
    }
}

impl Drop for Query {
    fn drop(&mut self) {
        unsafe {
            PdhCloseQuery(self.handle);
        }
    }
}

/// `Query` に属するカウンタ。`Query` より長く使ってはいけない。
pub struct Counter {
    handle: PDH_HCOUNTER,
}

impl Counter {
    /// 単一インスタンスのカウンタ値。
    pub fn value(&self) -> Option<f64> {
        let mut value = PDH_FMT_COUNTERVALUE::default();
        let status = unsafe { PdhGetFormattedCounterValue(self.handle, FORMAT, None, &mut value) };
        if status != 0 || !is_valid(value.CStatus) {
            return None;
        }
        Some(unsafe { value.Anonymous.doubleValue })
    }

    /// ワイルドカードカウンタの全インスタンスの値（インスタンス名, 値）。
    /// 取得できなかったインスタンスは含まない。
    pub fn values(&self) -> Vec<(String, f64)> {
        let mut size = 0u32;
        let mut count = 0u32;
        let status = unsafe {
            PdhGetFormattedCounterArrayW(self.handle, FORMAT, &mut size, &mut count, None)
        };
        if status != PDH_MORE_DATA || size == 0 {
            return Vec::new();
        }

        // バッファには構造体の配列の後ろにインスタンス名の文字列が詰められる。
        // アラインメントを満たすため u64 単位で確保する。
        let mut buf = vec![0u64; (size as usize).div_ceil(8)];
        let items = buf.as_mut_ptr() as *mut PDH_FMT_COUNTERVALUE_ITEM_W;
        let status = unsafe {
            PdhGetFormattedCounterArrayW(self.handle, FORMAT, &mut size, &mut count, Some(items))
        };
        if status != 0 {
            return Vec::new();
        }

        let items = unsafe { std::slice::from_raw_parts(items, count as usize) };
        items
            .iter()
            .filter(|item| is_valid(item.FmtValue.CStatus))
            .filter_map(|item| {
                let name = unsafe { item.szName.to_string() }.ok()?;
                Some((name, unsafe { item.FmtValue.Anonymous.doubleValue }))
            })
            .collect()
    }
}
