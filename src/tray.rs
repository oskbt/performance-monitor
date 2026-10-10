//! タスクトレイのアイコン。左クリックで表示/非表示、右クリックでメニュー。

use crate::icon;
use crate::winutil::{self, SendHwnd};
use eframe::egui;
use tray_icon::menu::{Menu, MenuEvent, MenuItem, PredefinedMenuItem};
use tray_icon::{Icon, MouseButton, MouseButtonState, TrayIcon, TrayIconBuilder, TrayIconEvent};
use windows::Win32::Foundation::HWND;

/// トレイアイコンを作る。戻り値を破棄するとアイコンが消えるので保持すること。
///
/// イベントはメインスレッドのメッセージループから呼ばれる。ウィンドウが隠れている間は
/// egui の更新処理が動かないことがあるため、表示切替は Win32 API で直接行う。
pub fn create(hwnd: HWND, ctx: egui::Context) -> Result<TrayIcon, Box<dyn std::error::Error>> {
    let toggle_item = MenuItem::new("表示 / 非表示", true, None);
    let quit_item = MenuItem::new("終了", true, None);
    let menu = Menu::new();
    menu.append_items(&[&toggle_item, &PredefinedMenuItem::separator(), &quit_item])?;

    let size = 32;
    let tray = TrayIconBuilder::new()
        .with_tooltip("Performance Monitor")
        .with_icon(Icon::from_rgba(icon::rgba(size), size, size)?)
        .with_menu(Box::new(menu))
        .with_menu_on_left_click(false)
        .build()?;

    let hwnd = SendHwnd::new(hwnd);
    let toggle = move |ctx: &egui::Context| {
        let hwnd = hwnd.get();
        if winutil::is_visible(hwnd) {
            winutil::hide(hwnd);
        } else {
            winutil::show(hwnd);
            ctx.request_repaint();
        }
    };

    let click_ctx = ctx.clone();
    TrayIconEvent::set_event_handler(Some(move |event| {
        if let TrayIconEvent::Click {
            button: MouseButton::Left,
            button_state: MouseButtonState::Up,
            ..
        } = event
        {
            toggle(&click_ctx);
        }
    }));

    let (toggle_id, quit_id) = (toggle_item.id().clone(), quit_item.id().clone());
    MenuEvent::set_event_handler(Some(move |event: MenuEvent| {
        if event.id == toggle_id {
            toggle(&ctx);
        } else if event.id == quit_id {
            winutil::request_close(hwnd.get());
        }
    }));

    Ok(tray)
}
