import { useRef, useState } from 'react';

import MiniMap from '@/app/editor/_components/MiniMap';
import { useI18n } from '@/shared/providers/i18n/I18nProvider';
import { type Pt, type Size, clampPan, previewScale } from '@/utils/edit';

import cx from 'classnames';

interface CompareViewProps {
  before: string;
  after: string;
  /** Full-resolution size of `after`; `before` is stretched to match. */
  size: Size;
  /** `1` shows one CSS px per preview px (full-res up to 2048 px) and pans by dragging. */
  zoom: 'fit' | 1;
  /** Room for the view, in CSS px. */
  stage: Size;
}

const TAG =
  'glass text-base-content pointer-events-none absolute top-2 z-3 rounded-md px-1.5 py-0.5 text-[11px] font-semibold';

/** The image's displayed size at `zoom` and the part of it the stage shows. */
function layout(size: Size, zoom: CompareViewProps['zoom'], stage: Size) {
  // Never above the preview's own px, so nothing is stretched.
  const actual = previewScale(size);
  const k = zoom === 'fit' ? Math.min(stage.w / size.w, stage.h / size.h, actual) : actual;
  const zoomed = { w: size.w * k, h: size.h * k };
  return { zoomed, view: { w: Math.min(zoomed.w, stage.w), h: Math.min(zoomed.h, stage.h) } };
}

/**
 * daisyUI `diff`: the original on the left, the result (over a checkerboard) on the right, split by a
 * draggable bar. Both share one transform, so they pan and zoom together.
 */
export default function CompareView({ before, after, size, zoom, stage }: CompareViewProps) {
  // ref
  /** The pointer and pan a pan drag started from. */
  const drag = useRef<{ p: Pt; pan: Pt } | null>(null);

  // state
  // Starts centered; the parent remounts this on a zoom change.
  const [pan, setPan] = useState<Pt>(() => {
    const { zoomed, view } = layout(size, zoom, stage);
    return { x: (zoomed.w - view.w) / 2, y: (zoomed.h - view.h) / 2 };
  });

  // hooks
  const { t } = useI18n();

  // handle
  const handlePointerDown = (e: React.PointerEvent<HTMLElement>) => {
    // The bar (`diff-resizer`) drags itself.
    if (zoom === 'fit' || e.button !== 0 || (e.target as HTMLElement).classList.contains('diff-resizer')) return;
    drag.current = { p: { x: e.clientX, y: e.clientY }, pan: at };
    e.currentTarget.setPointerCapture(e.pointerId);
  };

  const handlePointerMove = (e: React.PointerEvent<HTMLElement>) => {
    const d = drag.current;
    if (!d) return;
    setPan(clampPan({ x: d.pan.x + d.p.x - e.clientX, y: d.pan.y + d.p.y - e.clientY }, zoomed, view));
  };

  const handlePointerUp = () => {
    drag.current = null;
  };

  const { zoomed, view } = layout(size, zoom, stage);
  // A smaller stage (window resize) may have pushed the old pan past an edge.
  const at = clampPan(pan, zoomed, view);
  const img = (src: string) => (
    <div>
      <img
        src={src}
        alt=""
        draggable={false}
        className="absolute top-0 left-0 max-w-none select-none"
        style={{ width: zoomed.w, height: zoomed.h, transform: `translate(${-at.x}px, ${-at.y}px)` }}
      />
    </div>
  );

  return (
    <div className="relative" style={{ width: view.w, height: view.h }}>
      <figure
        tabIndex={0}
        className={cx('diff rounded-md', { 'cursor-grab active:cursor-grabbing': zoom === 1 })}
        style={{ width: view.w, height: view.h }}
        onPointerDown={handlePointerDown}
        onPointerMove={handlePointerMove}
        onPointerUp={handlePointerUp}
        onPointerCancel={handlePointerUp}
      >
        <div className="diff-item-1" role="img" tabIndex={0} aria-label={t.editor.original}>
          {img(before)}
        </div>
        <div className="diff-item-2 [&>div]:bg-checker" role="img" aria-label={t.editor.result}>
          {img(after)}
        </div>
        <div className="diff-resizer" />
      </figure>
      <span className={cx(TAG, 'left-2')}>{t.editor.original}</span>
      <span className={cx(TAG, 'right-2')}>{t.editor.result}</span>
      {zoom === 1 && <MiniMap src={after} zoomed={zoomed} view={view} pan={at} />}
    </div>
  );
}
