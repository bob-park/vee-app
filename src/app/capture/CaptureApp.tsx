import { type PointerEvent, useCallback, useEffect, useMemo, useRef, useState } from 'react';

import { cancelCapture, getCaptureSession, startRecording, takeCapture } from '@/domain/capture/apis/capture';
import type { CaptureOpen, Point, Rect, Selection } from '@/domain/capture/apis/capture.dto';
import { useEngineStatus } from '@/domain/convert/queries/engines';
import { RECORD_FORMATS, type RecordFormat } from '@/domain/settings/apis/settings.dto';
import { useSettings } from '@/domain/settings/queries/settings';
import Segmented from '@/shared/components/Segmented';
import { useI18n } from '@/shared/providers/i18n/I18nProvider';
import { coveredParts, normalizeRect, overlayShapes, windowAt } from '@/utils/captureSelection';
import { initialFormat, needsFfmpeg } from '@/utils/recording';

import { emit, listen } from '@tauri-apps/api/event';
import { getCurrentWindow } from '@tauri-apps/api/window';
import cx from 'classnames';

/** Room between the selection and its size label. */
const LABEL_GAP = 6;
const isMac = navigator.userAgent.includes('Mac');
/** This overlay's window; there is one per monitor. */
const me = getCurrentWindow();

/** A record-mode selection waiting for Record, with the rect it covers. */
interface Pending {
  selection: Selection;
  rect: Rect;
}

/** The selection overlay on one monitor: drag a region, or Space to pick a window. Shared by capture and record.
 * The overlays of the other monitors follow its Space mode and drop their pick when this one picks. */
export default function CaptureApp() {
  // ref
  const openRef = useRef<CaptureOpen | null>(null);
  const windowModeRef = useRef(false);

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
        windowModeRef.current = false;
        setOpen(session);
        setWindowMode(false);
        setPointer(null);
        setDragFrom(null);
        setPending(null);
        setPicked(null);
      });
    claim();
    const offs = [
      listen('capture://open', claim),
      // Another monitor's overlay took the shot or Esc closed them all: draw nothing when shown again.
      listen('capture://closed', () => {
        openRef.current = null;
        setOpen(null);
        setPending(null);
      }),
      listen<boolean>('capture://mode', (e) => {
        windowModeRef.current = e.payload;
        setDragFrom(null);
        setWindowMode(e.payload);
      }),
      // One waiting record selection across the monitors.
      listen<string>('capture://picked', (e) => {
        if (e.payload !== me.label) setPending(null);
      }),
    ];
    return () => offs.forEach((off) => void off.then((fn) => fn()));
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
        // To every monitor's overlay, this one included.
        void emit('capture://mode', !windowModeRef.current);
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
      void emit('capture://picked', me.label);
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
  const shapes = useMemo(() => {
    const hover = open && pointer && windowMode ? windowAt(open.windows, pointer) : null;
    const drag = dragFrom && pointer ? normalizeRect(dragFrom, pointer) : null;
    // Only what shows of the window is tinted: the parts in front of it aren't taken.
    const covered = open && hover ? coveredParts(open.windows, hover) : [];
    return { ...overlayShapes(windowMode, hover, drag, pending?.rect ?? null), covered };
  }, [open, pointer, windowMode, dragFrom, pending]);

  if (!open) return null;
  const r = t.record;
  const { width, height, scale } = open.monitor;
  const { hole, tint, outline, planned, covered } = shapes;
  const holePath = hole ? ` M${hole.x} ${hole.y}h${hole.width}v${hole.height}h${-hole.width}Z` : '';
  const labelBelow = hole && hole.y + hole.height + LABEL_GAP + 24 < height;
  const box = (rect: Rect) => ({ x: rect.x, y: rect.y, width: rect.width, height: rect.height });
  return (
    <div
      className={cx('fixed inset-0 select-none', windowMode ? 'cursor-camera' : 'cursor-crosshair')}
      onPointerDown={handlePointerDown}
      onPointerMove={(e) => setPointer(point(e))}
      onPointerUp={handlePointerUp}
      // Off to another monitor: its overlay takes over (Rust moves the keyboard there), so drop the tint here.
      onPointerLeave={() => setPointer(null)}
    >
      <svg className="absolute inset-0 size-full" aria-hidden>
        {/* Like macOS: window mode leaves the screen as it is and only tints the window to be taken. */}
        {shapes.dim && (
          <path className="fill-black/35" fillRule="evenodd" d={`M0 0H${width}V${height}H0Z${holePath}`} />
        )}
        {tint && (
          <>
            <mask id="tint-visible">
              <rect className="fill-white" {...box(tint)} />
              {covered.map((part) => (
                <rect key={`${part.x},${part.y}`} className="fill-black" {...box(part)} />
              ))}
            </mask>
            <rect className="fill-pick" mask="url(#tint-visible)" {...box(tint)} />
          </>
        )}
        {outline && <rect className="stroke-primary fill-none" strokeWidth={1} {...box(outline)} />}
        {/* The area the recording will cover, in the recording border's dashes. */}
        {planned && <rect className="stroke-rec fill-none" strokeWidth={2} strokeDasharray="6 4" {...box(planned)} />}
      </svg>
      {hole && (
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
