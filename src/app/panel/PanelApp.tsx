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
import { cancelFloat, cardLefts, floatCard, playFlip, reducedMotion } from '@/app/panel/_components/motion';
import { copyClip, deleteClip, listClips, listPinned, setPinned, startDrag } from '@/domain/clip/apis/clip';
import type { Clip } from '@/domain/clip/apis/clip.dto';
import { hidePanel, openSettings, revealPanel, setPanelPinned as setPanelPinnedApi } from '@/domain/panel/apis/panel';
import { useSettings } from '@/domain/settings/queries/settings';
import type { App } from '@/domain/source_app/apis/sourceApp.dto';
import { useAppSuggestions } from '@/domain/source_app/queries/sourceApp';
import { installUpdate as installUpdateApi } from '@/domain/updater/apis/updater';
import { useUpdateStatus } from '@/domain/updater/queries/updater';
import type { AppError } from '@/shared/api/common.dto';
import { useI18n } from '@/shared/providers/i18n/I18nProvider';
import { useStore } from '@/shared/store/rootStore';
import { acceleratorFromEvent } from '@/utils/accelerator';
import { dragFileName } from '@/utils/fileThumb';
import { panelKeyAction } from '@/utils/keys';
import {
  type Point,
  clampSelection,
  defaultSelection,
  followSelection,
  keepSelectionOnReload,
  pointerMoved,
} from '@/utils/selection';

import { listen } from '@tauri-apps/api/event';

import './_components/panel.css';

const PAGE = 50;
const isMac = navigator.userAgent.includes('Mac');

export function PanelApp() {
  // ref
  const noticeTimer = useRef<number | undefined>(undefined);
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
  const [closing, setClosing] = useState(false);
  const [dragging, setDragging] = useState(false);
  const [confirmingId, setConfirmingId] = useState<number | null>(null);
  const [updateConfirming, setUpdateConfirming] = useState(false);
  const [updateFailed, setUpdateFailed] = useState(false);
  const [panelPinned, setPanelPinned] = useState(false);
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
  const { suggestions } = useAppSuggestions(query.slice(1), suggesting);
  const { status: updateStatus } = useUpdateStatus();

  const updateVersion = updateStatus.status === 'ready' ? updateStatus.version : null;

  // callback (declared before the effects that depend on them)
  /** Reloads the first page. Responses from superseded searches are dropped. */
  const reload = useCallback(
    async (keepSelection: boolean, animate = false) => {
      const id = ++requestId.current;
      const [page, pins] = await Promise.all([listClips(textQuery, filter, appId, 0, PAGE), listPinned()]);
      if (id !== requestId.current) return;
      // Only copies arriving while the panel is on screen animate — not searches or filters.
      if (animate && openRef.current && !reducedMotion()) flipFrom.current = cardLefts(bodyRef.current);
      setPinnedClips(pins);
      setClips(page);
      setHasMore(page.length === PAGE);
      const ids = [...pins, ...page].map((c) => c.id);
      setSelected((s) =>
        keepSelection ? followSelection(selectedIdRef.current, s, ids) : defaultSelection(pins.length, page.length),
      );
      if (!keepSelection) {
        setConfirmingId(null);
        rowRef.current?.scrollTo({ left: 0 });
      }
    },
    [textQuery, filter, appId],
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
      requestAnimationFrame(() => requestAnimationFrame(() => void revealPanel().finally(() => setOpen(true))));
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
      setClosing(false);
      setDragging(false);
      resetView();
    });
    const offDragCancelled = listen('panel://drag-cancelled', () => setDragging(false));
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
    if (before && bodyRef.current) playFlip(bodyRef.current, before);
  }, [clips, pinned]);

  // handle
  /** Back to the newest card with no pending confirmation; the search, tag and filter stay. */
  const resetView = () => {
    setConfirmingId(null);
    setUpdateConfirming(false);
    setUpdateFailed(false);
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
    void setPanelPinnedApi(next);
  };

  const installUpdate = () => {
    setUpdateFailed(false);
    void installUpdateApi().catch(() => setUpdateFailed(true));
  };

  const cancelUpdate = () => {
    setUpdateConfirming(false);
    setUpdateFailed(false);
  };

  const onKeyDown = (e: KeyboardEvent) => {
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
      panelPinCombo: acceleratorFromEvent(e, isMac) === settings?.panelPinShortcut,
      resetCombo: acceleratorFromEvent(e, isMac) === settings?.resetSearchShortcut,
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
    if (Math.abs(e.deltaY) > Math.abs(e.deltaX)) e.currentTarget.scrollLeft += e.deltaY;
  };

  const onScroll = (e: UIEvent<HTMLDivElement>) => {
    const el = e.currentTarget;
    if (el.scrollLeft + el.clientWidth > el.scrollWidth - 400) void loadMore();
  };

  /** `i` is the card's index in `items`. */
  const renderCard = (clip: Clip, i: number) => (
    <Card
      key={clip.id}
      clip={clip}
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
    />
  );

  return (
    <div
      className={['panel', open && 'open', closing && 'closing', dragging && 'dragging', panelPinned && 'pinned']
        .filter(Boolean)
        .join(' ')}
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
        onPickApp={pickApp}
        onSettings={() => void openSettings()}
        panelPinned={panelPinned}
        panelPinShortcut={settings?.panelPinShortcut ?? ''}
        searching={searching}
        onResetSearch={resetSearch}
        resetSearchShortcut={settings?.resetSearchShortcut ?? ''}
        onTogglePanelPin={togglePanelPin}
        updateVersion={updateVersion}
        updateConfirming={updateConfirming}
        updateFailed={updateFailed}
        onUpdate={() => {
          setConfirmingId(null);
          if (updateConfirming) cancelUpdate();
          else setUpdateConfirming(true);
        }}
        onConfirmUpdate={installUpdate}
        onCancelUpdate={cancelUpdate}
      />
      <div className="body" ref={bodyRef}>
        {pinned.length > 0 && (
          <>
            <div className="pinned-row" role="listbox">
              {pinned.map((clip, i) => renderCard(clip, i))}
              {pinNotice && (
                <div className="pin-notice" role="status">
                  {t.pinLimit}
                </div>
              )}
            </div>
            <div className="pin-divider" aria-hidden />
          </>
        )}
        {clips.length === 0 ? (
          <div className="empty">{query || filter !== 'all' || app ? t.noResults : t.empty}</div>
        ) : (
          <div className="row" ref={rowRef} role="listbox" onWheel={onWheel} onScroll={onScroll}>
            {clips.map((clip, i) => renderCard(clip, pinned.length + i))}
          </div>
        )}
      </div>
    </div>
  );
}
