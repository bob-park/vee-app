import { useCallback, useEffect, useRef, useState, type KeyboardEvent, type UIEvent, type WheelEvent } from "react";
import { listen } from "@tauri-apps/api/event";
import { api, type Clip, type Filter } from "../api.ts";
import { usePrefs } from "../prefs.tsx";
import { Card } from "./Card.tsx";
import { panelKeyAction } from "./keys.ts";
import { FILTERS, Toolbar } from "./Toolbar.tsx";
import "./panel.css";

const PAGE = 50;

export function Panel() {
  const { t } = usePrefs();
  const [query, setQuery] = useState("");
  const [filter, setFilter] = useState<Filter>("all");
  const [clips, setClips] = useState<Clip[]>([]);
  const [selected, setSelected] = useState(0);
  const [hasMore, setHasMore] = useState(false);
  const [open, setOpen] = useState(false);
  const requestId = useRef(0);
  const loadingMore = useRef(false);
  const inputRef = useRef<HTMLInputElement>(null);
  const rowRef = useRef<HTMLDivElement>(null);

  /** Reloads the first page. Responses from superseded searches are dropped. */
  const reload = useCallback(
    async (keepSelection: boolean) => {
      const id = ++requestId.current;
      const page = await api.listClips(query, filter, 0, PAGE);
      if (id !== requestId.current) return;
      setClips(page);
      setHasMore(page.length === PAGE);
      setSelected((s) => (keepSelection ? Math.max(0, Math.min(s, page.length - 1)) : 0));
      if (!keepSelection) rowRef.current?.scrollTo({ left: 0 });
    },
    [query, filter],
  );

  const loadMore = useCallback(async () => {
    if (!hasMore || loadingMore.current) return;
    loadingMore.current = true;
    const id = requestId.current;
    try {
      const page = await api.listClips(query, filter, clips.length, PAGE);
      if (id !== requestId.current) return;
      setClips((prev) => [...prev, ...page]);
      setHasMore(page.length === PAGE);
    } finally {
      loadingMore.current = false;
    }
  }, [query, filter, clips.length, hasMore]);

  useEffect(() => {
    void reload(false);
  }, [reload]);

  // Subscribe once; always call the latest reload.
  const reloadRef = useRef(reload);
  reloadRef.current = reload;
  useEffect(() => {
    const offChanged = listen("clips://changed", () => void reloadRef.current(true));
    const offOpened = listen("panel://opened", () => {
      setOpen(true);
      inputRef.current?.focus();
    });
    // Reset while hidden so the next open slides in finished content.
    const offClosed = listen("panel://closed", () => {
      setOpen(false);
      setQuery("");
      setFilter("all");
      void reloadRef.current(false);
    });
    return () => {
      void offChanged.then((off) => off());
      void offOpened.then((off) => off());
      void offClosed.then((off) => off());
    };
  }, []);

  useEffect(() => {
    rowRef.current?.children[selected]?.scrollIntoView({ block: "nearest", inline: "nearest" });
    if (selected >= clips.length - 5) void loadMore();
  }, [selected, clips.length, loadMore]);

  const copy = (clip: Clip | undefined) => {
    if (clip) void api.copyClip(clip.id).catch(() => {});
  };

  const remove = (clip: Clip | undefined) => {
    if (clip) void api.deleteClip(clip.id);
  };

  const onKeyDown = (e: KeyboardEvent) => {
    const action = panelKeyAction({
      key: e.key,
      shiftKey: e.shiftKey,
      // WebKit reports keyCode 229 for the Enter that commits a Hangul syllable.
      isComposing: e.nativeEvent.isComposing || e.keyCode === 229,
      queryEmpty: query === "",
      repeat: e.repeat,
    });
    if (!action) return;
    e.preventDefault();
    switch (action.type) {
      case "move":
        setSelected((s) => Math.max(0, Math.min(s + action.delta, clips.length - 1)));
        break;
      case "copy":
        copy(clips[selected]);
        break;
      case "hide":
        void api.hidePanel();
        break;
      case "cycleFilter": {
        const n = FILTERS.length;
        setFilter(FILTERS[(FILTERS.indexOf(filter) + action.delta + n) % n]);
        break;
      }
      case "delete":
        remove(clips[selected]);
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
      className={open ? "panel open" : "panel"}
      onKeyDown={onKeyDown}
      // Keep keyboard focus in the search box no matter what is clicked.
      onMouseDown={(e) => {
        if (e.target !== inputRef.current) e.preventDefault();
      }}
    >
      <Toolbar
        query={query}
        onQuery={setQuery}
        filter={filter}
        onFilter={setFilter}
        inputRef={inputRef}
        onSettings={() => void api.openSettings()}
      />
      {clips.length === 0 ? (
        <div className="empty">{query || filter !== "all" ? t.noResults : t.empty}</div>
      ) : (
        <div className="row" ref={rowRef} role="listbox" onWheel={onWheel} onScroll={onScroll}>
          {clips.map((clip, i) => (
            <Card
              key={clip.id}
              clip={clip}
              selected={i === selected}
              onSelect={() => setSelected(i)}
              onCopy={() => copy(clip)}
            />
          ))}
        </div>
      )}
    </div>
  );
}
