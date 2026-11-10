import { useCallback, useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import type { ClipItem } from "./types";
import Settings from "./Settings";
import "./App.css";

function relativeTime(ms: number): string {
  const secs = Math.round((Date.now() - ms) / 1000);
  if (secs < 60) return "just now";
  const mins = Math.round(secs / 60);
  if (mins < 60) return `${mins}m ago`;
  const hours = Math.round(mins / 60);
  if (hours < 24) return `${hours}h ago`;
  return `${Math.round(hours / 24)}d ago`;
}

function preview(item: ClipItem): string {
  const text = (item.content ?? "").replace(/\s+/g, " ").trim();
  return text.length > 220 ? `${text.slice(0, 220)}…` : text;
}

export default function App() {
  const [items, setItems] = useState<ClipItem[]>([]);
  const [query, setQuery] = useState("");
  const [selected, setSelected] = useState(0);
  const [status, setStatus] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [thumbs, setThumbs] = useState<Record<number, string>>({});
  const [showSettings, setShowSettings] = useState(false);
  const listRef = useRef<HTMLUListElement>(null);
  const inputRef = useRef<HTMLInputElement>(null);

  const refresh = useCallback(async (q: string) => {
    try {
      const rows = await invoke<ClipItem[]>("search_items", { query: q });
      setItems(rows);
      setError(null);
    } catch (e) {
      setError(String(e));
    }
  }, []);

  // Reload on every keystroke in the search box (debounced lightly).
  useEffect(() => {
    const t = setTimeout(() => void refresh(query), 80);
    return () => clearTimeout(t);
  }, [query, refresh]);

  // Reload whenever the watcher stores something new.
  useEffect(() => {
    const unlisten = listen("clip-added", () => void refresh(query));
    return () => {
      void unlisten.then((f) => f());
    };
  }, [query, refresh]);

  // The window is reused, so reset it each time the hotkey reveals it.
  useEffect(() => {
    const unlisten = getCurrentWindow().onFocusChanged(({ payload: focused }) => {
      if (focused) {
        setShowSettings(false);
        setQuery("");
        setSelected(0);
        void refresh("");
        inputRef.current?.focus();
      }
    });
    return () => {
      void unlisten.then((f) => f());
    };
  }, [refresh]);

  // Fetch a thumbnail for each image item we have not loaded yet.
  useEffect(() => {
    const missing = items.filter((i) => i.kind === "image" && !(i.id in thumbs));
    if (missing.length === 0) return;
    let cancelled = false;
    void (async () => {
      for (const item of missing) {
        try {
          const url = await invoke<string | null>("item_thumbnail", { id: item.id });
          if (!cancelled && url) {
            setThumbs((t) => ({ ...t, [item.id]: url }));
          }
        } catch {
          // A deleted or unreadable file just leaves the placeholder showing.
        }
      }
    })();
    return () => {
      cancelled = true;
    };
  }, [items, thumbs]);

  useEffect(() => {
    const unlisten = listen("open-settings", () => setShowSettings(true));
    return () => {
      void unlisten.then((f) => f());
    };
  }, []);

  // Keep the selection inside the current result set.
  useEffect(() => {
    setSelected((s) => Math.min(s, Math.max(items.length - 1, 0)));
  }, [items.length]);

  // Keep the highlighted row visible while arrowing through the list.
  useEffect(() => {
    listRef.current?.children[selected]?.scrollIntoView({ block: "nearest" });
  }, [selected]);

  const flash = (msg: string) => {
    setStatus(msg);
    setTimeout(() => setStatus(null), 1600);
  };

  // Enter pastes into the app that had focus. If auto-paste is disabled (or
  // unavailable, as on Wayland) the item is still on the clipboard.
  const paste = async (item: ClipItem) => {
    try {
      const pasted = await invoke<boolean>("paste_item", { id: item.id });
      if (!pasted) flash("Copied — press Ctrl+V to paste");
    } catch (e) {
      setError(String(e));
    }
  };

  const pin = async (item: ClipItem) => {
    try {
      await invoke("toggle_pin", { id: item.id });
      await refresh(query);
    } catch (e) {
      setError(String(e));
    }
  };

  const remove = async (item: ClipItem) => {
    try {
      await invoke("delete_item", { id: item.id });
      await refresh(query);
    } catch (e) {
      setError(String(e));
    }
  };

  const clearAll = async () => {
    try {
      await invoke("clear_all");
      await refresh(query);
      flash("History cleared (pinned items kept)");
    } catch (e) {
      setError(String(e));
    }
  };

  const onKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === "ArrowDown") {
      e.preventDefault();
      setSelected((s) => Math.min(s + 1, items.length - 1));
    } else if (e.key === "ArrowUp") {
      e.preventDefault();
      setSelected((s) => Math.max(s - 1, 0));
    } else if (e.key === "Enter") {
      e.preventDefault();
      const item = items[selected];
      if (item) void paste(item);
    } else if (e.key === "Escape") {
      e.preventDefault();
      if (showSettings) {
        setShowSettings(false);
      } else if (query) {
        setQuery("");
      } else {
        void invoke("hide_popup");
      }
    }
  };

  if (showSettings) {
    return (
      <div
        className="h-full bg-neutral-900 text-neutral-100"
        onKeyDown={onKeyDown}
        tabIndex={-1}
      >
        <Settings onClose={() => setShowSettings(false)} />
      </div>
    );
  }

  return (
    <div
      className="flex h-full flex-col bg-neutral-900 text-neutral-100"
      onKeyDown={onKeyDown}
    >
      <header className="flex items-center gap-2 border-b border-neutral-800 p-2">
        <input
          ref={inputRef}
          autoFocus
          value={query}
          onChange={(e) => setQuery(e.target.value)}
          placeholder="Search clipboard history…"
          className="min-w-0 flex-1 rounded-md bg-neutral-800 px-3 py-2 text-sm outline-none placeholder:text-neutral-500 focus:ring-2 focus:ring-sky-600"
        />
        <button
          onClick={() => setShowSettings(true)}
          className="shrink-0 rounded-md px-2 py-2 text-xs text-neutral-400 hover:bg-neutral-800 hover:text-neutral-100"
          title="Settings"
        >
          Settings
        </button>
        <button
          onClick={clearAll}
          className="shrink-0 rounded-md px-2 py-2 text-xs text-neutral-400 hover:bg-neutral-800 hover:text-neutral-100"
          title="Delete all unpinned items"
        >
          Clear
        </button>
      </header>

      {error && (
        <p className="border-b border-red-900 bg-red-950 px-3 py-2 text-xs text-red-200">
          {error}
        </p>
      )}

      <ul ref={listRef} className="min-h-0 flex-1 overflow-y-auto">
        {items.length === 0 && (
          <li className="p-6 text-center text-sm text-neutral-500">
            {query ? "No text matches that search." : "Copy something to get started."}
          </li>
        )}
        {items.map((item, i) => (
          <li
            key={item.id}
            onClick={() => setSelected(i)}
            onDoubleClick={() => void paste(item)}
            className={`group flex cursor-default items-start gap-2 border-b border-neutral-800/60 px-3 py-2 ${
              i === selected ? "bg-sky-950/60" : "hover:bg-neutral-800/50"
            }`}
          >
            {item.kind === "image" && (
              <div className="flex h-12 w-12 shrink-0 items-center justify-center overflow-hidden rounded bg-neutral-800">
                {thumbs[item.id] ? (
                  <img
                    src={thumbs[item.id]}
                    alt=""
                    className="max-h-full max-w-full object-contain"
                  />
                ) : (
                  <span className="text-[10px] text-neutral-500">IMG</span>
                )}
              </div>
            )}
            <div className="min-w-0 flex-1">
              <p className="truncate text-sm">
                {item.kind === "image" ? (
                  <span className="text-neutral-400">Image</span>
                ) : (
                  preview(item)
                )}
              </p>
              <p className="mt-0.5 text-[11px] text-neutral-500">
                {item.pinned && <span className="mr-1 text-amber-400">pinned</span>}
                {relativeTime(item.created_at)}
              </p>
            </div>
            <div className="flex shrink-0 gap-1 opacity-0 transition group-hover:opacity-100">
              <button
                onClick={() => void pin(item)}
                className="rounded px-1.5 py-0.5 text-[11px] text-neutral-300 hover:bg-neutral-700"
              >
                {item.pinned ? "Unpin" : "Pin"}
              </button>
              <button
                onClick={() => void remove(item)}
                className="rounded px-1.5 py-0.5 text-[11px] text-neutral-300 hover:bg-red-900"
              >
                Delete
              </button>
            </div>
          </li>
        ))}
      </ul>

      <footer className="border-t border-neutral-800 px-3 py-1.5 text-[11px] text-neutral-500">
        {status ?? "↑↓ move · Enter paste · Esc close"}
      </footer>
    </div>
  );
}
