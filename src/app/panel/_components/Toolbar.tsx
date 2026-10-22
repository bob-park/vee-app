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
  /** The pointer moved onto suggestion `index`. */
  onHoverApp: (index: number) => void;
  onPickApp: (app: App) => void;
  onSettings: () => void;
  /** The panel stays up when focus leaves it. */
  panelPinned: boolean;
  /** Pinning was just switched on: the pin icon drops in. */
  panelPinPressing: boolean;
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
  /** Restart was pressed and the app is fetching the latest version. */
  updateInstalling: boolean;
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
  onHoverApp,
  onPickApp,
  onSettings,
  panelPinned,
  panelPinPressing,
  panelPinShortcut,
  searching,
  onResetSearch,
  resetSearchShortcut,
  onTogglePanelPin,
  updateVersion,
  updateConfirming,
  updateFailed,
  updateInstalling,
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
    // z-2: above the card body (z-1, later in the DOM) so the suggest list, update popover and tips
    // aren't covered by cards; both stay above the aurora (z-0).
    <div className="relative z-2 mb-2.5 flex items-center gap-1.5">
      <div className="rounded-box glass text-muted relative flex shrink-0 basis-55 items-center gap-1.5 px-2.5 py-1.5">
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
            onHover={onHoverApp}
            onPick={onPickApp}
          />
        )}
      </div>
      <div className="glass rounded-box relative inline-grid auto-cols-fr grid-flow-col gap-1 p-[3px]" role="tablist">
        {/* The sliding pill behind the selected tab: one column wide (3px padding, 4px gaps), moved by whole columns. */}
        <span
          className="bg-primary ease-spring absolute inset-y-[3px] left-[3px] rounded-[9px] transition-transform duration-450"
          style={{
            width: `calc((100% - 6px - ${(FILTERS.length - 1) * 4}px) / ${FILTERS.length})`,
            transform: `translateX(calc(${FILTERS.indexOf(filter)} * (100% + 4px)))`,
          }}
          aria-hidden
        />
        {FILTERS.map((f) => (
          <button
            key={f}
            role="tab"
            aria-selected={f === filter}
            className={cx(
              'relative px-2.5 py-1 text-[12px]',
              f === filter ? 'text-primary-content font-semibold' : 'text-muted',
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
                  {updateFailed ? t.updateFailed : updateInstalling ? t.updateInstalling : t.updateRestartHint}
                </p>
              </div>
              <div className="flex justify-end gap-2">
                <ConfirmButton onClick={onCancelUpdate} disabled={updateInstalling} tabIndex={-1}>
                  {t.cancel}
                </ConfirmButton>
                <ConfirmButton tone="go" onClick={onConfirmUpdate} disabled={updateInstalling} tabIndex={-1}>
                  {t.restart}
                </ConfirmButton>
              </div>
            </div>
          )}
        </div>
      )}
      <button
        // The transparent border keeps the size of the idle glass button's hairline.
        className={cx(
          ICON_BTN,
          panelPinned ? 'bg-accent-subtle text-primary border border-transparent' : 'glass text-muted',
        )}
        onClick={onTogglePanelPin}
        aria-pressed={panelPinned}
        aria-label={panelPinned ? t.unpinPanel : t.pinPanel}
        tabIndex={-1}
      >
        {/* Lucide `pin` (ISC); the head fills while pinned. */}
        <svg
          className={cx('block size-4.5', panelPinPressing && 'animate-pin-drop')}
          viewBox="0 0 24 24"
          fill="none"
          stroke="currentColor"
          strokeWidth="2"
          strokeLinecap="round"
          strokeLinejoin="round"
          aria-hidden
        >
          <path d="M12 17v5" />
          <path
            className="transition-[fill] duration-200"
            fill={panelPinned ? 'currentColor' : 'none'}
            d="M9 10.76a2 2 0 0 1-1.11 1.79l-1.78.9A2 2 0 0 0 5 15.24V16a1 1 0 0 0 1 1h12a1 1 0 0 0 1-1v-.76a2 2 0 0 0-1.11-1.79l-1.78-.9A2 2 0 0 1 15 10.76V7a1 1 0 0 1 1-1 2 2 0 0 0 0-4H8a2 2 0 0 0 0 4 1 1 0 0 1 1 1z"
          />
        </svg>
        <Tip label={panelPinned ? t.unpinPanel : t.pinPanel} accel={panelPinShortcut} />
      </button>
      <button
        className={cx(ICON_BTN, 'glass text-muted')}
        onClick={onSettings}
        aria-label={t.settings.title}
        tabIndex={-1}
      >
        {/* An SVG, not the ⚙︎ glyph: font metrics put the glyph off centre, so it wobbled as it turned. 60° is one tooth, so it lands looking as it started.
            Lucide `settings` (ISC); the tip below is a sibling so it doesn't turn with it. */}
        <svg
          className="ease-spring block size-4.5 transition-transform duration-500 motion-safe:group-hover:rotate-60"
          viewBox="0 0 24 24"
          fill="none"
          stroke="currentColor"
          strokeWidth="2"
          strokeLinecap="round"
          strokeLinejoin="round"
          aria-hidden
        >
          <path d="M12.22 2h-.44a2 2 0 0 0-2 2v.18a2 2 0 0 1-1 1.73l-.43.25a2 2 0 0 1-2 0l-.15-.08a2 2 0 0 0-2.73.73l-.22.38a2 2 0 0 0 .73 2.73l.15.1a2 2 0 0 1 1 1.72v.51a2 2 0 0 1-1 1.74l-.15.09a2 2 0 0 0-.73 2.73l.22.38a2 2 0 0 0 2.73.73l.15-.08a2 2 0 0 1 2 0l.43.25a2 2 0 0 1 1 1.73V20a2 2 0 0 0 2 2h.44a2 2 0 0 0 2-2v-.18a2 2 0 0 1 1-1.73l.43-.25a2 2 0 0 1 2 0l.15.08a2 2 0 0 0 2.73-.73l.22-.39a2 2 0 0 0-.73-2.73l-.15-.08a2 2 0 0 1-1-1.74v-.5a2 2 0 0 1 1-1.74l.15-.09a2 2 0 0 0 .73-2.73l-.22-.38a2 2 0 0 0-2.73-.73l-.15.08a2 2 0 0 1-2 0l-.43-.25a2 2 0 0 1-1-1.73V4a2 2 0 0 0-2-2z" />
          <circle cx="12" cy="12" r="3" />
        </svg>
        <Tip label={t.settings.title} accel="CommandOrControl+Comma" />
      </button>
    </div>
  );
}
