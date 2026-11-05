import { type ReactNode, type RefObject, useEffect, useRef } from 'react';

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
function Tip({ label, accel, className = 'right-0' }: { label: string; accel?: string; className?: string }) {
  return (
    <span
      className={cx(
        'bg-tip pointer-events-none invisible absolute top-[calc(100%+6px)] z-10 flex items-center gap-2 rounded-lg py-1.25 pr-1.5 pl-2.25 text-[11px] whitespace-nowrap text-white opacity-0 transition-[opacity,visibility] duration-120 ease-[ease] group-hover:visible group-hover:opacity-100 group-hover:delay-500',
        className,
      )}
      aria-hidden
    >
      {label}
      {accel && (
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
      )}
    </span>
  );
}

const ICON_BTN = 'group relative rounded-[10px] px-1.5 text-[24px] leading-none';

/** Filter tab icons for a toolbar too narrow for the labels. Lucide `layout-grid`, `type`, `image`, `file`, `link` (ISC). */
const FILTER_ICONS: Record<Filter, ReactNode> = {
  all: (
    <>
      <rect x="3" y="3" width="7" height="7" rx="1" />
      <rect x="14" y="3" width="7" height="7" rx="1" />
      <rect x="14" y="14" width="7" height="7" rx="1" />
      <rect x="3" y="14" width="7" height="7" rx="1" />
    </>
  ),
  text: (
    <>
      <path d="M12 4v16" />
      <path d="M4 7V5a1 1 0 0 1 1-1h14a1 1 0 0 1 1 1v2" />
      <path d="M9 20h6" />
    </>
  ),
  image: (
    <>
      <rect x="3" y="3" width="18" height="18" rx="2" />
      <circle cx="9" cy="9" r="2" />
      <path d="m21 15-3.086-3.086a2 2 0 0 0-2.828 0L6 21" />
    </>
  ),
  files: (
    <>
      <path d="M15 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V7Z" />
      <path d="M14 2v4a2 2 0 0 0 2 2h4" />
    </>
  ),
  link: (
    <>
      <path d="M10 13a5 5 0 0 0 7.54.54l3-3a5 5 0 0 0-7.07-7.07l-1.72 1.71" />
      <path d="M14 11a5 5 0 0 0-7.54-.54l-3 3a5 5 0 0 0 7.07 7.07l1.71-1.71" />
    </>
  ),
};

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
  /** Pinning was just switched off: the pin icon is pulled out. */
  panelPinReleasing: boolean;
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
  panelPinReleasing,
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
    <div className="@container relative z-2 mb-2.5 flex items-center gap-1.5 in-data-side:flex-wrap in-data-side:gap-y-2">
      {/* Docked left/right the toolbar wraps below the search box; z-3 keeps its app list over that second line. */}
      <div className="rounded-box glass text-muted relative flex shrink-0 basis-55 items-center gap-1.5 px-2.5 py-1.5 in-data-side:z-3 in-data-side:basis-full">
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
          className="text-base-content placeholder:text-placeholder w-0 min-w-0 flex-1 p-px outline-none"
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
      {/* in-data-side:z-2: the glass is its own stacking context, so without it the icon tips sit under row 3. */}
      <div
        className="glass rounded-box relative inline-grid auto-cols-fr grid-flow-col gap-1 p-[3px] in-data-side:z-2"
        role="tablist"
      >
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
              'group relative flex items-center justify-center px-2.5 py-1 text-[12px]',
              f === filter ? 'text-primary-content font-semibold' : 'text-muted',
            )}
            onClick={() => onFilter(f)}
            aria-label={t.filters[f]}
            tabIndex={-1}
          >
            {/* 328px: the widest label row (English, 324px) plus slack; narrower, the tabs show icons. */}
            <span className="@max-[328px]:hidden">{t.filters[f]}</span>
            <svg
              className="hidden size-4 @max-[328px]:block"
              viewBox="0 0 24 24"
              fill="none"
              stroke="currentColor"
              strokeWidth="2"
              strokeLinecap="round"
              strokeLinejoin="round"
              aria-hidden
            >
              {FILTER_ICONS[f]}
            </svg>
            <Tip label={t.filters[f]} className="left-1/2 -translate-x-1/2 font-normal @min-[328px]:hidden" />
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
      {/* Docked left/right this is its own row: the update pill at the start, pin and settings at the end. */}
      <div className="flex items-center gap-1.5 in-data-side:basis-full in-data-side:justify-between">
        {updateVersion && (
          // `update` is the hook PanelApp keeps a click inside from cancelling the prompt by.
          // mr-3: the hopping pill needs more room from the pin button than the toolbar's 6px gap.
          // Hover pauses the hop on this still wrapper, not the moving button, so a cursor just above the pill
          // can't flicker it; `before:` stretches the hover area over the hop's ~8px rise.
          <div className="update group relative mr-3 before:absolute before:inset-x-0 before:-top-2.5 before:bottom-0">
            {/* Landing shadow: a sibling of the button so it stays on the ground while the pill hops. */}
            <span
              className="bg-ok-solid/45 animate-jump-shadow pointer-events-none absolute inset-x-[15%] -bottom-1.5 h-1 rounded-full blur-[2px] group-hover:animate-none"
              aria-hidden
            />
            <button
              // Hops until hovered so a ready update isn't missed.
              className="bg-ok-solid text-ok-solid-fg animate-jump relative flex items-center gap-1.25 rounded-md py-0.75 pr-2 pl-1.5 text-[12px] font-semibold group-hover:animate-none"
              onClick={onUpdate}
              tabIndex={-1}
            >
              <span aria-hidden>↑</span>
              {t.newVersion(updateVersion)}
            </button>
            {updateConfirming && (
              <div
                className="rounded-box border-base-300 bg-card shadow-suggest animate-spring-in absolute top-[calc(100%+6px)] right-0 z-2 grid w-60 origin-top-right gap-2.5 border p-3 in-data-side:right-auto in-data-side:left-0 in-data-side:max-w-[calc(100vw-24px)] in-data-side:origin-top-left"
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
        {/* ml-auto keeps pin and settings at the end when there is no update pill. */}
        <div className="ml-auto flex items-center gap-1.5">
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
            {/* A red pushpin drawn for Vee (not an emoji, which differs per OS); it tilts while pinned.
            `rotate` composes with the drop/pull keyframes, which animate `transform`. */}
            <svg
              className={cx(
                'ease-spring block size-4.5 transition-[rotate] duration-200',
                panelPinned && '-rotate-30',
                panelPinPressing && 'animate-pin-drop',
                panelPinReleasing && 'animate-pin-pull',
              )}
              viewBox="0 0 24 24"
              fill="none"
              strokeLinecap="round"
              aria-hidden
            >
              <path className="stroke-pin-needle" strokeWidth="2" d="M11 14.5 4.5 21" />
              <path
                className="fill-pin"
                d="M14.6 2.6a1.2 1.2 0 0 1 1.7 0l5.1 5.1a1.2 1.2 0 0 1 0 1.7l-1.2 1.2-1.6-.5-3.1 3.1.7 3.4a1 1 0 0 1-.3.9l-.8.8L6.7 10l.8-.8a1 1 0 0 1 .9-.3l3.4.7 3.1-3.1-.5-1.6z"
              />
              <path className="stroke-pin-hi" strokeWidth="1.3" opacity="0.8" d="M15.4 3.6 19.9 8" />
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
      </div>
    </div>
  );
}
