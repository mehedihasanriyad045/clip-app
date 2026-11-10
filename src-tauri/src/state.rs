use rusqlite::Connection;
use std::sync::atomic::AtomicBool;
use std::sync::Mutex;
use tauri_plugin_global_shortcut::Shortcut;

pub struct AppState {
    pub db: Mutex<Connection>,
    /// When true the watcher ignores clipboard changes (tray "pause recording").
    pub paused: AtomicBool,
    /// Hash of the value the app itself put on the clipboard, so pasting an old
    /// item does not re-record it as a new one.
    pub last_written: Mutex<Option<String>>,
    /// The shortcut currently registered for the popup.
    pub hotkey: Mutex<Option<Shortcut>>,
}

impl AppState {
    pub fn new(db: Connection) -> Self {
        Self {
            db: Mutex::new(db),
            paused: AtomicBool::new(false),
            last_written: Mutex::new(None),
            hotkey: Mutex::new(None),
        }
    }
}
