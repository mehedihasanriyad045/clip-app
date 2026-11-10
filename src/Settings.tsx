import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";

interface AppSettings {
  auto_paste: boolean;
  history_limit: number;
  paused: boolean;
  autostart: boolean;
  hotkey: string;
  hotkey_active: boolean;
}

/// Turns a keydown into the accelerator string Tauri expects, e.g. "Ctrl+Alt+V".
/// Returns null until a non-modifier key is pressed alongside a modifier.
function accelFrom(e: React.KeyboardEvent): string | null {
  const parts: string[] = [];
  if (e.ctrlKey) parts.push("Ctrl");
  if (e.altKey) parts.push("Alt");
  if (e.shiftKey) parts.push("Shift");
  if (e.metaKey) parts.push("Super");

  const key = e.key;
  if (["Control", "Alt", "Shift", "Meta", "OS"].includes(key)) return null;
  // A bare key would swallow that key system-wide, so require a modifier.
  if (parts.length === 0) return null;

  let main: string;
  if (/^[a-z]$/i.test(key)) main = key.toUpperCase();
  else if (/^[0-9]$/.test(key)) main = key;
  else if (key.startsWith("F") && /^F\d{1,2}$/.test(key)) main = key;
  else if (key === " ") main = "Space";
  else if (key === "Insert" || key === "Delete" || key === "Home" || key === "End") main = key;
  else return null;

  parts.push(main);
  return parts.join("+");
}

export default function Settings({ onClose }: { onClose: () => void }) {
  const [s, setS] = useState<AppSettings | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    void (async () => {
      try {
        setS(await invoke<AppSettings>("get_settings"));
      } catch (e) {
        setError(String(e));
      }
    })();
  }, []);

  // Apply the change, then re-read so the UI reflects what was actually saved
  // rather than what we hoped it would be.
  const apply = async (cmd: string, args: Record<string, unknown>) => {
    try {
      await invoke(cmd, args);
      setS(await invoke<AppSettings>("get_settings"));
      setError(null);
    } catch (e) {
      setError(String(e));
    }
  };

  if (error && !s) {
    return (
      <div className="p-4 text-sm text-red-300">
        {error}
        <button onClick={onClose} className="mt-3 block text-neutral-400 underline">
          Back
        </button>
      </div>
    );
  }

  if (!s) return <div className="p-4 text-sm text-neutral-500">Loading…</div>;

  return (
    <div className="flex h-full flex-col">
      <header className="flex items-center justify-between border-b border-neutral-800 px-3 py-2">
        <h1 className="text-sm font-medium">Settings</h1>
        <button
          onClick={onClose}
          className="rounded px-2 py-1 text-xs text-neutral-400 hover:bg-neutral-800 hover:text-neutral-100"
        >
          Back
        </button>
      </header>

      <div className="flex-1 space-y-4 overflow-y-auto p-3 text-sm">
        <Toggle
          label="Paste automatically"
          hint="Sends Ctrl+V after picking an item. Off: the item is only copied."
          checked={s.auto_paste}
          onChange={(v) => void apply("set_auto_paste", { enabled: v })}
        />

        <Toggle
          label="Start with Windows"
          hint="Launches hidden in the tray when you sign in."
          checked={s.autostart}
          onChange={(v) => void apply("set_autostart", { enabled: v })}
        />

        <Toggle
          label="Pause recording"
          hint="Stops saving new clips. Existing history is kept."
          checked={s.paused}
          onChange={(v) => void apply("set_paused", { paused: v })}
        />

        <div>
          <span className="font-medium">Popup shortcut</span>
          <p className="mb-1 text-[11px] text-neutral-500">
            Click, then press the keys you want. Needs at least one modifier.
          </p>
          <HotkeyField
            value={s.hotkey}
            active={s.hotkey_active}
            onCapture={(accel) => void apply("set_hotkey", { accel })}
          />
        </div>

        <label className="block">
          <span className="font-medium">History limit</span>
          <p className="mb-1 text-[11px] text-neutral-500">
            Oldest unpinned items are removed past this count. Pinned items never count.
          </p>
          <input
            type="number"
            min={10}
            max={5000}
            defaultValue={s.history_limit}
            onBlur={(e) => {
              const n = Number(e.target.value);
              if (Number.isFinite(n) && n !== s.history_limit) {
                void apply("set_history_limit", { limit: Math.round(n) });
              }
            }}
            className="w-28 rounded bg-neutral-800 px-2 py-1 outline-none focus:ring-2 focus:ring-sky-600"
          />
        </label>

        <p className="border-t border-neutral-800 pt-3 text-[11px] text-neutral-500">
          Clips marked private by password managers are never recorded. History is
          stored locally and never leaves this device.
        </p>

        {error && <p className="text-[11px] text-red-300">{error}</p>}
      </div>
    </div>
  );
}

function HotkeyField({
  value,
  active,
  onCapture,
}: {
  value: string;
  active: boolean;
  onCapture: (accel: string) => void;
}) {
  const [capturing, setCapturing] = useState(false);

  return (
    <div className="flex items-center gap-2">
      <button
        onClick={() => setCapturing(true)}
        onBlur={() => setCapturing(false)}
        onKeyDown={(e) => {
          if (!capturing) return;
          e.preventDefault();
          e.stopPropagation();
          if (e.key === "Escape") {
            setCapturing(false);
            return;
          }
          const accel = accelFrom(e);
          if (accel) {
            setCapturing(false);
            onCapture(accel);
          }
        }}
        className={`rounded px-3 py-1 font-mono text-xs ${
          capturing
            ? "bg-sky-900 text-sky-100 ring-2 ring-sky-500"
            : "bg-neutral-800 text-neutral-100 hover:bg-neutral-700"
        }`}
      >
        {capturing ? "press keys…" : value}
      </button>
      {!active && (
        <span className="text-[11px] text-amber-400" title="Another app may already use it">
          not active
        </span>
      )}
    </div>
  );
}

function Toggle({
  label,
  hint,
  checked,
  onChange,
}: {
  label: string;
  hint: string;
  checked: boolean;
  onChange: (v: boolean) => void;
}) {
  return (
    <label className="flex cursor-pointer items-start gap-2">
      <input
        type="checkbox"
        checked={checked}
        onChange={(e) => onChange(e.target.checked)}
        className="mt-0.5 h-4 w-4 shrink-0 accent-sky-600"
      />
      <span>
        <span className="font-medium">{label}</span>
        <p className="text-[11px] text-neutral-500">{hint}</p>
      </span>
    </label>
  );
}
