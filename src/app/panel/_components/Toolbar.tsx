import { useEffect, useRef, type RefObject } from "react";
import type { Filter } from "@/domain/clip/apis/clip.dto";
import type { App } from "@/domain/source_app/apis/sourceApp.dto";
import { usePrefs } from "@/prefs";
import { highlight } from "@/shared/components/highlight";
import { acceleratorKeys } from "@/utils/accelerator";

export const FILTERS: Filter[] = ["all", "text", "image", "files", "link"];

const isMac = navigator.userAgent.includes("Mac");

/** Hover label under a toolbar button, with the shortcut drawn as keycaps. */
function Tip({ label, accel }: { label: string; accel: string }) {
  return (
    <span className="tip" aria-hidden>
      {label}
      <span className="tip-keys">
        {acceleratorKeys(accel, isMac).map((k, i) => (
          <kbd key={i}>{k}</kbd>
        ))}
      </span>
    </span>
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
  /** The panel stays up when focus leaves it. */
  panelPinned: boolean;
  panelPinShortcut: string;
  onTogglePanelPin: () => void;
  /** A search, app tag or filter is set. */
  searching: boolean;
  onResetSearch: () => void;
  resetSearchShortcut: string;
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
  panelPinned,
  panelPinShortcut,
  searching,
  onResetSearch,
  resetSearchShortcut,
  onTogglePanelPin,
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
      {searching && (
        <button className="reset-chip" onClick={onResetSearch} tabIndex={-1}>
          ↺ {t.resetSearch}
          <Tip label={t.resetSearchTip} accel={resetSearchShortcut} />
        </button>
      )}
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
      <button
        className={panelPinned ? "icon-btn on" : "icon-btn"}
        onClick={onTogglePanelPin}
        aria-pressed={panelPinned}
        aria-label={panelPinned ? t.unpinPanel : t.pinPanel}
        tabIndex={-1}
      >
        <svg
          viewBox="0 0 24 24"
          fill="none"
          stroke="currentColor"
          strokeWidth="2"
          strokeLinecap="round"
          strokeLinejoin="round"
          aria-hidden
        >
          <path d="M12 17v5M9 3h6M10 3v7l-4 4v3h12v-3l-4-4V3" />
        </svg>
        <Tip label={panelPinned ? t.unpinPanel : t.pinPanel} accel={panelPinShortcut} />
      </button>
      <button className="icon-btn" onClick={onSettings} aria-label={t.settings.title} tabIndex={-1}>
        ⚙︎
        <Tip label={t.settings.title} accel="CommandOrControl+Comma" />
      </button>
    </div>
  );
}
