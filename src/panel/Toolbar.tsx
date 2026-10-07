import type { RefObject } from "react";
import type { Filter } from "../api.ts";
import { usePrefs } from "../prefs.tsx";

export const FILTERS: Filter[] = ["all", "text", "image", "files", "link"];

interface Props {
  query: string;
  onQuery: (query: string) => void;
  filter: Filter;
  onFilter: (filter: Filter) => void;
  inputRef: RefObject<HTMLInputElement | null>;
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
  onSettings,
  updateVersion,
  updateConfirming,
  updateFailed,
  onUpdate,
  onConfirmUpdate,
  onCancelUpdate,
}: Props) {
  const { t } = usePrefs();
  return (
    <div className="toolbar">
      <label className="search">
        <span aria-hidden>🔍</span>
        <input
          ref={inputRef}
          value={query}
          placeholder={t.search}
          aria-label={t.search}
          onChange={(e) => onQuery(e.target.value)}
          autoFocus
          spellCheck={false}
        />
      </label>
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
