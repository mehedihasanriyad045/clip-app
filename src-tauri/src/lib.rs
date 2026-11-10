mod commands;
mod db;
mod hotkey;
mod images;
mod paste;
mod popup;
mod privacy;
mod state;
mod tray;
mod watcher;

use state::AppState;
use std::sync::Arc;
use tauri::{Manager, WindowEvent};
use tauri_plugin_global_shortcut::ShortcutState;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            Some(vec!["--hidden"]),
        ))
        .plugin(
            tauri_plugin_global_shortcut::Builder::new()
                .with_handler(|app, shortcut, event| {
                    // Fire on key press only; the release event would toggle twice.
                    if event.state() != ShortcutState::Pressed {
                        return;
                    }
                    let state = app.state::<Arc<AppState>>();
                    if hotkey::is_current(&state, shortcut) {
                        let _ = popup::toggle(app);
                    }
                })
                .build(),
        )
        .setup(|app| {
            let dir = app.path().app_data_dir()?;
            std::fs::create_dir_all(&dir)?;
            let conn = db::open(&dir.join("clipapp.db"))?;

            app.manage(Arc::new(AppState::new(conn)));
            watcher::spawn(app.handle().clone());
            tray::build(app)?;

            // A failed registration must not kill the app: on Wayland the
            // shortcut is blocked, and the tray icon still works.
            let accel = {
                let state = app.state::<Arc<AppState>>();
                let conn = state.db.lock().unwrap();
                hotkey::stored(&conn)
            };
            if let Err(e) = hotkey::apply(app.handle(), &accel) {
                eprintln!("{e}");
                // Fall back to the platform default if a stored shortcut has
                // become unusable, so the popup stays reachable by keyboard.
                if accel != hotkey::DEFAULT {
                    if let Err(e) = hotkey::apply(app.handle(), hotkey::DEFAULT) {
                        eprintln!("{e}");
                    }
                }
            }

            Ok(())
        })
        .on_window_event(|win, event| match event {
            // Hide instead of quitting: the app lives in the tray.
            WindowEvent::CloseRequested { api, .. } => {
                api.prevent_close();
                let _ = win.hide();
            }
            // Clicking away dismisses the popup, like Win+V does.
            WindowEvent::Focused(false) => {
                let _ = win.hide();
            }
            _ => {}
        })
        .invoke_handler(tauri::generate_handler![
            commands::list_items,
            commands::search_items,
            commands::toggle_pin,
            commands::delete_item,
            commands::clear_all,
            commands::copy_item,
            commands::item_thumbnail,
            commands::set_paused,
            commands::is_paused,
            commands::hide_popup,
            commands::paste_item,
            commands::get_settings,
            commands::set_auto_paste,
            commands::set_history_limit,
            commands::set_autostart,
            commands::set_hotkey,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
