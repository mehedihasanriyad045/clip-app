//! Detects clipboard content that applications have marked as private.
//!
//! Password managers advertise extra clipboard data so clipboard history tools
//! can opt out. Honouring it is the difference between a clipboard manager that
//! is safe to run and one that quietly archives every password you copy.
//!
//! The conventions are platform-specific: Windows uses registered clipboard
//! formats, X11 uses selection targets. There is no Wayland equivalent.

/// True when the current clipboard contents should not be recorded.
#[cfg(windows)]
pub fn should_skip_clipboard() -> bool {
    use windows::core::w;
    use windows::Win32::System::DataExchange::{
        IsClipboardFormatAvailable, RegisterClipboardFormatW,
    };

    // Set by apps that opt out of Windows clipboard history (the format Windows
    // itself honours), and the older convention KeePass and others use.
    let names = [
        w!("ExcludeClipboardContentFromMonitorProcessing"),
        w!("Clipboard Viewer Ignore"),
    ];

    for name in names {
        // SAFETY: both calls take a null-terminated wide string and a format id,
        // and neither writes through a pointer we own.
        unsafe {
            let format = RegisterClipboardFormatW(name);
            if format != 0 && IsClipboardFormatAvailable(format).is_ok() {
                return true;
            }
        }
    }

    false
}

#[cfg(target_os = "linux")]
pub fn should_skip_clipboard() -> bool {
    linux::should_skip()
}

/// Other platforms have no convention to honour.
#[cfg(not(any(windows, target_os = "linux")))]
pub fn should_skip_clipboard() -> bool {
    false
}

#[cfg(target_os = "linux")]
mod linux {
    use std::cell::RefCell;
    use std::time::{Duration, Instant};
    use x11rb::connection::Connection;
    use x11rb::protocol::xproto::{
        AtomEnum, ConnectionExt, CreateWindowAux, EventMask, WindowClass,
    };
    use x11rb::protocol::Event;
    use x11rb::rust_connection::RustConnection;

    /// Selection targets that mark the clipboard as holding a secret.
    ///
    /// `x-kde-passwordManagerHint` is the KDE convention, which KeePassXC and
    /// Bitwarden's desktop app also set. The others are seen in the wild from
    /// password managers following the same idea.
    const PRIVATE_TARGETS: [&str; 3] = [
        "x-kde-passwordManagerHint",
        "x-keepassxc-passwordManagerHint",
        "SECRET",
    ];

    /// Giving up quickly matters: this runs on every poll, and a selection owner
    /// that never answers must not stall the watcher.
    const REPLY_TIMEOUT: Duration = Duration::from_millis(120);

    struct Probe {
        conn: RustConnection,
        window: u32,
        clipboard: u32,
        targets: u32,
        property: u32,
    }

    impl Probe {
        fn new() -> Option<Self> {
            let (conn, screen_num) = x11rb::connect(None).ok()?;
            let root = conn.setup().roots[screen_num].root;
            let visual = conn.setup().roots[screen_num].root_visual;

            // A 1x1 unmapped window is enough to own a property and receive the
            // SelectionNotify reply.
            let window = conn.generate_id().ok()?;
            conn.create_window(
                x11rb::COPY_DEPTH_FROM_PARENT,
                window,
                root,
                0,
                0,
                1,
                1,
                0,
                WindowClass::INPUT_OUTPUT,
                visual,
                &CreateWindowAux::new().event_mask(EventMask::PROPERTY_CHANGE),
            )
            .ok()?;

            let intern = |name: &str| -> Option<u32> {
                Some(
                    conn.intern_atom(false, name.as_bytes())
                        .ok()?
                        .reply()
                        .ok()?
                        .atom,
                )
            };

            let clipboard = intern("CLIPBOARD")?;
            let targets = intern("TARGETS")?;
            let property = intern("CLIPAPP_TARGETS")?;
            conn.flush().ok()?;

            Some(Probe {
                conn,
                window,
                clipboard,
                targets,
                property,
            })
        }

        /// Asks the clipboard owner which formats it offers, and looks for a
        /// password hint among them.
        fn is_private(&self) -> Option<bool> {
            self.conn
                .convert_selection(
                    self.window,
                    self.clipboard,
                    self.targets,
                    self.property,
                    x11rb::CURRENT_TIME,
                )
                .ok()?;
            self.conn.flush().ok()?;

            let deadline = Instant::now() + REPLY_TIMEOUT;
            loop {
                if Instant::now() > deadline {
                    // No owner, or an owner that did not answer: nothing to skip.
                    return Some(false);
                }
                match self.conn.poll_for_event().ok()? {
                    Some(Event::SelectionNotify(event)) => {
                        if event.property == 0 {
                            return Some(false);
                        }
                        break;
                    }
                    // Not our reply; keep waiting.
                    Some(_) => continue,
                    None => std::thread::sleep(Duration::from_millis(5)),
                }
            }

            let reply = self
                .conn
                .get_property(true, self.window, self.property, AtomEnum::ATOM, 0, 1024)
                .ok()?
                .reply()
                .ok()?;

            let atoms: Vec<u32> = reply.value32()?.collect();
            for atom in atoms {
                let Ok(cookie) = self.conn.get_atom_name(atom) else {
                    continue;
                };
                let Ok(name) = cookie.reply() else {
                    continue;
                };
                let name = String::from_utf8_lossy(&name.name);
                if PRIVATE_TARGETS.iter().any(|t| name.eq_ignore_ascii_case(t)) {
                    return Some(true);
                }
            }

            Some(false)
        }
    }

    thread_local! {
        /// Built once per thread. The outer Option distinguishes "not tried yet"
        /// from the inner None, meaning X11 is unavailable and should not be
        /// retried on every poll.
        static PROBE: RefCell<Option<Option<Probe>>> = const { RefCell::new(None) };
    }

    pub fn should_skip() -> bool {
        PROBE.with(|cell| {
            let mut slot = cell.borrow_mut();
            if slot.is_none() {
                let probe = Probe::new();
                if probe.is_none() {
                    // Wayland without XWayland, or no display at all. Said once,
                    // because it is the user's cue that the protection is off.
                    eprintln!(
                        "no X11 display: clipboard privacy hints cannot be read, \
                         so password managers will not be detected"
                    );
                }
                *slot = Some(probe);
            }

            match slot.as_ref().and_then(|p| p.as_ref()) {
                Some(probe) => probe.is_private().unwrap_or(false),
                None => false,
            }
        })
    }
}
