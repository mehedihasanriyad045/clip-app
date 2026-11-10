use enigo::{Direction, Enigo, Key, Keyboard, Settings};
use std::time::Duration;

/// Time given to the OS to restore focus to the previously active window before
/// the keystroke is sent. Too short and the paste lands nowhere; too long and it
/// feels sluggish.
const FOCUS_SETTLE: Duration = Duration::from_millis(140);

/// Sends Ctrl+V to whatever window has focus, after a short delay.
///
/// Runs on its own thread: it must not block the Tauri event loop, because the
/// popup needs to finish hiding before focus returns to the target app.
pub fn send_paste_after_delay() {
    std::thread::spawn(|| {
        std::thread::sleep(FOCUS_SETTLE);
        if let Err(e) = send_ctrl_v() {
            // Wayland and some locked-down desktops refuse synthetic input. The
            // item is already on the clipboard, so the user can still paste it.
            eprintln!("auto-paste failed ({e}); item is on the clipboard instead");
        }
    });
}

fn send_ctrl_v() -> Result<(), Box<dyn std::error::Error>> {
    let mut enigo = Enigo::new(&Settings::default())?;
    enigo.key(Key::Control, Direction::Press)?;
    enigo.key(Key::Unicode('v'), Direction::Click)?;
    enigo.key(Key::Control, Direction::Release)?;
    Ok(())
}
