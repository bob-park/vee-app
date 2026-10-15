import { type RefObject, useEffect, useRef } from 'react';

import type { Filter } from '@/domain/clip/apis/clip.dto';
import type { App } from '@/domain/source_app/apis/sourceApp.dto';
import Suggest from '@/shared/components/suggest/Suggest';
import { useI18n } from '@/shared/providers/i18n/I18nProvider';
import { acceleratorKeys } from '@/utils/accelerator';

import cx from 'classnames';

import { ConfirmButton } from './ConfirmButton';

export const FILTERS: Filter[] = ['all', 'text', 'image', 'files', 'link'];

const isMac = navigator.userAgent.includes('Mac');

/**
 * Hover label under a toolbar button (the `group`), with the shortcut drawn as keycaps.
 * Dark in both themes, like a native tooltip; shows after a short hover.
 */
function Tip({ label, accel, className = 'right-0' }: { label: string; accel: string; className?: string }) {
  return (
    <span
      className={cx(
        'bg-tip pointer-events-none invisible absolute top-[calc(100%+6px)] z-10 flex items-center gap-2 rounded-lg py-1.25 pr-1.5 pl-2.25 text-[11px] whitespace-nowrap text-white opacity-0 transition-[opacity,visibility] duration-120 ease-[ease] group-hover:visible group-hover:opacity-100 group-hover:delay-500',
        className,
      )}
      aria-hidden
    >
      {label}
      <span className="flex gap-0.75">
        {acceleratorKeys(accel, isMac).map((k, i) => (
          // A keycap: thicker bottom edge.
          <kbd
            key={i}
            className="border-keycap-border bg-keycap-bg flex h-[1.7em] min-w-[1.7em] items-center justify-center rounded-[5px] border border-b-2 px-[0.4em] [font-family:inherit] text-[10px]"
          >
            {k}
          </kbd>
        ))}
      </span>
    </span>
  );
}

const ICON_BTN = 'group relative rounded-[10px] px-1.5 text-[24px] leading-none';

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
  // ref
  const listRef = useRef<HTMLDivElement>(null);

  // hooks
  const { t } = useI18n();

  // useEffect
  // Only the list scrolls, so keep the highlighted app in view while moving with the arrows.
  useEffect(() => {
    listRef.current?.children[suggestIndex]?.scrollIntoView({ block: 'nearest' });
  }, [suggestIndex, suggestions]);
  return (
    <div className="mb-2.5 flex items-center gap-1.5">
      <div className="rounded-box bg-subtle text-muted relative flex shrink-0 basis-55 items-center gap-1.5 px-2.5 py-1.5">
        <span aria-hidden>🔍</span>
        {app && (
          <span className="bg-accent-subtle text-primary flex max-w-[120px] flex-none items-center gap-1 rounded-md px-0.5 py-px text-[12px] font-semibold whitespace-nowrap">
            {app.icon && <img className="size-4 max-w-none rounded-sm" src={app.icon} alt="" />}
            <span className="min-w-0 truncate">{app.name}</span>
            <button
              className="px-0.5 leading-none opacity-60"
              onClick={onClearApp}
              aria-label={t.clearApp}
              tabIndex={-1}
            >
              ×
            </button>
          </span>
        )}
        <input
          className="text-base-content placeholder:text-placeholder min-w-0 flex-1 p-px outline-none"
          ref={inputRef}
          value={query}
          placeholder={app ? '' : t.search}
          aria-label={t.search}
          onChange={(e) => onQuery(e.target.value)}
          autoFocus
          spellCheck={false}
        />
        {suggesting && (
          <Suggest
            apps={suggestions}
            index={suggestIndex}
            query={query.slice(1)}
            listRef={listRef}
            className="max-h-[calc(100vh-70px)] w-60"
            onPick={onPickApp}
          />
        )}
      </div>
      <div className="flex gap-1.5" role="tablist">
        {FILTERS.map((f) => (
          <button
            key={f}
            role="tab"
            aria-selected={f === filter}
            className={cx(
              'rounded-box px-2.5 py-1 text-[12px]',
              f === filter ? 'bg-accent-subtle text-primary font-semibold' : 'bg-subtle',
            )}
            onClick={() => onFilter(f)}
            tabIndex={-1}
          >
            {t.filters[f]}
          </button>
        ))}
      </div>
      {searching && (
        // Shown only while a search, tag or filter is kept.
        <button
          className="group rounded-box border-primary text-primary hover:bg-accent-subtle relative border border-dashed px-2.5 py-0.75 text-[12px]"
          onClick={onResetSearch}
          tabIndex={-1}
        >
          ↺ {t.resetSearch}
          {/* Opens rightwards: the chip sits mid-toolbar, so a right-anchored tip would cover the chips. */}
          <Tip label={t.resetSearchTip} accel={resetSearchShortcut} className="left-0" />
        </button>
      )}
      <span className="flex-1" />
      {updateVersion && (
        // `update` is the hook PanelApp keeps a click inside from cancelling the prompt by.
        <div className="update relative">
          <button
            className="bg-ok-bg text-ok-fg flex items-center gap-1.25 rounded-md py-0.75 pr-2 pl-1.5 text-[12px] font-semibold"
            onClick={onUpdate}
            tabIndex={-1}
          >
            <i className="size-1.5 rounded-full bg-current" aria-hidden />
            {t.newVersion(updateVersion)}
          </button>
          {updateConfirming && (
            <div
              className="rounded-box border-base-300 bg-card shadow-suggest absolute top-[calc(100%+6px)] right-0 z-2 grid w-60 gap-2.5 border p-3"
              role="dialog"
            >
              <div>
                <p className="font-semibold">{t.updateConfirm(updateVersion)}</p>
                <p className={cx('mt-0.5 text-[12px]', updateFailed ? 'text-error' : 'text-muted')}>
                  {updateFailed ? t.updateFailed : t.updateRestartHint}
                </p>
              </div>
              <div className="flex justify-end gap-2">
                <ConfirmButton onClick={onCancelUpdate} tabIndex={-1}>
                  {t.cancel}
                </ConfirmButton>
                <ConfirmButton tone="go" onClick={onConfirmUpdate} tabIndex={-1}>
                  {t.restart}
                </ConfirmButton>
              </div>
            </div>
          )}
        </div>
      )}
      <button
        className={cx(ICON_BTN, panelPinned ? 'bg-accent-subtle text-primary' : 'text-muted hover:bg-subtle')}
        onClick={onTogglePanelPin}
        aria-pressed={panelPinned}
        aria-label={panelPinned ? t.unpinPanel : t.pinPanel}
        tabIndex={-1}
      >
        <svg
          className="block size-4.5"
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
      <button
        className={cx(ICON_BTN, 'text-muted hover:bg-subtle')}
        onClick={onSettings}
        aria-label={t.settings.title}
        tabIndex={-1}
      >
        ⚙︎
        <Tip label={t.settings.title} accel="CommandOrControl+Comma" />
      </button>
    </div>
  );
}
