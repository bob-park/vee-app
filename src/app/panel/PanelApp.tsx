import {
  type KeyboardEvent,
  type UIEvent,
  type WheelEvent,
  useCallback,
  useEffect,
  useLayoutEffect,
  useRef,
  useState,
} from 'react';

import { Card } from '@/app/panel/_components/Card';
import { ResizeHandle } from '@/app/panel/_components/ResizeHandle';
import { FILTERS, Toolbar } from '@/app/panel/_components/Toolbar';
import { cancelFloat, cardOffsets, floatCard, playFlip, reducedMotion } from '@/app/panel/_components/motion';
import { copyClip, deleteClip, listClips, listPinned, setPinned, startDrag } from '@/domain/clip/apis/clip';
import type { Clip } from '@/domain/clip/apis/clip.dto';
import { useConvertJobs } from '@/domain/convert/queries/convert';
import {
  hidePanel,
  openSettings,
  revealPanel,
  setPanelPinned as setPanelPinnedApi,
  warnSidePins,
} from '@/domain/panel/apis/panel';
import type { PanelPosition } from '@/domain/settings/apis/settings.dto';
import { useSettings } from '@/domain/settings/queries/settings';
import type { App } from '@/domain/source_app/apis/sourceApp.dto';
import { useAppSuggestions } from '@/domain/source_app/queries/sourceApp';
import { installUpdate as installUpdateApi } from '@/domain/updater/apis/updater';
import { useUpdateStatus } from '@/domain/updater/queries/updater';
import type { AppError } from '@/shared/api/common.dto';
import Aurora from '@/shared/components/Aurora';
import { useI18n } from '@/shared/providers/i18n/I18nProvider';
import { useStore } from '@/shared/store/rootStore';
import { acceleratorFromEvent } from '@/utils/accelerator';
import { dragFileName } from '@/utils/fileThumb';
import { type PinnedSlot, panelKeyAction } from '@/utils/keys';
import { isSide, pinsThatFit } from '@/utils/panelEdge';
import {
  type Point,
  clampSelection,
  defaultSelection,
  followSelection,
  keepSelectionOnReload,
  pointerMoved,
} from '@/utils/selection';

import { listen } from '@tauri-apps/api/event';
import cx from 'classnames';

const PAGE = 50;
const isMac = navigator.userAgent.includes('Mac');
/** How long after a cancelled drag an Escape is taken as the one that cancelled it (logged at ~36 ms). */
const DRAG_CANCEL_ESC_MS = 300;
/** How long `data-entering` stays on after an open: the last staggered card (8 × 25 ms) plus its 650 ms spring. */
const ENTER_MS = 900;

// Where the panel waits off screen (hidden, closing, or while a file is dragged out): past its docked edge.
const PARKED: Record<PanelPosition, string> = {
  bottom: 'translate-y-[calc(100%+16px)]',
  top: '-translate-y-[calc(100%+16px)]',
  left: '-translate-x-[calc(100%+16px)]',
  right: 'translate-x-[calc(100%+16px)]',
};

export function PanelApp() {
  // ref
  const noticeTimer = useRef<number | undefined>(undefined);
  /** Panel size and pin count the side-pins toast last warned about. */
  const warnedPins = useRef<string | null>(null);
  const enterTimer = useRef<number | undefined>(undefined);
  const bodyRef = useRef<HTMLDivElement>(null);
  const requestId = useRef(0);
  const loadingMore = useRef(false);
  const inputRef = useRef<HTMLInputElement>(null);
  const rowRef = useRef<HTMLDivElement>(null);
  /** Card positions captured just before an animated reload, consumed by the next layout. */
  const flipFrom = useRef<Map<number, number> | null>(null);
  /** Lets a reload follow the selected card when pinning moves it to the other area. */
  const selectedIdRef = useRef<number | undefined>(undefined);
  /**
   * Last cursor position anywhere over the panel, gaps included; null until the first move after
   * the panel opens. Tracked on the panel so a card sliding under a still cursor compares against
   * where the cursor really is.
   */
  const lastPointer = useRef<Point | null>(null);
  const dragCancelledAt = useRef(0);
  /** Whether the current mousemove is a real pointer move, set before the card sees the event. */
  const pointerMovedNow = useRef(false);
  /** The next selection change came from hovering: the card is already under the cursor, don't scroll. */
  const hoverSelected = useRef(false);
  const openRef = useRef(false);
  /** Lets the listeners, subscribed once, always call the latest reload. */
  const reloadRef = useRef<(keepSelection: boolean, animate?: boolean) => Promise<void>>(async () => {});

  // state
  const [clips, setClips] = useState<Clip[]>([]);
  const [pinned, setPinnedClips] = useState<Clip[]>([]);
  const [pinNotice, setPinNotice] = useState(false);
  const [selected, setSelected] = useState(0);
  const [hasMore, setHasMore] = useState(false);
  const [open, setOpen] = useState(false);
  /** The cards play their open entrance (CSS only, so the keyboard never waits on it). */
  const [entering, setEntering] = useState(false);
  const [closing, setClosing] = useState(false);
  const [dragging, setDragging] = useState(false);
  const [confirmingId, setConfirmingId] = useState<number | null>(null);
  const [updateConfirming, setUpdateConfirming] = useState(false);
  const [updateFailed, setUpdateFailed] = useState(false);
  /** Restart was pressed; the app is re-checking or downloading before it restarts. */
  const [updateInstalling, setUpdateInstalling] = useState(false);
  const [panelPinned, setPanelPinned] = useState(false);
  /** Pinning was just switched on; plays the press, then the edge sweep, then shows the pinned border. */
  const [pinPressing, setPinPressing] = useState(false);
  const [suggestIndex, setSuggestIndex] = useState(0);

  // store
  // Kept between openings: the window is only hidden.
  const query = useStore((s) => s.query);
  const filter = useStore((s) => s.filter);
  const app = useStore((s) => s.app);
  const setQuery = useStore((s) => s.setQuery);
  const setFilter = useStore((s) => s.setFilter);
  const setApp = useStore((s) => s.setApp);
  const clearSearch = useStore((s) => s.resetSearch);

  // hooks
  const { t } = useI18n();

  openRef.current = open;
  /** Every card in screen order; `selected` indexes this. */
  const items = [...pinned, ...clips];
  const selectedId = items[selected]?.id;
  selectedIdRef.current = selectedId;

  // While `@…` is being typed it names an app, not text to search for.
  const suggesting = app === null && query.startsWith('@');
  const textQuery = suggesting ? '' : query;
  const appId = app?.id ?? null;

  // queries
  const { settings } = useSettings();
  const position = settings?.panelPosition ?? 'bottom';
  const side = isSide(position);
  const { suggestions } = useAppSuggestions(query.slice(1), suggesting);
  const { status: updateStatus } = useUpdateStatus();
  const { jobs } = useConvertJobs();

  const updateVersion = updateStatus.status === 'ready' ? updateStatus.version : null;

  // callback (declared before the effects that depend on them)
  /** Reloads the first page. Responses from superseded searches are dropped. */
  const reload = useCallback(
    async (keepSelection: boolean, animate = false) => {
      const id = ++requestId.current;
      const [page, pins] = await Promise.all([listClips(textQuery, filter, appId, 0, PAGE), listPinned()]);
      if (id !== requestId.current) return;
      // Only copies arriving while the panel is on screen animate — not searches or filters.
      if (animate && openRef.current && !reducedMotion()) flipFrom.current = cardOffsets(bodyRef.current, side);
      setPinnedClips(pins);
      setClips(page);
      setHasMore(page.length === PAGE);
      const ids = [...pins, ...page].map((c) => c.id);
      setSelected((s) =>
        keepSelection ? followSelection(selectedIdRef.current, s, ids) : defaultSelection(pins.length, page.length),
      );
      if (!keepSelection) {
        setConfirmingId(null);
        rowRef.current?.scrollTo({ left: 0, top: 0 });
      }
    },
    [textQuery, filter, appId, side],
  );

  const loadMore = useCallback(async () => {
    if (!hasMore || loadingMore.current) return;
    loadingMore.current = true;
    const id = requestId.current;
    try {
      const page = await listClips(textQuery, filter, appId, clips.length, PAGE);
      if (id !== requestId.current) return;
      setClips((prev) => [...prev, ...page]);
      setHasMore(page.length === PAGE);
    } finally {
      loadingMore.current = false;
    }
  }, [textQuery, filter, appId, clips.length, hasMore]);

  reloadRef.current = reload;

  // useEffect
  // On <html>, not the panel root, so a card's floating copy (appended to <body>) keeps the side sizes too.
  useEffect(() => {
    document.documentElement.toggleAttribute('data-side', side);
  }, [side]);

  // Square side cards: warn once when the pinned ones leave no room for history on this panel, again only
  // after another pin, a resize or another monitor changes the picture.
  useEffect(() => {
    if (!open || !side) return;
    const fits = pinsThatFit(window.innerWidth, window.innerHeight);
    const key = `${window.innerWidth}x${window.innerHeight}:${pinned.length}`;
    if (pinned.length <= fits || warnedPins.current === key) return;
    warnedPins.current = key;
    void warnSidePins(fits);
  }, [open, side, pinned.length]);

  useEffect(() => {
    void reload(false);
  }, [reload]);

  useEffect(() => {
    // New results (or the list closing) start from the first app.
    setSuggestIndex(0);
  }, [suggestions]);

  useEffect(() => {
    const offChanged = listen(
      'clips://changed',
      () => void reloadRef.current(keepSelectionOnReload(openRef.current, false), true),
    );
    // A copy, even one made while the panel is hidden, selects the newest card.
    const offAdded = listen(
      'clips://added',
      () => void reloadRef.current(keepSelectionOnReload(openRef.current, true), true),
    );
    const offOpened = listen('panel://opened', () => {
      lastPointer.current = null;
      setClosing(false);
      inputRef.current?.focus();
      // The window is shown transparent; reveal it once the parked frame is on screen, then slide.
      requestAnimationFrame(() =>
        requestAnimationFrame(
          () =>
            void revealPanel().finally(() => {
              setOpen(true);
              setEntering(true);
              window.clearTimeout(enterTimer.current);
              enterTimer.current = window.setTimeout(() => setEntering(false), ENTER_MS);
            }),
        ),
      );
    });
    const offClosing = listen('panel://closing', () => {
      // A card floating under the cursor lives outside the panel; it must not outlast it.
      cancelFloat();
      setClosing(true);
    });
    // Reset while hidden so the next open slides in finished content.
    const offClosed = listen('panel://closed', () => {
      lastPointer.current = null;
      hoverSelected.current = false;
      setOpen(false);
      window.clearTimeout(enterTimer.current);
      setEntering(false);
      setClosing(false);
      setDragging(false);
      resetView();
    });
    const offDragCancelled = listen('panel://drag-cancelled', () => {
      dragCancelledAt.current = Date.now();
      setDragging(false);
    });
    // A pinned panel never closed, so the shortcut or tray only hands focus back; start fresh like an open.
    const offRefocused = listen('panel://refocused', () => {
      hoverSelected.current = false;
      resetView();
      inputRef.current?.focus();
    });
    return () => {
      void offChanged.then((off) => off());
      void offAdded.then((off) => off());
      void offOpened.then((off) => off());
      void offClosing.then((off) => off());
      void offClosed.then((off) => off());
      void offDragCancelled.then((off) => off());
      void offRefocused.then((off) => off());
    };
  }, []);

  useEffect(() => {
    if (hoverSelected.current) hoverSelected.current = false;
    else
      bodyRef.current
        ?.querySelector(`[data-id="${selectedId}"]`)
        ?.scrollIntoView({ block: 'nearest', inline: 'nearest' });
    if (selected >= items.length - 5) void loadMore();
  }, [selectedId, selected, items.length, loadMore]);

  // useLayoutEffect
  useLayoutEffect(() => {
    const before = flipFrom.current;
    flipFrom.current = null;
    if (before && bodyRef.current) playFlip(bodyRef.current, before, side);
  }, [clips, pinned]);

  // handle
  /** Back to the newest card with no pending confirmation; the search, tag and filter stay. */
  const resetView = () => {
    setConfirmingId(null);
    setUpdateConfirming(false);
    setUpdateFailed(false);
    setUpdateInstalling(false);
    setPinNotice(false);
    void reloadRef.current(false);
  };

  const copy = (clip: Clip | undefined, plain = false) => {
    if (clip) void copyClip(clip.id, plain).catch(() => {});
  };

  const showPinNotice = () => {
    window.clearTimeout(noticeTimer.current);
    setPinNotice(true);
    noticeTimer.current = window.setTimeout(() => setPinNotice(false), 2000);
  };

  const togglePin = (clip: Clip | undefined) => {
    if (!clip) return;
    setPinned(clip.id, !clip.pinned).catch((e: AppError) => {
      if (e.code === 'PIN_LIMIT') showPinNotice();
    });
  };

  const dragOut = (clip: Clip, image: string | null = null) => {
    setDragging(true);
    void startDrag(clip.id, dragFileName(clip.lastUsedAt), image).catch(() => setDragging(false));
  };

  /** Floats the card under the cursor first; the OS drag starts once it leaves the panel. */
  const liftCard = (clip: Clip, card: HTMLElement, x: number, y: number) => {
    if (reducedMotion()) dragOut(clip);
    else floatCard(card, x, y, (image) => dragOut(clip, image));
  };

  const remove = (clip: Clip | undefined) => {
    if (clip) void deleteClip(clip.id);
  };

  const searching = query !== '' || app !== null || filter !== 'all';
  /** Clears the search, app tag and filter; the reload then selects the newest card. */
  const resetSearch = () => {
    clearSearch();
    inputRef.current?.focus();
  };

  const pickApp = (picked: App) => {
    setApp(picked);
    setQuery('');
  };

  const togglePanelPin = () => {
    const next = !panelPinned;
    setPanelPinned(next);
    // Without motion there is no sweep to end the press, so the border shows at once.
    setPinPressing(next && !reducedMotion());
    void setPanelPinnedApi(next);
  };

  const installUpdate = () => {
    if (updateInstalling) return;
    setUpdateFailed(false);
    setUpdateInstalling(true);
    // Resolves only on failure: success restarts the app.
    void installUpdateApi().catch(() => {
      setUpdateFailed(true);
      setUpdateInstalling(false);
    });
  };

  const cancelUpdate = () => {
    // Too late once Restart was pressed: the backend restarts when its check ends.
    if (updateInstalling) return;
    setUpdateConfirming(false);
    setUpdateFailed(false);
    setUpdateInstalling(false);
  };

  const onKeyDown = (e: KeyboardEvent) => {
    const accel = acceleratorFromEvent(e, isMac);
    const pinnedCopy =
      settings && accel
        ? [settings.pinnedCopy1Shortcut, settings.pinnedCopy2Shortcut, settings.pinnedCopy3Shortcut].indexOf(accel)
        : -1;
    const action = panelKeyAction({
      key: e.key,
      shiftKey: e.shiftKey,
      // WebKit reports keyCode 229 for the Enter that commits a Hangul syllable.
      isComposing: e.nativeEvent.isComposing || e.keyCode === 229,
      queryEmpty: query === '',
      repeat: e.repeat,
      confirming: confirmingId !== null || updateConfirming,
      suggesting,
      hasTag: app !== null,
      metaOrCtrl: e.metaKey || e.ctrlKey,
      code: e.code,
      panelPinCombo: accel === settings?.panelPinShortcut,
      resetCombo: accel === settings?.resetSearchShortcut,
      pinnedCopy: pinnedCopy === -1 ? null : (pinnedCopy as PinnedSlot),
      justCancelledDrag: Date.now() - dragCancelledAt.current < DRAG_CANCEL_ESC_MS,
      side,
    });
    if (!action) return;
    e.preventDefault();
    switch (action.type) {
      case 'move':
        setSelected((s) => clampSelection(s + action.delta, items.length));
        break;
      case 'copy':
        copy(items[selected], action.plain);
        break;
      case 'copyPinned':
        // An empty slot does nothing; the key is still swallowed so no digit lands in the search.
        if (pinned[action.index]) copy(pinned[action.index], false);
        break;
      case 'togglePin':
        togglePin(items[selected]);
        break;
      case 'togglePanelPin':
        togglePanelPin();
        break;
      case 'resetSearch':
        resetSearch();
        break;
      case 'hide':
        // hide_panel would still hand focus to the previous app.
        if (!panelPinned) void hidePanel();
        break;
      case 'openSettings':
        void openSettings();
        break;
      case 'cycleFilter': {
        const n = FILTERS.length;
        setFilter(FILTERS[(FILTERS.indexOf(filter) + action.delta + n) % n]);
        break;
      }
      case 'delete': {
        const clip = items[selected];
        if (settings?.confirmDelete === 'on') setConfirmingId(clip?.id ?? null);
        else remove(clip);
        break;
      }
      case 'confirmDelete':
        if (updateConfirming) {
          installUpdate();
          break;
        }
        remove(items.find((c) => c.id === confirmingId));
        setConfirmingId(null);
        break;
      case 'suggestMove':
        setSuggestIndex((i) => Math.max(0, Math.min(i + action.delta, suggestions.length - 1)));
        break;
      case 'suggestPick': {
        const picked = suggestions[suggestIndex];
        if (picked) pickApp(picked);
        break;
      }
      case 'suggestClose':
        setQuery('');
        break;
      case 'clearTag':
        setApp(null);
        break;
      case 'cancelDelete':
        setConfirmingId(null);
        cancelUpdate();
        break;
    }
  };

  const onWheel = (e: WheelEvent<HTMLDivElement>) => {
    if (!side && Math.abs(e.deltaY) > Math.abs(e.deltaX)) e.currentTarget.scrollLeft += e.deltaY;
  };

  const onScroll = (e: UIEvent<HTMLDivElement>) => {
    const el = e.currentTarget;
    const near = side
      ? el.scrollTop + el.clientHeight > el.scrollHeight - 400
      : el.scrollLeft + el.clientWidth > el.scrollWidth - 400;
    if (near) void loadMore();
  };

  /** `i` is the card's index in `items`. */
  const renderCard = (clip: Clip, i: number) => (
    <Card
      key={clip.id}
      clip={clip}
      job={jobs.get(clip.id)}
      selected={i === selected}
      onSelect={() => {
        setSelected(i);
        setConfirmingId(null);
      }}
      // Hovering only moves the selection; a pending delete confirmation stays.
      onHover={() => {
        if (!pointerMovedNow.current || dragging || i === selected) return;
        hoverSelected.current = true;
        setSelected(i);
      }}
      onCopy={(plain) => copy(clip, plain)}
      onTogglePin={() => togglePin(clip)}
      onDragOut={!clip.missing ? (card, x, y) => liftCard(clip, card, x, y) : undefined}
      confirming={clip.id === confirmingId}
      onConfirmDelete={() => {
        remove(clip);
        setConfirmingId(null);
      }}
      onCancelDelete={() => setConfirmingId(null)}
      enterIndex={Math.min(i, 8)}
    />
  );

  return (
    <div
      className={cx(
        // Thumbnails must not start WebKit's own HTML drag.
        'bg-base-100 relative flex h-full flex-col overflow-hidden rounded-[18px] border px-3 pt-2.5 pb-3 [&_img]:[-webkit-user-drag:none]',
        // While pinning plays, the border waits: the sweep's last lap hands over to it.
        panelPinned && !pinPressing ? 'border-primary shadow-panel-pinned' : 'border-base-300',
        pinPressing && 'animate-pin-press',
        /*
         * Entrance: the window appears instantly with the panel parked past its docked edge (set
         * while hidden), then the panel slides in like a sheet. Closing, or dragging a file out so the
         * whole screen is a drop target, slides it back out.
         */
        (!open || closing || dragging) && PARKED[position],
        open && 'transition-[translate] motion-reduce:transition-none',
        open &&
          (closing || dragging ? 'duration-200 ease-[ease-in]' : 'duration-300 ease-[cubic-bezier(0.32,0.72,0,1)]'),
      )}
      data-entering={entering || undefined}
      onKeyDown={onKeyDown}
      // Capture runs before the card's handler, so the card sees this event's verdict.
      onMouseMoveCapture={(e) => {
        pointerMovedNow.current = pointerMoved(lastPointer.current, e.screenX, e.screenY);
        lastPointer.current = { x: e.screenX, y: e.screenY };
      }}
      // Keep keyboard focus in the search box no matter what is clicked.
      onMouseDown={(e) => {
        if (e.target !== inputRef.current) e.preventDefault();
        if (!(e.target as Element).closest('.update')) cancelUpdate();
      }}
    >
      {/* The glow sits behind the toolbar only. */}
      <Aurora className="inset-x-0 top-0 h-[72px]" />
      {pinPressing && (
        // The sweep ends last: its end shows the pinned border.
        <span
          className="animate-pin-sweep pin-sweep pointer-events-none absolute inset-0 z-10 rounded-[inherit] motion-reduce:hidden"
          onAnimationEnd={() => setPinPressing(false)}
          aria-hidden
        />
      )}
      {!closing && <ResizeHandle />}
      <Toolbar
        query={query}
        onQuery={setQuery}
        filter={filter}
        onFilter={setFilter}
        inputRef={inputRef}
        app={app}
        onClearApp={() => setApp(null)}
        suggesting={suggesting}
        suggestions={suggestions}
        suggestIndex={suggestIndex}
        onHoverApp={setSuggestIndex}
        onPickApp={pickApp}
        onSettings={() => void openSettings()}
        panelPinned={panelPinned}
        panelPinPressing={pinPressing}
        panelPinShortcut={settings?.panelPinShortcut ?? ''}
        searching={searching}
        onResetSearch={resetSearch}
        resetSearchShortcut={settings?.resetSearchShortcut ?? ''}
        onTogglePanelPin={togglePanelPin}
        updateVersion={updateVersion}
        updateConfirming={updateConfirming}
        updateFailed={updateFailed}
        updateInstalling={updateInstalling}
        onUpdate={() => {
          setConfirmingId(null);
          if (updateConfirming) cancelUpdate();
          else setUpdateConfirming(true);
        }}
        onConfirmUpdate={installUpdate}
        onCancelUpdate={cancelUpdate}
      />
      {/* Pins first (left, or on top when docked to a side), then the scrolling history. */}
      <div className={cx('relative z-1 flex min-h-0 flex-1', side && 'flex-col')} ref={bodyRef}>
        {pinned.length > 0 && (
          <>
            <div className={cx('relative flex flex-none gap-2.5 p-1 pt-1.5', side && 'flex-col')} role="listbox">
              {pinned.map((clip, i) => renderCard(clip, i))}
              {pinNotice && (
                <div
                  className="rounded-box border-base-300 bg-card shadow-suggest absolute top-3 left-1/2 z-3 -translate-x-1/2 border px-3 py-2 text-[12px] font-semibold whitespace-nowrap"
                  role="status"
                >
                  {t.pinLimit}
                </div>
              )}
            </div>
            <div className={cx('bg-base-300 flex-none', side ? 'mx-2 my-1.5 h-px' : 'mx-1.5 my-2 w-px')} aria-hidden />
          </>
        )}
        {clips.length === 0 ? (
          <div className="text-muted flex flex-1 items-center justify-center">
            {query || filter !== 'all' || app ? t.noResults : t.empty}
          </div>
        ) : (
          <div
            className={cx(
              'flex min-w-0 flex-1 [scrollbar-width:none] gap-2.5 p-1 [&::-webkit-scrollbar]:hidden',
              // Room above the first card for its hover lift and selection ring, which the scroll box would clip.
              side
                ? 'min-h-0 scroll-pt-2.5 flex-col overflow-x-hidden overflow-y-auto pt-2.5'
                : 'overflow-x-auto overflow-y-hidden pt-1.5',
            )}
            ref={rowRef}
            role="listbox"
            onWheel={onWheel}
            onScroll={onScroll}
          >
            {clips.map((clip, i) => renderCard(clip, pinned.length + i))}
          </div>
        )}
      </div>
    </div>
  );
}
