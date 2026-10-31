import { useState } from 'react';

import type { SubjectPoint } from '@/domain/edit/apis/edit.dto';
import { type Pt, type Size, capPoints, classifyGesture, thinPoints, toImagePx } from '@/utils/edit';

import cx from 'classnames';

/** Most lasso points sent to Rust; the AI worker's frame header is capped at 64 KiB. */
const LASSO_MAX = 1000;

/** The pointer in px from the element's top-left. */
const pointIn = (e: React.PointerEvent<HTMLElement>): Pt => {
  const r = e.currentTarget.getBoundingClientRect();
  return { x: e.clientX - r.left, y: e.clientY - r.top };
};

interface SubjectLayerProps {
  /** Displayed size of the image, in CSS px. */
  view: Size;
  /** Full-resolution size the points and the lasso are in. */
  image: Size;
  points: SubjectPoint[];
  /** The lasso the cut uses, if any. */
  lasso: [number, number][] | undefined;
  onPoint: (point: SubjectPoint) => void;
  onLasso: (lasso: [number, number][]) => void;
  disabled: boolean;
}

/** Click to mark what to keep, ⌥/Alt-click to mark background, or drag around the subject to
 *  lasso it. Points show as `+`/`−` dots, the lasso as a dashed outline. */
export default function SubjectLayer({ view, image, points, lasso, onPoint, onLasso, disabled }: SubjectLayerProps) {
  // state
  /** The press's path in view px while the pointer is down. */
  const [path, setPath] = useState<Pt[] | null>(null);

  // handle
  const handlePointerDown = (e: React.PointerEvent<HTMLDivElement>) => {
    if (e.button !== 0) return;
    e.currentTarget.setPointerCapture(e.pointerId);
    setPath([pointIn(e)]);
  };

  const handlePointerMove = (e: React.PointerEvent<HTMLDivElement>) => {
    if (!path) return;
    const p = pointIn(e);
    setPath((cur) => cur && [...cur, p]);
  };

  const handlePointerUp = (e: React.PointerEvent<HTMLDivElement>) => {
    if (!path) return;
    setPath(null);
    const full = [...path, pointIn(e)];
    if (classifyGesture(full) === 'click') {
      onPoint({ ...toImagePx(full[0], view, image), keep: !e.altKey });
      return;
    }
    const gap = Math.max(1, Math.round(Math.max(image.w, image.h) / 400));
    const imagePts = full.map((p): [number, number] => {
      const q = toImagePx(p, view, image);
      return [q.x, q.y];
    });
    const out = capPoints(thinPoints(imagePts, gap), LASSO_MAX);
    // A quick flick can leave only its two ends: no outline to cut by.
    if (out.length >= 3) onLasso(out);
  };

  const scale = view.w / image.w;
  const drawing = path && classifyGesture(path) === 'lasso';

  return (
    <div
      className={cx('absolute inset-0 cursor-crosshair touch-none', { 'pointer-events-none': disabled })}
      onPointerDown={handlePointerDown}
      onPointerMove={handlePointerMove}
      onPointerUp={handlePointerUp}
      // Also after a cancel, or when a render disables the layer mid-drag.
      onLostPointerCapture={() => setPath(null)}
    >
      <svg className="pointer-events-none absolute inset-0 size-full overflow-visible" aria-hidden>
        {lasso && !drawing && (
          <polygon
            points={lasso.map(([x, y]) => `${x * scale},${y * scale}`).join(' ')}
            fill="var(--color-primary)"
            fillOpacity={0.1}
            stroke="var(--color-primary)"
            strokeWidth={2.5}
            strokeDasharray="6 4"
            strokeLinejoin="round"
          />
        )}
        {drawing && (
          <polyline
            points={path.map((p) => `${p.x},${p.y}`).join(' ')}
            fill="none"
            stroke="var(--color-primary)"
            strokeWidth={2.5}
            strokeDasharray="6 4"
            strokeLinecap="round"
            strokeLinejoin="round"
          />
        )}
      </svg>
      {points.map((p, i) => (
        <span
          key={i}
          className={cx(
            'pointer-events-none absolute flex size-4 -translate-1/2 items-center justify-center rounded-full text-[12px] leading-none font-bold ring-[1.5px] ring-white',
            p.keep ? 'bg-success text-success-content' : 'bg-error text-error-content',
          )}
          style={{ left: p.x * scale, top: p.y * scale }}
          aria-hidden
        >
          {p.keep ? '+' : '−'}
        </span>
      ))}
    </div>
  );
}
