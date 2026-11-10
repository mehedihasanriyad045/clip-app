use crate::popup;
use crate::state::AppState;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use tauri::menu::{CheckMenuItem, Menu, MenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{App, Emitter, Manager, Wry};

pub const PAUSE_ID: &str = "pause";
pub const SHOW_ID: &str = "show";
pub const SETTINGS_ID: &str = "settings";
pub const CLEAR_ID: &str = "clear";
pub const QUIT_ID: &str = "quit";

pub fn build(app: &App) -> tauri::Result<()> {
    let handle = app.handle();

    let pause = CheckMenuItem::with_id(handle, PAUSE_ID, "Pause recording", true, false, None::<&str>)?;
    let show = MenuItem::with_id(handle, SHOW_ID, "Show history", true, None::<&str>)?;
    let settings = MenuItem::with_id(handle, SETTINGS_ID, "Settings", true, None::<&str>)?;
    let clear = MenuItem::with_id(handle, CLEAR_ID, "Clear history", true, None::<&str>)?;
    let quit = MenuItem::with_id(handle, QUIT_ID, "Quit", true, None::<&str>)?;

    let menu = Menu::with_items(handle, &[&show, &pause, &settings, &clear, &quit])?;

    TrayIconBuilder::<Wry>::with_id("main-tray")
        .icon(app.default_window_icon().unwrap().clone())
        .tooltip("Clipboard History — Ctrl+Shift+V")
        .menu(&menu)
        // The menu is for the right button only, so a left click can open the popup.
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| {
            let state = app.state::<Arc<AppState>>();
            match event.id().as_ref() {
                SHOW_ID => {
                    let _ = popup::show_at_cursor(app);
                }
                SETTINGS_ID => {
                    let _ = popup::show_at_cursor(app);
                    let _ = app.emit("open-settings", ());
                }
                PAUSE_ID => {
                    let now = !state.paused.load(Ordering::Relaxed);
                    state.paused.store(now, Ordering::Relaxed);
                }
                CLEAR_ID => {
                    if let Ok(conn) = state.db.lock() {
                        if let Ok(paths) = crate::db::clear_unpinned(&conn) {
                            for p in paths {
                                let _ = std::fs::remove_file(p);
                            }
                        }
                    }
                }
                QUIT_ID => app.exit(0),
                _ => {}
            }
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                let _ = popup::show_at_cursor(tray.app_handle());
            }
        })
        .build(app)?;

    Ok(())
}
