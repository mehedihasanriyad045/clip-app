use tauri::{AppHandle, Manager, PhysicalPosition, WebviewWindow};

/// Gap between the mouse cursor and the window edge, in physical pixels.
const CURSOR_MARGIN: i32 = 12;

pub fn main_window(app: &AppHandle) -> Option<WebviewWindow> {
    app.get_webview_window("main")
}

/// Shows the popup next to the mouse cursor, keeping it fully on the monitor
/// the cursor is on.
pub fn show_at_cursor(app: &AppHandle) -> tauri::Result<()> {
    let Some(win) = main_window(app) else {
        return Ok(());
    };

    if let Ok(cursor) = win.cursor_position() {
        let size = win.outer_size()?;
        let mut x = cursor.x as i32 + CURSOR_MARGIN;
        let mut y = cursor.y as i32 + CURSOR_MARGIN;

        // Flip to the other side of the cursor when the window would spill off
        // the monitor's work area.
        if let Ok(Some(monitor)) = win.monitor_from_point(cursor.x, cursor.y) {
            let m_pos = monitor.position();
            let m_size = monitor.size();
            let right = m_pos.x + m_size.width as i32;
            let bottom = m_pos.y + m_size.height as i32;

            if x + size.width as i32 > right {
                x = cursor.x as i32 - size.width as i32 - CURSOR_MARGIN;
            }
            if y + size.height as i32 > bottom {
                y = cursor.y as i32 - size.height as i32 - CURSOR_MARGIN;
            }
            x = x.max(m_pos.x);
            y = y.max(m_pos.y);
        }

        let _ = win.set_position(PhysicalPosition::new(x, y));
    }

    win.show()?;
    win.set_focus()?;
    Ok(())
}

/// Hotkey behaviour: visible popup hides, hidden popup appears at the cursor.
pub fn toggle(app: &AppHandle) -> tauri::Result<()> {
    let Some(win) = main_window(app) else {
        return Ok(());
    };
    if win.is_visible().unwrap_or(false) {
        win.hide()?;
        Ok(())
    } else {
        show_at_cursor(app)
    }
}
