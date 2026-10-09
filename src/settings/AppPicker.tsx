import { useEffect, useRef, useState, type KeyboardEvent } from "react";
import type { App } from "../api.ts";
import { highlight } from "../panel/Toolbar.tsx";
import { usePrefs } from "../prefs.tsx";
import { nextIndex } from "./dropdownKeys.ts";

/** Search box with the panel's `@app` suggestion popup, for picking an app to exclude. */
export function AppPicker({ apps, onPick }: { apps: App[]; onPick: (app: App) => void }) {
  const { t } = usePrefs();
  const [query, setQuery] = useState("");
  const [open, setOpen] = useState(false);
  const [index, setIndex] = useState(0);
  const root = useRef<HTMLDivElement>(null);
  const listRef = useRef<HTMLDivElement>(null);
  const q = query.trim().toLowerCase();
  const matches = q ? apps.filter((a) => a.name.toLowerCase().includes(q)) : apps;

  useEffect(() => {
    if (!open) return;
    const close = (e: PointerEvent) => {
      if (!root.current?.contains(e.target as Node)) setOpen(false);
    };
    document.addEventListener("pointerdown", close);
    return () => document.removeEventListener("pointerdown", close);
  }, [open]);

  useEffect(() => {
    listRef.current?.children[index]?.scrollIntoView({ block: "nearest" });
  }, [index, open]);

  const pick = (app: App | undefined) => {
    if (!app) return;
    setQuery("");
    setOpen(false);
    onPick(app);
  };

  const onKeyDown = (e: KeyboardEvent) => {
    if (e.key === "Escape") {
      e.preventDefault();
      setOpen(false);
    } else if (!open) {
      if (e.key === "ArrowDown" || e.key === "ArrowUp") setOpen(true);
    } else if (e.key === "Enter") {
      e.preventDefault();
      pick(matches[index]);
    } else if (e.key === "ArrowDown" || e.key === "ArrowUp") {
      e.preventDefault();
      setIndex((i) => nextIndex(e.key, i, matches.length) ?? i);
    }
  };

  return (
    <div className="app-search" ref={root}>
      <span aria-hidden>🔍</span>
      <input
        value={query}
        placeholder={t.settings.searchApps}
        aria-label={t.settings.searchApps}
        disabled={apps.length === 0}
        spellCheck={false}
        onFocus={() => setOpen(true)}
        onChange={(e) => {
          setQuery(e.target.value);
          setIndex(0);
          setOpen(true);
        }}
        onKeyDown={onKeyDown}
      />
      {open && (
        <div className="suggest">
          {matches.length === 0 ? (
            <p className="suggest-empty">{t.noApps}</p>
          ) : (
            <div className="suggest-list" role="listbox" ref={listRef}>
              {matches.map((a, i) => (
                <div
                  key={a.id}
                  role="option"
                  aria-selected={i === index}
                  className={i === index ? "suggest-item on" : "suggest-item"}
                  onPointerEnter={() => setIndex(i)}
                  onClick={() => pick(a)}
                >
                  {a.icon ? <img src={a.icon} alt="" /> : <span className="suggest-noicon" />}
                  <span className="suggest-name">{highlight(a.name, query.trim())}</span>
                  <span className="suggest-count">{a.count}</span>
                </div>
              ))}
            </div>
          )}
          <p className="suggest-hint">{t.suggestHint}</p>
        </div>
      )}
    </div>
  );
}
