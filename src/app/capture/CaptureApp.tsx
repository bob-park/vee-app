import { type PointerEvent, useCallback, useEffect, useMemo, useRef, useState } from 'react';

import CaptureToolbar from '@/app/capture/_components/CaptureToolbar';
import RecordPill from '@/app/capture/_components/RecordPill';
import {
  cancelCapture,
  getCaptureSession,
  setCaptureMode,
  startRecording,
  takeCapture,
} from '@/domain/capture/apis/capture';
import type { CaptureMode, CaptureOpen, Point, Rect, Selection } from '@/domain/capture/apis/capture.dto';
import { useEngineStatus } from '@/domain/convert/queries/engines';
import type { RecordFormat } from '@/domain/settings/apis/settings.dto';
import { useSettings } from '@/domain/settings/queries/settings';
import { coveredParts, normalizeRect, overlayShapes, windowAt } from '@/utils/captureSelection';
import { initialFormat } from '@/utils/recording';

import { emit, listen } from '@tauri-apps/api/event';
import { getCurrentWindow } from '@tauri-apps/api/window';
import cx from 'classnames';

/** Room between the selection and its size label. */
const LABEL_GAP = 6;
/** The overlay's fade out (its `duration-150`) before Rust hides it. */
const FADE_MS = 150;
const isMac = navigator.userAgent.includes('Mac');
/** A picked window's highlight follows the system's window corners. */
const WINDOW_RADIUS = isMac ? 12 : 8;
/** This overlay's window; there is one per monitor. */
const me = getCurrentWindow();

/** `capture://picked`: which overlay picked, and whether it picked a window. */
interface PickedEvent {
  label: string;
  window: boolean;
}

/** A record-mode selection waiting for Record, with the rect it covers. */
interface Pending {
  selection: Selection;
  rect: Rect;
}

/** Asks Rust to switch the session's mode; `capture://session-mode` then updates every overlay. A refused
 * switch (Rust answers with the old mode) changes nothing, so the answer is ignored. */
function requestMode(open: CaptureOpen | null, next: CaptureMode) {
  if (!open || next === open.mode || (next === 'record' && open.recordBlocked)) return;
  void setCaptureMode(next);
}

/** The selection overlay on one monitor: drag a region, or Space to pick a window. Shared by capture and record.
 * The overlays of the other monitors follow its Space mode and drop their pick when this one picks. */
export default function CaptureApp() {
  // ref
  const openRef = useRef<CaptureOpen | null>(null);
  const windowModeRef = useRef(false);
  /** Record was pressed: the overlay fades out while the recording frame fades in, then Rust hides it. */
  const leavingRef = useRef(false);
  /** The record pill is up here: Tab then moves focus through it instead of switching the mode. */
  const pillShownRef = useRef(false);

  // state
  const [open, setOpen] = useState<CaptureOpen | null>(null);
  const [windowMode, setWindowMode] = useState(false);
  const [pointer, setPointer] = useState<Point | null>(null);
  const [dragFrom, setDragFrom] = useState<Point | null>(null);
  const [pending, setPending] = useState<Pending | null>(null);
  /** A window was picked for recording on some monitor: window mode stops picking until the overlay closes. */
  const [windowPicked, setWindowPicked] = useState(false);
  const [leaving, setLeaving] = useState(false);
  /** The format picked in the pill; null until the user picks one. */
  const [picked, setPicked] = useState<RecordFormat | null>(null);

  // queries
  const { settings } = useSettings();
  const { status: ffmpegStatus } = useEngineStatus('ffmpeg');

  const ffmpeg = ffmpegStatus?.state === 'installed';
  const format = picked ?? initialFormat(settings?.recordFormat ?? 'mp4', ffmpeg, isMac);
  // Only the monitor under the mouse shows the toolbar and the pill.
  const showToolbar = pointer !== null && !dragFrom && !leaving;
  const showPill = showToolbar && pending !== null;

  // callback
  // Before the effects: the Enter key starts it.
  const handleRecord = useCallback(() => {
    if (!pending) return;
    // Drawing stays up (fading) until Rust closes the overlay behind the recording frame.
    openRef.current = null;
    leavingRef.current = true;
    setLeaving(true);
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
        leavingRef.current = false;
        setLeaving(false);
        setOpen(session);
        setWindowMode(false);
        // Known before the mouse moves, so Space tints the window under it at once.
        setPointer(session.cursor);
        setDragFrom(null);
        setPending(null);
        setWindowPicked(false);
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
        setWindowPicked(false);
        leavingRef.current = false;
        setLeaving(false);
      }),
      listen<boolean>('capture://mode', (e) => {
        windowModeRef.current = e.payload;
        setDragFrom(null);
        setWindowMode(e.payload);
      }),
      // Rust switched the session (this overlay asked, or another one did): start over in the new mode.
      listen<CaptureMode>('capture://session-mode', (e) => {
        const current = openRef.current;
        if (!current) return;
        const next = {
          ...current,
          mode: e.payload,
          recordBlocked: e.payload === 'record' ? null : current.recordBlocked,
        };
        openRef.current = next;
        setOpen(next);
        setPending(null);
        setWindowPicked(false);
        setDragFrom(null);
        setPicked(null);
      }),
      // One waiting record selection across the monitors.
      listen<PickedEvent>('capture://picked', (e) => {
        if (e.payload.label !== me.label) setPending(null);
        if (e.payload.window) setWindowPicked(true);
      }),
    ];
    return () => offs.forEach((off) => void off.then((fn) => fn()));
  }, []);

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      // Esc always closes, even if the open event never arrived and nothing is drawn.
      if (leavingRef.current) return;
      if (e.key === 'Escape') {
        e.preventDefault();
        // Fades out first; with nothing drawn (the open event never came) it just closes.
        openRef.current = null;
        leavingRef.current = true;
        setLeaving(true);
        setTimeout(() => void cancelCapture(), FADE_MS);
      } else if (e.code === 'Space' && !e.repeat && openRef.current) {
        e.preventDefault();
        // Here at once, then to every other monitor's overlay (this one gets it too, unchanged).
        const next = !windowModeRef.current;
        windowModeRef.current = next;
        setDragFrom(null);
        setWindowMode(next);
        void emit('capture://mode', next);
      } else if (e.key === 'Tab' && !e.repeat && openRef.current && !pillShownRef.current) {
        e.preventDefault();
        requestMode(openRef.current, openRef.current.mode === 'capture' ? 'record' : 'capture');
      }
    };
    window.addEventListener('keydown', onKey);
    return () => window.removeEventListener('keydown', onKey);
  }, []);

  useEffect(() => {
    pillShownRef.current = showPill;
  }, [showPill]);

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
    setWindowPicked(false);
  };

  const handleCancel = () => {
    openRef.current = null;
    leavingRef.current = true;
    setLeaving(true);
    setTimeout(() => void cancelCapture(), FADE_MS);
  };

  const handleMode = (next: CaptureMode) => requestMode(openRef.current, next);

  /** The toolbar's area/window switch, like Space. */
  const handleWindowMode = (on: boolean) => {
    if (on === windowModeRef.current) return;
    windowModeRef.current = on;
    setDragFrom(null);
    setWindowMode(on);
    void emit('capture://mode', on);
  };

  /** Capture grabs it now; record waits in the pill, and a new drag or click replaces it. */
  const select = (selection: Selection, rect: Rect) => {
    if (openRef.current?.mode === 'record') {
      setPending({ selection, rect });
      // Reaches this overlay too, which locks window picking here as well.
      void emit<PickedEvent>('capture://picked', { label: me.label, window: selection.kind === 'window' });
      return;
    }
    reset();
    void takeCapture(selection);
  };

  const point = (e: PointerEvent): Point => ({ x: e.clientX, y: e.clientY });

  const handlePointerDown = (e: PointerEvent) => {
    if (e.button !== 0 || !open) return;
    if (windowMode) {
      if (windowPicked) return;
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
    const hover = open && pointer && windowMode && !windowPicked ? windowAt(open.windows, pointer) : null;
    const drag = dragFrom && pointer ? normalizeRect(dragFrom, pointer) : null;
    // Only what shows of the window is tinted: the parts in front of it aren't taken.
    const covered = open && hover ? coveredParts(open.windows, hover) : [];
    return { ...overlayShapes(windowMode, hover, drag, pending?.rect ?? null), covered };
  }, [open, pointer, windowMode, windowPicked, dragFrom, pending]);

  if (!open) return null;
  const { width, height, scale } = open.monitor;
  const { hole, tint, outline, planned, covered } = shapes;
  const holePath = hole ? ` M${hole.x} ${hole.y}h${hole.width}v${hole.height}h${-hole.width}Z` : '';
  const labelBelow = hole && hole.y + hole.height + LABEL_GAP + 24 < height;
  const box = (rect: Rect) => ({ x: rect.x, y: rect.y, width: rect.width, height: rect.height });
  // Every tint is a window; the planned rect only when the pick is one.
  const corner = { rx: WINDOW_RADIUS, ry: WINDOW_RADIUS };
  const plannedCorner = pending?.selection.kind === 'window' ? corner : {};
  return (
    <div
      className={cx(
        // Fades in each time it opens (it renders nothing while closed, so this remounts).
        'animate-fade-in fixed inset-0 transition-opacity duration-150 select-none motion-reduce:animate-none motion-reduce:transition-none',
        leaving && 'pointer-events-none opacity-0',
        !windowMode
          ? 'cursor-crosshair'
          : windowPicked
            ? 'cursor-default'
            : open.mode === 'record'
              ? 'cursor-record'
              : 'cursor-camera',
      )}
      onPointerDown={handlePointerDown}
      onPointerMove={(e) => setPointer(point(e))}
      onPointerUp={handlePointerUp}
      // Off to another monitor: its overlay takes over (Rust moves the keyboard there), so drop the tint here.
      onPointerLeave={() => setPointer(null)}
    >
      <svg className="absolute inset-0 size-full" aria-hidden>
        {/* Like macOS: window mode leaves the screen as it is and only tints the window to be taken. */}
        {/* Always drawn so Space fades the dim in and out instead of cutting it. */}
        <path
          className={cx(
            'fill-black/35 transition-opacity duration-150 motion-reduce:transition-none',
            !shapes.dim && 'opacity-0',
          )}
          fillRule="evenodd"
          d={`M0 0H${width}V${height}H0Z${holePath}`}
        />
        {tint && (
          <>
            <mask id="tint-visible">
              <rect className="fill-white" {...box(tint)} {...corner} />
              {covered.map((part) => (
                <rect key={`${part.x},${part.y}`} className="fill-black" {...box(part)} />
              ))}
            </mask>
            <rect className="fill-pick" mask="url(#tint-visible)" {...box(tint)} {...corner} />
          </>
        )}
        {outline && <rect className="stroke-primary fill-none" strokeWidth={1} {...box(outline)} />}
        {/* The area the recording will cover, in the recording border's dashes. */}
        {planned && (
          <rect
            className="stroke-rec fill-none"
            strokeWidth={2}
            strokeDasharray="6 4"
            {...box(planned)}
            {...plannedCorner}
          />
        )}
      </svg>
      {/* A picked window's size isn't the user's to set: no label. */}
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
      {showPill && <RecordPill format={format} ffmpeg={ffmpeg} onFormat={setPicked} onStart={handleRecord} />}
      {showToolbar && (
        <CaptureToolbar
          mode={open.mode}
          windowMode={windowMode}
          recordBlocked={open.recordBlocked}
          onMode={handleMode}
          onWindowMode={handleWindowMode}
          onCancel={handleCancel}
        />
      )}
    </div>
  );
}
