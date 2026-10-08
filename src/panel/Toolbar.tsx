import { useEffect, useRef, type RefObject } from "react";
import type { App, Filter } from "../api.ts";
import { usePrefs } from "../prefs.tsx";

export const FILTERS: Filter[] = ["all", "text", "image", "files", "link"];

/** The app name with the part matching `q` in bold. */
function highlight(name: string, q: string) {
  const i = q ? name.toLowerCase().indexOf(q.toLowerCase()) : -1;
  if (i < 0) return name;
  return (
    <>
      {name.slice(0, i)}
      <b>{name.slice(i, i + q.length)}</b>
      {name.slice(i + q.length)}
    </>
  );
}

interface Props {
  query: string;
  onQuery: (query: string) => void;
  filter: Filter;
  onFilter: (filter: Filter) => void;
  inputRef: RefObject<HTMLInputElement | null>;
  /** The app tag in the search box, or null. */
  app: App | null;
  onClearApp: () => void;
  /** The `@app` suggestion list is open. */
  suggesting: boolean;
  suggestions: App[];
  suggestIndex: number;
  onPickApp: (app: App) => void;
  onSettings: () => void;
  /** The downloaded update's version, or null when there is none. */
  updateVersion: string | null;
  updateConfirming: boolean;
  updateFailed: boolean;
  onUpdate: () => void;
  onConfirmUpdate: () => void;
  onCancelUpdate: () => void;
}

export function Toolbar({
  query,
  onQuery,
  filter,
  onFilter,
  inputRef,
  app,
  onClearApp,
  suggesting,
  suggestions,
  suggestIndex,
  onPickApp,
  onSettings,
  updateVersion,
  updateConfirming,
  updateFailed,
  onUpdate,
  onConfirmUpdate,
  onCancelUpdate,
}: Props) {
  const { t } = usePrefs();
  const listRef = useRef<HTMLDivElement>(null);
  // Only the list scrolls, so keep the highlighted app in view while moving with the arrows.
  useEffect(() => {
    listRef.current?.children[suggestIndex]?.scrollIntoView({ block: "nearest" });
  }, [suggestIndex, suggestions]);
  return (
    <div className="toolbar">
      <div className="search">
        <span aria-hidden>🔍</span>
        {app && (
          <span className="app-tag">
            {app.icon && <img src={app.icon} alt="" />}
            <span className="app-tag-name">{app.name}</span>
            <button className="app-tag-x" onClick={onClearApp} aria-label={t.clearApp} tabIndex={-1}>
              ×
            </button>
          </span>
        )}
        <input
          ref={inputRef}
          value={query}
          placeholder={app ? "" : t.search}
          aria-label={t.search}
          onChange={(e) => onQuery(e.target.value)}
          autoFocus
          spellCheck={false}
        />
        {suggesting && (
          <div className="suggest">
            {suggestions.length === 0 ? (
              <p className="suggest-empty">{t.noApps}</p>
            ) : (
              <div className="suggest-list" role="listbox" ref={listRef}>
                {suggestions.map((a, i) => (
                  <div
                    key={a.id}
                    role="option"
                    aria-selected={i === suggestIndex}
                    className={i === suggestIndex ? "suggest-item on" : "suggest-item"}
                    onClick={() => onPickApp(a)}
                  >
                    {a.icon ? <img src={a.icon} alt="" /> : <span className="suggest-noicon" />}
                    <span className="suggest-name">{highlight(a.name, query.slice(1))}</span>
                    <span className="suggest-count">{a.count}</span>
                  </div>
                ))}
              </div>
            )}
            <p className="suggest-hint">{t.suggestHint}</p>
          </div>
        )}
      </div>
      <div className="chips" role="tablist">
        {FILTERS.map((f) => (
          <button
            key={f}
            role="tab"
            aria-selected={f === filter}
            className={f === filter ? "chip on" : "chip"}
            onClick={() => onFilter(f)}
            tabIndex={-1}
          >
            {t.filters[f]}
          </button>
        ))}
      </div>
      <span className="spacer" />
      {updateVersion && (
        <div className="update">
          <button className="update-badge" onClick={onUpdate} tabIndex={-1}>
            <i aria-hidden />
            {t.newVersion(updateVersion)}
          </button>
          {updateConfirming && (
            <div className="update-pop" role="dialog">
              <div>
                <p className="update-title">{t.updateConfirm(updateVersion)}</p>
                <p className={updateFailed ? "update-sub failed" : "update-sub"}>
                  {updateFailed ? t.updateFailed : t.updateRestartHint}
                </p>
              </div>
              <div className="confirm-btns">
                <button className="confirm-btn" onClick={onCancelUpdate} tabIndex={-1}>
                  {t.cancel}
                </button>
                <button className="confirm-btn go" onClick={onConfirmUpdate} tabIndex={-1}>
                  {t.restart}
                </button>
              </div>
            </div>
          )}
        </div>
      )}
      <button className="icon-btn" onClick={onSettings} aria-label={t.settings.title} tabIndex={-1}>
        ⚙︎
      </button>
    </div>
  );
}
