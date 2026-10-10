//! ウィンドウ位置の保存・復元など、Win32 API を直接使う処理。

use crate::config::WindowRect;
use raw_window_handle::{HasWindowHandle, RawWindowHandle};
use windows::Win32::Foundation::{HWND, LPARAM, RECT, WPARAM};
use windows::Win32::Graphics::Gdi::{MONITOR_DEFAULTTONULL, MonitorFromRect};
use windows::Win32::UI::WindowsAndMessaging::{
    GetWindowRect, IsIconic, IsWindowVisible, PostMessageW, SW_HIDE, SW_SHOW, SWP_NOACTIVATE,
    SWP_NOZORDER, SetForegroundWindow, SetWindowPos, ShowWindow, WM_CLOSE,
};

/// スレッド間で受け渡せるウィンドウハンドル（トレイのイベントハンドラから使う）。
#[derive(Clone, Copy)]
pub struct SendHwnd(isize);

impl SendHwnd {
    pub fn new(hwnd: HWND) -> Self {
        Self(hwnd.0 as isize)
    }

    pub fn get(self) -> HWND {
        HWND(self.0 as *mut _)
    }
}

pub fn is_visible(hwnd: HWND) -> bool {
    unsafe { IsWindowVisible(hwnd).as_bool() }
}

/// ウィンドウを表示して前面に出す。
pub fn show(hwnd: HWND) {
    unsafe {
        let _ = ShowWindow(hwnd, SW_SHOW);
        let _ = SetForegroundWindow(hwnd);
    }
}

/// ウィンドウを隠す（トレイに常駐した状態になる）。
pub fn hide(hwnd: HWND) {
    unsafe {
        let _ = ShowWindow(hwnd, SW_HIDE);
    }
}

/// 通常の終了処理（設定の保存など）を通してアプリを閉じる。
/// 隠れている間は eframe が閉じる要求を処理しないため、いったん表示してから送る。
pub fn request_close(hwnd: HWND) {
    unsafe {
        let _ = ShowWindow(hwnd, SW_SHOW);
        let _ = PostMessageW(Some(hwnd), WM_CLOSE, WPARAM(0), LPARAM(0));
    }
}

pub fn hwnd_of(handle: &impl HasWindowHandle) -> Option<HWND> {
    match handle.window_handle().ok()?.as_raw() {
        RawWindowHandle::Win32(h) => Some(HWND(h.hwnd.get() as *mut _)),
        _ => None,
    }
}

/// 現在のウィンドウ位置・サイズ（物理ピクセル）。最小化中は None。
pub fn window_rect(hwnd: HWND) -> Option<WindowRect> {
    unsafe {
        if IsIconic(hwnd).as_bool() {
            return None;
        }
        let mut r = RECT::default();
        GetWindowRect(hwnd, &mut r).ok()?;
        Some(WindowRect { x: r.left, y: r.top, width: r.right - r.left, height: r.bottom - r.top })
    }
}

/// 保存された位置へウィンドウを移動する。どのモニターにも掛からない位置なら何もしない。
pub fn restore_window_rect(hwnd: HWND, rect: WindowRect) -> bool {
    if rect.width < 100 || rect.height < 60 {
        return false;
    }
    let r = RECT {
        left: rect.x,
        top: rect.y,
        right: rect.x + rect.width,
        bottom: rect.y + rect.height,
    };
    unsafe {
        if MonitorFromRect(&r, MONITOR_DEFAULTTONULL).is_invalid() {
            return false;
        }
        SetWindowPos(
            hwnd,
            None,
            rect.x,
            rect.y,
            rect.width,
            rect.height,
            SWP_NOZORDER | SWP_NOACTIVATE,
        )
        .is_ok()
    }
}
