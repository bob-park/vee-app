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
}

export function Toolbar({ query, onQuery, filter, onFilter, inputRef, onSettings }: Props) {
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
      <button className="icon-btn" onClick={onSettings} aria-label={t.settings.title} tabIndex={-1}>
        ⚙︎
      </button>
    </div>
  );
}
