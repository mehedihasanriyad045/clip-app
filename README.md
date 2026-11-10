# clipapp

A clipboard history manager for Windows and Linux. Press a hotkey, pick
something you copied earlier, and it gets pasted into whatever you were typing
in. Everything stays on your machine.

It exists because Windows' own Win+V is fine but stops at the Windows version
you happen to have, and because the equivalent on Linux varies by desktop. One
codebase, same behaviour on both.

## What it does

- Keeps a history of copied text and images
- Opens next to your mouse cursor on Ctrl+Shift+V (Ctrl+Alt+V on Linux), and the
  shortcut is configurable
- Pastes the item you pick into the window that had focus, by copying it and
  sending Ctrl+V
- Pins items so they survive clearing and the history limit
- Searches text clips as you type
- Keyboard driven: arrows to move, Enter to paste, Esc to close
- Keeps 200 items by default, dropping the oldest unpinned ones first
- Skips a clip that is identical to one already saved, moving the old one back to
  the top instead
- Skips clips that password managers mark as private
- Lives in the system tray, where recording can be paused
- Can start when you sign in

Images are written to disk as PNG files and the database stores only the path, so
the database stays small.

## Status

Working and in daily use on Windows. The Linux build compiles and packages, but
has had less testing — see Limitations.

Not implemented yet: snippets, an emoji tab, automatic categories (link, code,
OTP), masking for sensitive values, rich text, theming, JSON export.

## Install

Download a build from the releases page, or build it yourself.

Windows: the `.msi` or the NSIS `.exe`. Both are unsigned, so SmartScreen will
warn you.

Linux: the `.AppImage` is self-contained apart from the system tray library.

```
chmod +x clipapp_0.1.0_amd64.AppImage
./clipapp_0.1.0_amd64.AppImage
```

The `.deb` is smaller but needs `libwebkit2gtk-4.1-0`, `libgtk-3-0` and
`libayatana-appindicator3-1` already installed.

## Building

Needs Rust, Node 20+, and on Windows the MSVC build tools with the Windows SDK.

```
npm install
npm run tauri dev     # development, with hot reload for the UI
npm run tauri build   # installers in src-tauri/target/release/bundle
```

On Linux, install the GTK and WebKitGTK development packages first:

```
sudo apt install libwebkit2gtk-4.1-dev build-essential curl file libssl-dev \
  libayatana-appindicator3-dev librsvg2-dev
```

`scripts/build-linux.sh` and `scripts/check-linux.sh` build and compile-check the
Linux version in a container, which is useful from a Windows machine.

## How it works

The Rust side polls the clipboard twice a second, hashes what it finds with
SHA-256 to recognise duplicates, and writes new items to SQLite. Polling is used
on every platform rather than the native Windows listener, so the behaviour is
the same everywhere.

```
src-tauri/src/
  watcher.rs    polls the clipboard and stores new items
  db.rs         SQLite queries
  privacy.rs    detects clips marked private by password managers
  images.rs     PNG files and thumbnails
  paste.rs      sends Ctrl+V after the popup hides
  popup.rs      positions the window at the cursor
  hotkey.rs     registers the configurable shortcut
  tray.rs       tray icon and menu
  commands.rs   the API the UI calls
src/
  App.tsx       the list, search and keyboard handling
  Settings.tsx  settings panel
```

Pasting is two steps: the item goes on the clipboard, the popup hides, and 140 ms
later a Ctrl+V keystroke is sent. The delay gives the window manager time to
return focus to the application you were in. If sending the keystroke fails, the
item is still on the clipboard and you can paste it yourself.

## Where your data is

Windows: `%APPDATA%\com.mehediriyad.clipapp\`
Linux: `~/.local/share/com.mehediriyad.clipapp/`

A `clipapp.db` SQLite file and an `images/` folder. Nothing is sent anywhere, and
there is no sync between machines.

## Limitations

The privacy check is platform-specific, because the conventions are. On Windows
it looks for the `ExcludeClipboardContentFromMonitorProcessing` and
`Clipboard Viewer Ignore` clipboard formats. On X11 it reads the clipboard's
advertised targets and looks for the KDE and KeePassXC password hints. **A pure
Wayland session has no equivalent, so nothing is detected there** — the app says
so on startup when it cannot reach an X11 display.

Other things worth knowing:

- Automatic pasting needs to synthesise a keypress, which Wayland generally does
  not allow. The fallback is that the item is on your clipboard.
- Global shortcuts are also blocked under Wayland. Bind a desktop-level shortcut
  to the executable, or use the tray icon.
- Searching only matches text. Image clips have no text to match, so they
  disappear while a search is active.
- The 140 ms paste delay is a fixed value that works on the machines tested so
  far. A slower machine may need more.
- Copying a file in a file manager can put odd data on the clipboard. Clips that
  look like binary are rejected, but the heuristic is a proportion of control
  characters rather than anything precise.

## License

MIT
