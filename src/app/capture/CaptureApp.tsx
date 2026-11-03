import { type PointerEvent, useCallback, useEffect, useMemo, useRef, useState } from 'react';

import { cancelCapture, getCaptureSession, startRecording, takeCapture } from '@/domain/capture/apis/capture';
import type { CaptureOpen, Point, Rect, Selection } from '@/domain/capture/apis/capture.dto';
import { useEngineStatus } from '@/domain/convert/queries/engines';
import { RECORD_FORMATS, type RecordFormat } from '@/domain/settings/apis/settings.dto';
import { useSettings } from '@/domain/settings/queries/settings';
import Segmented from '@/shared/components/Segmented';
import { useI18n } from '@/shared/providers/i18n/I18nProvider';
import { normalizeRect, windowAt } from '@/utils/captureSelection';
import { initialFormat, needsFfmpeg } from '@/utils/recording';

import { listen } from '@tauri-apps/api/event';
import cx from 'classnames';

/** Room between the selection and its size label. */
const LABEL_GAP = 6;
const isMac = navigator.userAgent.includes('Mac');

/** A record-mode selection waiting for Record, with the rect it covers. */
interface Pending {
  selection: Selection;
  rect: Rect;
}

/** The selection overlay: drag a region, or Space to pick a window. Shared by capture and record. */
export default function CaptureApp() {
  // ref
  const openRef = useRef<CaptureOpen | null>(null);

  // state
  const [open, setOpen] = useState<CaptureOpen | null>(null);
  const [windowMode, setWindowMode] = useState(false);
  const [pointer, setPointer] = useState<Point | null>(null);
  const [dragFrom, setDragFrom] = useState<Point | null>(null);
  const [pending, setPending] = useState<Pending | null>(null);
  /** The format picked in the pill; null until the user picks one. */
  const [picked, setPicked] = useState<RecordFormat | null>(null);

  // hooks
  const { t } = useI18n();

  // queries
  const { settings } = useSettings();
  const { status: ffmpegStatus } = useEngineStatus('ffmpeg');

  const ffmpeg = ffmpegStatus?.state === 'installed';
  const format = picked ?? initialFormat(settings?.recordFormat ?? 'mp4', ffmpeg, isMac);

  // callback
  // Before the effects: the Enter key starts it.
  const handleRecord = useCallback(() => {
    if (!pending) return;
    openRef.current = null;
    setOpen(null);
    setPending(null);
    // A refused start (FFmpeg gone, session expired) must not leave the overlay up with nothing drawn.
    void startRecording(pending.selection, format).catch(() => cancelCapture());
  }, [pending, format]);

  // useEffect
  useEffect(() => {
    // Rust shows the overlay only once the page has claimed the session, so a page that isn't ready never
    // leaves an invisible window behind. Claim on mount too, in case `capture://open` came before `listen`.
    const claim = () =>
      void getCaptureSession().then((session) => {
        if (!session) return;
        openRef.current = session;
        setOpen(session);
        setWindowMode(false);
        setPointer(null);
        setDragFrom(null);
        setPending(null);
        setPicked(null);
      });
    claim();
    const off = listen('capture://open', claim);
    return () => void off.then((fn) => fn());
  }, []);

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      // Esc always closes, even if the open event never arrived and nothing is drawn.
      if (e.key === 'Escape') {
        e.preventDefault();
        openRef.current = null;
        setOpen(null);
        void cancelCapture();
      } else if (e.code === 'Space' && !e.repeat && openRef.current) {
        e.preventDefault();
        setDragFrom(null);
        setWindowMode((on) => !on);
      }
    };
    window.addEventListener('keydown', onKey);
    return () => window.removeEventListener('keydown', onKey);
  }, []);

  useEffect(() => {
    if (!pending) return;
    const onKey = (e: KeyboardEvent) => {
      // A focused button (Record, ✕) acts on Enter itself.
      if (e.key === 'Enter' && !e.repeat && !(e.target instanceof HTMLButtonElement)) {
        e.preventDefault();
        handleRecord();
      }
    };
    window.addEventListener('keydown', onKey);
    return () => window.removeEventListener('keydown', onKey);
  }, [pending, handleRecord]);

  // handle
  const reset = () => {
    openRef.current = null;
    setOpen(null);
    setDragFrom(null);
    setPointer(null);
    setPending(null);
  };

  const handleCancel = () => {
    reset();
    void cancelCapture();
  };

  /** Capture grabs it now; record waits in the pill, and a new drag or click replaces it. */
  const select = (selection: Selection, rect: Rect) => {
    if (openRef.current?.mode === 'record') {
      setPending({ selection, rect });
      return;
    }
    reset();
    void takeCapture(selection);
  };

  const point = (e: PointerEvent): Point => ({ x: e.clientX, y: e.clientY });

  const handlePointerDown = (e: PointerEvent) => {
    if (e.button !== 0 || !open) return;
    if (windowMode) {
      const win = windowAt(open.windows, point(e));
      if (win) select({ kind: 'window', id: win.id }, win);
      return;
    }
    e.currentTarget.setPointerCapture(e.pointerId);
    setDragFrom(point(e));
    setPointer(point(e));
  };

  const handlePointerUp = (e: PointerEvent) => {
    if (!dragFrom) return;
    const rect = normalizeRect(dragFrom, point(e));
    setDragFrom(null);
    if (rect) select({ kind: 'rect', ...rect }, rect);
  };

  // memorize
  const hole = useMemo<Rect | null>(() => {
    if (!open) return null;
    const drawn = pending?.rect ?? null;
    if (!pointer) return drawn;
    if (windowMode) return windowAt(open.windows, pointer) ?? drawn;
    return dragFrom ? normalizeRect(dragFrom, pointer) : drawn;
  }, [open, pointer, windowMode, dragFrom, pending]);

  if (!open) return null;
  const r = t.record;
  const { width, height, scale } = open.monitor;
  const holePath = hole ? ` M${hole.x} ${hole.y}h${hole.width}v${hole.height}h${-hole.width}Z` : '';
  const labelBelow = hole && hole.y + hole.height + LABEL_GAP + 24 < height;
  return (
    <div
      className={cx('fixed inset-0 select-none', windowMode ? 'cursor-camera' : 'cursor-crosshair')}
      onPointerDown={handlePointerDown}
      onPointerMove={(e) => setPointer(point(e))}
      onPointerUp={handlePointerUp}
    >
      <svg className="absolute inset-0 size-full" aria-hidden>
        {windowMode ? (
          // Like macOS: the screen stays as it is and only the window to be taken is tinted.
          hole && <rect className="fill-pick" x={hole.x} y={hole.y} width={hole.width} height={hole.height} />
        ) : (
          <>
            <path className="fill-black/35" fillRule="evenodd" d={`M0 0H${width}V${height}H0Z${holePath}`} />
            {hole && (
              <rect
                className="stroke-primary fill-none"
                strokeWidth={1}
                x={hole.x}
                y={hole.y}
                width={hole.width}
                height={hole.height}
              />
            )}
          </>
        )}
      </svg>
      {hole && !windowMode && (
        <span
          className="glass text-base-content absolute rounded-md px-1.5 py-0.5 font-mono text-[11px] tabular-nums"
          style={{
            left: hole.x,
            top: labelBelow ? hole.y + hole.height + LABEL_GAP : hole.y + LABEL_GAP,
          }}
        >
          {Math.round(hole.width * scale)} × {Math.round(hole.height * scale)}
        </span>
      )}
      {pending && !dragFrom ? (
        // Solid, not glass: it floats over any desktop and must be read (aurora-glass §3). Its own clicks
        // must not start a new drag on the overlay.
        <div
          className="bg-card border-toast-border text-base-content shadow-toast absolute bottom-10 left-1/2 flex -translate-x-1/2 cursor-default items-center gap-1.5 rounded-full border p-1.5"
          onPointerDown={(e) => e.stopPropagation()}
        >
          <Segmented
            ariaLabel={r.format}
            value={format}
            onChange={setPicked}
            options={RECORD_FORMATS.map((f) => {
              const locked = !ffmpeg && needsFfmpeg(f, isMac);
              return { value: f, label: f.toUpperCase(), disabled: locked, title: locked ? r.ffmpegNeeded : undefined };
            })}
          />
          <button
            className="from-primary to-sky flex items-center gap-1.5 rounded-full bg-linear-to-r px-3 py-1.5 text-[12px] font-semibold text-white transition-transform duration-150 active:scale-96 motion-reduce:transition-none"
            onClick={handleRecord}
          >
            <span className="size-2 rounded-full bg-white" aria-hidden />
            {r.start}
          </button>
          <button
            className="hover:bg-subtle grid size-7.5 place-items-center rounded-full"
            aria-label={r.cancel}
            title={r.cancel}
            onClick={handleCancel}
          >
            <svg className="size-4" viewBox="0 0 24 24" aria-hidden>
              <path d="M6 6l12 12M18 6L6 18" stroke="currentColor" strokeWidth={2} strokeLinecap="round" fill="none" />
            </svg>
          </button>
        </div>
      ) : (
        !dragFrom && (
          <div className="glass text-base-content shadow-glass absolute bottom-10 left-1/2 -translate-x-1/2 rounded-full px-4 py-2 text-[13px]">
            {windowMode ? t.capture.windowHint : t.capture.regionHint}
          </div>
        )
      )}
    </div>
  );
}
