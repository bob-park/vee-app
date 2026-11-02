import { type PointerEvent, useEffect, useMemo, useRef, useState } from 'react';

import { cancelCapture, takeCapture } from '@/domain/capture/apis/capture';
import type { CaptureOpen, Point, Rect, Selection } from '@/domain/capture/apis/capture.dto';
import { useI18n } from '@/shared/providers/i18n/I18nProvider';
import { normalizeRect, windowAt } from '@/utils/captureSelection';

import { listen } from '@tauri-apps/api/event';

/** Room between the selection and its size label. */
const LABEL_GAP = 6;

/** The selection overlay: drag a region, or Space to pick a window. Shared by capture and record. */
export default function CaptureApp() {
  // ref
  const openRef = useRef<CaptureOpen | null>(null);

  // state
  const [open, setOpen] = useState<CaptureOpen | null>(null);
  const [windowMode, setWindowMode] = useState(false);
  const [pointer, setPointer] = useState<Point | null>(null);
  const [dragFrom, setDragFrom] = useState<Point | null>(null);

  // hooks
  const { t } = useI18n();

  // useEffect
  useEffect(() => {
    const off = listen<CaptureOpen>('capture://open', (e) => {
      openRef.current = e.payload;
      setOpen(e.payload);
      setWindowMode(false);
      setPointer(null);
      setDragFrom(null);
    });
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
      // Record mode: Enter starts the recording from the pill (Task 8).
    };
    window.addEventListener('keydown', onKey);
    return () => window.removeEventListener('keydown', onKey);
  }, []);

  // handle
  const reset = () => {
    openRef.current = null;
    setOpen(null);
    setDragFrom(null);
    setPointer(null);
  };

  const select = (selection: Selection) => {
    if (openRef.current?.mode !== 'capture') return; // Record mode: the pill takes over here (Task 8).
    reset();
    void takeCapture(selection);
  };

  const point = (e: PointerEvent): Point => ({ x: e.clientX, y: e.clientY });

  const handlePointerDown = (e: PointerEvent) => {
    if (e.button !== 0 || !open) return;
    if (windowMode) {
      const win = windowAt(open.windows, point(e));
      if (win) select({ kind: 'window', id: win.id });
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
    if (rect) select({ kind: 'rect', ...rect });
  };

  // memorize
  const hole = useMemo<Rect | null>(() => {
    if (!open || !pointer) return null;
    if (windowMode) return windowAt(open.windows, pointer);
    return dragFrom ? normalizeRect(dragFrom, pointer) : null;
  }, [open, pointer, windowMode, dragFrom]);

  if (!open) return null;
  const { width, height, scale } = open.monitor;
  const holePath = hole ? ` M${hole.x} ${hole.y}h${hole.width}v${hole.height}h${-hole.width}Z` : '';
  const labelBelow = hole && hole.y + hole.height + LABEL_GAP + 24 < height;
  return (
    <div
      className="fixed inset-0 cursor-crosshair select-none"
      onPointerDown={handlePointerDown}
      onPointerMove={(e) => setPointer(point(e))}
      onPointerUp={handlePointerUp}
    >
      <svg className="absolute inset-0 size-full" aria-hidden>
        <path className="fill-black/35" fillRule="evenodd" d={`M0 0H${width}V${height}H0Z${holePath}`} />
        {hole && (
          <rect
            className="stroke-primary fill-none"
            strokeWidth={windowMode ? 3 : 1}
            x={hole.x}
            y={hole.y}
            width={hole.width}
            height={hole.height}
          />
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
      {!dragFrom && (
        <div className="glass text-base-content shadow-glass absolute bottom-10 left-1/2 -translate-x-1/2 rounded-full px-4 py-2 text-[13px]">
          {windowMode ? t.capture.windowHint : t.capture.regionHint}
        </div>
      )}
    </div>
  );
}
