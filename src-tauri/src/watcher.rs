use crate::state::AppState;
use crate::{db, images, privacy};
use std::path::PathBuf;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::time::Duration;
use tauri::{AppHandle, Emitter, Manager};

/// How often the clipboard is polled. Polling is used on every platform for v1:
/// Windows has a native listener available, but one code path keeps the
/// behaviour identical on Windows, X11 and Wayland.
const POLL_INTERVAL: Duration = Duration::from_millis(500);

/// What was found on the clipboard this tick.
enum Found {
    Text(String),
    /// width, height, RGBA pixels
    Image(usize, usize, Vec<u8>),
}

/// True for clipboard "text" that is really binary data.
///
/// Copying a file in Explorer, or some archive tools, can leave raw bytes on the
/// clipboard as text. It decodes as a String but is unreadable in the list and
/// pointless to paste, so it is dropped rather than recorded.
fn looks_like_binary(text: &str) -> bool {
    if text.contains('\0') {
        return true;
    }

    // Tab, newline and carriage return are normal in copied text; other control
    // characters are not. A few may appear legitimately, so judge by proportion.
    let mut control = 0usize;
    let mut total = 0usize;
    for c in text.chars().take(2048) {
        total += 1;
        if c.is_control() && c != '\t' && c != '\n' && c != '\r' {
            control += 1;
        }
    }

    total > 0 && control * 100 / total >= 5
}

fn read_clipboard() -> Option<Found> {
    let mut cb = arboard::Clipboard::new().ok()?;

    // Text first: an image copied from a browser often carries a text/HTML
    // alternative, and text is the more useful of the two.
    if let Ok(text) = cb.get_text() {
        if !text.trim().is_empty() && !looks_like_binary(&text) {
            return Some(Found::Text(text));
        }
    }

    if let Ok(img) = cb.get_image() {
        return Some(Found::Image(img.width, img.height, img.bytes.into_owned()));
    }

    None
}

/// Starts the background clipboard poller. Emits `clip-added` to the frontend
/// whenever a new item is stored.
pub fn spawn(app: AppHandle) {
    std::thread::spawn(move || {
        let state: Arc<AppState> = app.state::<Arc<AppState>>().inner().clone();
        let app_data: PathBuf = match app.path().app_data_dir() {
            Ok(d) => d,
            Err(e) => {
                eprintln!("no app data dir, watcher not starting: {e}");
                return;
            }
        };
        let mut last_seen: Option<String> = None;

        loop {
            std::thread::sleep(POLL_INTERVAL);

            if state.paused.load(Ordering::Relaxed) {
                continue;
            }

            // A password manager (or anything else) asked not to be recorded.
            if privacy::should_skip_clipboard() {
                continue;
            }

            // Empty, locked or unsupported clipboard: nothing to do this tick.
            let Some(found) = read_clipboard() else {
                continue;
            };

            let hash = match &found {
                Found::Text(t) => db::hash_text(t),
                Found::Image(_, _, rgba) => images::hash_bytes(rgba),
            };

            // Unchanged since the previous tick.
            if last_seen.as_deref() == Some(hash.as_str()) {
                continue;
            }

            // Something the app itself wrote while pasting: mark it seen but do
            // not record it again.
            {
                let mut own = state.last_written.lock().unwrap();
                if own.as_deref() == Some(hash.as_str()) {
                    *own = None;
                    last_seen = Some(hash);
                    continue;
                }
            }

            last_seen = Some(hash.clone());

            // Images go to disk first; only the path is stored in the database.
            let (kind, content, image_path) = match &found {
                Found::Text(t) => ("text", Some(t.clone()), None),
                Found::Image(w, h, rgba) => {
                    match images::save_png(&app_data, *w, *h, rgba, &hash) {
                        Some(p) => ("image", None, Some(p.to_string_lossy().into_owned())),
                        None => {
                            eprintln!("skipped an image ({w}x{h}): unusable or too large");
                            continue;
                        }
                    }
                }
            };

            let stored = {
                let conn = state.db.lock().unwrap();
                let limit = db::history_limit(&conn);
                match db::insert(&conn, kind, content.as_deref(), image_path.as_deref(), &hash) {
                    Ok(id) => {
                        if let Ok(paths) = db::trim(&conn, limit) {
                            for p in paths {
                                let _ = std::fs::remove_file(p);
                            }
                        }
                        Some(id)
                    }
                    Err(e) => {
                        eprintln!("clip insert failed: {e}");
                        None
                    }
                }
            };

            if let Some(id) = stored {
                let _ = app.emit("clip-added", id);
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::looks_like_binary;

    #[test]
    fn accepts_ordinary_text() {
        assert!(!looks_like_binary("hello world"));
        assert!(!looks_like_binary("line one\nline two\ttabbed\r\n"));
        assert!(!looks_like_binary("SELECT * FROM users WHERE id = 1;"));
        assert!(!looks_like_binary("accents and emoji: cafe\u{301} \u{1F600}"));
    }

    #[test]
    fn rejects_nul_bytes() {
        assert!(looks_like_binary("abc\0def"));
    }

    #[test]
    fn rejects_archive_signature() {
        // The RAR header that turned up in the real history.
        assert!(looks_like_binary("Rar!\u{1a}\u{7}\u{1}\u{0}\u{1}\u{5}\u{9}\u{8}"));
    }

    #[test]
    fn tolerates_an_occasional_control_char() {
        let mostly_text = format!("{}\u{1}", "a".repeat(100));
        assert!(!looks_like_binary(&mostly_text));
    }

    #[test]
    fn ignores_empty_input() {
        assert!(!looks_like_binary(""));
    }
}
