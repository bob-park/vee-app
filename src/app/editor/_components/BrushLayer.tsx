import { useEffect, useRef } from 'react';

import type { BrushMode, Stroke } from '@/domain/edit/apis/edit.dto';
import { type Pt, type Size, brushRadius, thinPoints, toImagePx } from '@/utils/edit';

import cx from 'classnames';

/** The pointer in px from the element's top-left. */
const pointIn = (e: React.PointerEvent<HTMLElement>): Pt => {
  const r = e.currentTarget.getBoundingClientRect();
  return { x: e.clientX - r.left, y: e.clientY - r.top };
};

interface BrushLayerProps {
  /** Displayed size of the image, in CSS px. */
  view: Size;
  /** Full-resolution size the strokes are in. */
  image: Size;
  mode: BrushMode;
  /** Brush radius in CSS px; it stays the same on screen whatever the image size. */
  viewRadius: number;
  /** The image shows every stroke handed over; drawn strokes stay on the canvas until then. */
  synced: boolean;
  onStroke: (stroke: Stroke) => void;
  disabled: boolean;
}

/** Drag to paint a stroke: drawn live on a canvas, handed over as image px on release, and kept
 *  on the canvas until the render that includes it shows. */
export default function BrushLayer({ view, image, mode, viewRadius, synced, onStroke, disabled }: BrushLayerProps) {
  // ref
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const cursorRef = useRef<HTMLDivElement>(null);
  /** The drag's points in view px while a stroke is on. */
  const drag = useRef<Pt[] | null>(null);

  // useEffect
  useEffect(() => {
    if (synced) clear();
  }, [synced]);

  // handle
  const context = () => canvasRef.current?.getContext('2d') ?? null;

  const clear = () => {
    const c = canvasRef.current;
    c?.getContext('2d')?.clearRect(0, 0, c.width, c.height);
  };

  const moveCursor = (p: Pt | null) => {
    const el = cursorRef.current;
    if (!el) return;
    el.style.visibility = p ? 'visible' : 'hidden';
    if (p) [el.style.left, el.style.top] = [`${p.x}px`, `${p.y}px`];
  };

  const handlePointerDown = (e: React.PointerEvent<HTMLDivElement>) => {
    const ctx = context();
    if (e.button !== 0 || !ctx) return;
    e.currentTarget.setPointerCapture(e.pointerId);
    const p = pointIn(e);
    drag.current = [p];
    const dpr = window.devicePixelRatio || 1;
    ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
    // Opaque here; the canvas itself is drawn at 35 %, so overlapping segments don't darken.
    ctx.strokeStyle = getComputedStyle(e.currentTarget).getPropertyValue(
      mode === 'erase' ? '--color-primary' : '--color-sky',
    );
    ctx.lineWidth = viewRadius * 2;
    ctx.lineCap = 'round';
    ctx.lineJoin = 'round';
    ctx.beginPath();
    ctx.moveTo(p.x, p.y);
    ctx.lineTo(p.x, p.y);
    ctx.stroke();
  };

  const handlePointerMove = (e: React.PointerEvent<HTMLDivElement>) => {
    const p = pointIn(e);
    moveCursor(p);
    const pts = drag.current;
    const ctx = context();
    if (!pts || !ctx) return;
    const last = pts[pts.length - 1];
    pts.push(p);
    ctx.beginPath();
    ctx.moveTo(last.x, last.y);
    ctx.lineTo(p.x, p.y);
    ctx.stroke();
  };

  const handlePointerUp = () => {
    const pts = drag.current;
    drag.current = null;
    if (!pts) return;
    const radius = brushRadius(viewRadius, view, image);
    const imagePts = pts.map((p): [number, number] => {
      const q = toImagePx(p, view, image);
      return [q.x, q.y];
    });
    onStroke({ mode, radius, points: thinPoints(imagePts, radius / 4) });
  };

  const handlePointerCancel = () => {
    drag.current = null;
    clear();
  };

  const dpr = window.devicePixelRatio || 1;

  return (
    <div
      className={cx('absolute inset-0 cursor-none touch-none overflow-hidden', { 'pointer-events-none': disabled })}
      onPointerDown={handlePointerDown}
      onPointerMove={handlePointerMove}
      onPointerUp={handlePointerUp}
      onPointerCancel={handlePointerCancel}
      onPointerLeave={() => moveCursor(null)}
    >
      <canvas
        ref={canvasRef}
        width={Math.round(view.w * dpr)}
        height={Math.round(view.h * dpr)}
        className="size-full opacity-35"
      />
      {/* White ring plus a dark dashed one, so the size reads on any image. */}
      <div
        ref={cursorRef}
        className="pointer-events-none invisible absolute -translate-1/2 rounded-full border-[1.5px] border-white outline-1 outline-black/80 outline-dashed"
        style={{ width: viewRadius * 2, height: viewRadius * 2 }}
      />
    </div>
  );
}
