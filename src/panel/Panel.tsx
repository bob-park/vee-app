import { useCallback, useEffect, useLayoutEffect, useRef, useState, type KeyboardEvent, type UIEvent, type WheelEvent } from "react";
import { listen } from "@tauri-apps/api/event";
import { api, type App, type Clip, type Filter, type UpdateStatus } from "../api.ts";
import { usePrefs } from "../prefs.tsx";
import { Card } from "./Card.tsx";
import { dragFileName } from "./fileThumb.ts";
import { panelKeyAction } from "./keys.ts";
import { cancelFloat, cardLefts, floatCard, playFlip, reducedMotion } from "./motion.ts";
import { FILTERS, Toolbar } from "./Toolbar.tsx";
import "./panel.css";

const PAGE = 50;

export function Panel() {
  const { t, settings } = usePrefs();
  const [query, setQuery] = useState("");
  const [filter, setFilter] = useState<Filter>("all");
  const [clips, setClips] = useState<Clip[]>([]);
  const [selected, setSelected] = useState(0);
  const [hasMore, setHasMore] = useState(false);
  const [open, setOpen] = useState(false);
  const [closing, setClosing] = useState(false);
  const [dragging, setDragging] = useState(false);
  const [confirmingId, setConfirmingId] = useState<number | null>(null);
  const [updateVersion, setUpdateVersion] = useState<string | null>(null);
  const [updateConfirming, setUpdateConfirming] = useState(false);
  const [updateFailed, setUpdateFailed] = useState(false);
  const [app, setApp] = useState<App | null>(null);
  const [suggestions, setSuggestions] = useState<App[]>([]);
  const [suggestIndex, setSuggestIndex] = useState(0);
  const requestId = useRef(0);
  const loadingMore = useRef(false);
  const inputRef = useRef<HTMLInputElement>(null);
  const rowRef = useRef<HTMLDivElement>(null);
  /** Card positions captured just before an animated reload, consumed by the next layout. */
  const flipFrom = useRef<Map<number, number> | null>(null);
  const openRef = useRef(open);
  openRef.current = open;

  // While `@…` is being typed it names an app, not text to search for.
  const suggesting = app === null && query.startsWith("@");
  const textQuery = suggesting ? "" : query;
  const appId = app?.id ?? null;

  /** Reloads the first page. Responses from superseded searches are dropped. */
  const reload = useCallback(
    async (keepSelection: boolean, animate = false) => {
      const id = ++requestId.current;
      const page = await api.listClips(textQuery, filter, appId, 0, PAGE);
      if (id !== requestId.current) return;
      // Only copies arriving while the panel is on screen animate — not searches or filters.
      if (animate && openRef.current && !reducedMotion()) flipFrom.current = cardLefts(rowRef.current);
      setClips(page);
      setHasMore(page.length === PAGE);
      setSelected((s) => (keepSelection ? Math.max(0, Math.min(s, page.length - 1)) : 0));
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
      const page = await api.listClips(textQuery, filter, appId, clips.length, PAGE);
      if (id !== requestId.current) return;
      setClips((prev) => [...prev, ...page]);
      setHasMore(page.length === PAGE);
    } finally {
      loadingMore.current = false;
    }
  }, [textQuery, filter, appId, clips.length, hasMore]);

  useEffect(() => {
    void reload(false);
  }, [reload]);

  useEffect(() => {
    // Closing the list (pick, esc, panel closed) drops its results so the next `@` never shows stale apps.
    if (!suggesting) {
      setSuggestions([]);
      setSuggestIndex(0);
      return;
    }
    let live = true;
    void api.listApps(query.slice(1)).then((apps) => {
      if (!live) return;
      setSuggestions(apps);
      setSuggestIndex(0);
    });
    return () => {
      live = false;
    };
  }, [suggesting, query]);

  useEffect(() => {
    const apply = (s: UpdateStatus) => setUpdateVersion(s.status === "ready" ? s.version : null);
    void api.getUpdateStatus().then(apply);
    const unlisten = listen<UpdateStatus>("update://status", (e) => apply(e.payload));
    return () => void unlisten.then((off) => off());
  }, []);

  // Subscribe once; always call the latest reload.
  const reloadRef = useRef(reload);
  reloadRef.current = reload;
  useEffect(() => {
    const offChanged = listen("clips://changed", () => void reloadRef.current(true, true));
    const offOpened = listen("panel://opened", () => {
      setClosing(false);
      inputRef.current?.focus();
      // The window is shown transparent; reveal it once the parked frame is on screen, then slide.
      requestAnimationFrame(() =>
        requestAnimationFrame(() => void api.revealPanel().finally(() => setOpen(true))),
      );
    });
    const offClosing = listen("panel://closing", () => {
      // A card floating under the cursor lives outside the panel; it must not outlast it.
      cancelFloat();
      setClosing(true);
    });
    // Reset while hidden so the next open slides in finished content.
    const offClosed = listen("panel://closed", () => {
      setOpen(false);
      setClosing(false);
      setDragging(false);
      setConfirmingId(null);
      setUpdateConfirming(false);
      setUpdateFailed(false);
      setQuery("");
      setApp(null);
      setFilter("all");
      void reloadRef.current(false);
    });
    const offDragCancelled = listen("panel://drag-cancelled", () => setDragging(false));
    return () => {
      void offChanged.then((off) => off());
      void offOpened.then((off) => off());
      void offClosing.then((off) => off());
      void offClosed.then((off) => off());
      void offDragCancelled.then((off) => off());
    };
  }, []);

  useLayoutEffect(() => {
    const before = flipFrom.current;
    flipFrom.current = null;
    if (before && rowRef.current) playFlip(rowRef.current, before);
  }, [clips]);

  useEffect(() => {
    rowRef.current?.children[selected]?.scrollIntoView({ block: "nearest", inline: "nearest" });
    if (selected >= clips.length - 5) void loadMore();
  }, [selected, clips.length, loadMore]);

  const copy = (clip: Clip | undefined, plain = false) => {
    if (clip) void api.copyClip(clip.id, plain).catch(() => {});
  };

  const togglePin = (clip: Clip | undefined) => {
    if (clip) void api.setPinned(clip.id, !clip.pinned);
  };

  const dragOut = (clip: Clip, image: string | null = null) => {
    setDragging(true);
    void api.startDrag(clip.id, dragFileName(clip.lastUsedAt), image).catch(() => setDragging(false));
  };

  /** Floats the card under the cursor first; the OS drag starts once it leaves the panel. */
  const liftCard = (clip: Clip, card: HTMLElement, x: number, y: number) => {
    if (reducedMotion()) dragOut(clip);
    else floatCard(card, x, y, (image) => dragOut(clip, image));
  };

  const remove = (clip: Clip | undefined) => {
    if (clip) void api.deleteClip(clip.id);
  };

  const pickApp = (picked: App) => {
    setApp(picked);
    setQuery("");
  };

  const installUpdate = () => {
    setUpdateFailed(false);
    void api.installUpdate().catch(() => setUpdateFailed(true));
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
      queryEmpty: query === "",
      repeat: e.repeat,
      confirming: confirmingId !== null || updateConfirming,
      suggesting,
      hasTag: app !== null,
      metaOrCtrl: e.metaKey || e.ctrlKey,
      code: e.code,
    });
    if (!action) return;
    e.preventDefault();
    switch (action.type) {
      case "move":
        setSelected((s) => Math.max(0, Math.min(s + action.delta, clips.length - 1)));
        break;
      case "copy":
        copy(clips[selected], action.plain);
        break;
      case "togglePin":
        togglePin(clips[selected]);
        break;
      case "hide":
        void api.hidePanel();
        break;
      case "cycleFilter": {
        const n = FILTERS.length;
        setFilter(FILTERS[(FILTERS.indexOf(filter) + action.delta + n) % n]);
        break;
      }
      case "delete": {
        const clip = clips[selected];
        if (settings.confirmDelete === "on") setConfirmingId(clip?.id ?? null);
        else remove(clip);
        break;
      }
      case "confirmDelete":
        if (updateConfirming) {
          installUpdate();
          break;
        }
        remove(clips.find((c) => c.id === confirmingId));
        setConfirmingId(null);
        break;
      case "suggestMove":
        setSuggestIndex((i) => Math.max(0, Math.min(i + action.delta, suggestions.length - 1)));
        break;
      case "suggestPick": {
        const picked = suggestions[suggestIndex];
        if (picked) pickApp(picked);
        break;
      }
      case "suggestClose":
        setQuery("");
        break;
      case "clearTag":
        setApp(null);
        break;
      case "cancelDelete":
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

  return (
    <div
      className={["panel", open && "open", closing && "closing", dragging && "dragging"].filter(Boolean).join(" ")}
      onKeyDown={onKeyDown}
      // Keep keyboard focus in the search box no matter what is clicked.
      onMouseDown={(e) => {
        if (e.target !== inputRef.current) e.preventDefault();
        if (!(e.target as Element).closest(".update")) cancelUpdate();
      }}
    >
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
        onSettings={() => void api.openSettings()}
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
      {clips.length === 0 ? (
        <div className="empty">{query || filter !== "all" || app ? t.noResults : t.empty}</div>
      ) : (
        <div className="row" ref={rowRef} role="listbox" onWheel={onWheel} onScroll={onScroll}>
          {clips.map((clip, i) => (
            <Card
              key={clip.id}
              clip={clip}
              selected={i === selected}
              onSelect={() => {
                setSelected(i);
                setConfirmingId(null);
              }}
              onCopy={(plain) => copy(clip, plain)}
              onTogglePin={() => togglePin(clip)}
              onDragOut={
                (clip.kind === "files" || clip.kind === "image") && !clip.missing
                  ? (card, x, y) => liftCard(clip, card, x, y)
                  : undefined
              }
              confirming={clip.id === confirmingId}
              onConfirmDelete={() => {
                remove(clip);
                setConfirmingId(null);
              }}
              onCancelDelete={() => setConfirmingId(null)}
            />
          ))}
        </div>
      )}
    </div>
  );
}
