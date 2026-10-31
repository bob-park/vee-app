import { useRef } from 'react';

import { type Pt, type Rect, type Size, cropFromDrag } from '@/utils/edit';

import cx from 'classnames';

import { pointIn } from './pointer';

/** Handle sides: -1 moves the left/top edge, 1 the right/bottom, 0 leaves that axis alone. */
const HANDLES: { hx: -1 | 0 | 1; hy: -1 | 0 | 1; cursor: string }[] = [
  { hx: -1, hy: -1, cursor: 'nwse-resize' },
  { hx: 0, hy: -1, cursor: 'ns-resize' },
  { hx: 1, hy: -1, cursor: 'nesw-resize' },
  { hx: 1, hy: 0, cursor: 'ew-resize' },
  { hx: 1, hy: 1, cursor: 'nwse-resize' },
  { hx: 0, hy: 1, cursor: 'ns-resize' },
  { hx: -1, hy: 1, cursor: 'nesw-resize' },
  { hx: -1, hy: 0, cursor: 'ew-resize' },
];

interface CropBoxProps {
  /** Ignores the pointer while the image under it is re-rendering. */
  disabled?: boolean;
  /** Displayed size of the image, in CSS px. */
  view: Size;
  /** Full-resolution size the rect is in. */
  image: Size;
  rect: Rect | null;
  onChange: (rect: Rect | null) => void;
}

/** Drag over the image to draw a crop box; its 8 handles move its edges. Outside the box is dimmed. */
export default function CropBox({ disabled, view, image, rect, onChange }: CropBoxProps) {
  // ref
  /** Maps the pointer to the drag's two corners while a drag is on. */
  const drag = useRef<((p: Pt) => [Pt, Pt]) | null>(null);

  // handle
  const handlePointerDown = (e: React.PointerEvent<HTMLDivElement>) => {
    if (e.button !== 0) return;
    const p0 = pointIn(e);
    const handle = (e.target as HTMLElement).dataset.handle;
    if (handle && box) {
      const [hx, hy] = handle.split(',').map(Number);
      // The opposite corner (or edge) stays put.
      const start = { x: hx < 0 ? box.x + box.w : box.x, y: hy < 0 ? box.y + box.h : box.y };
      drag.current = (p) => [start, { x: hx ? p.x : box.x + box.w, y: hy ? p.y : box.y + box.h }];
    } else {
      drag.current = (p) => [p0, p];
    }
    e.currentTarget.setPointerCapture(e.pointerId);
  };

  const handlePointerMove = (e: React.PointerEvent<HTMLDivElement>) => {
    if (!drag.current) return;
    const [start, end] = drag.current(pointIn(e));
    const next = cropFromDrag(start, end, view, image);
    onChange(next.w > 0 && next.h > 0 ? next : null);
  };

  const handlePointerUp = () => {
    drag.current = null;
  };

  const scale = view.w / image.w;
  const box = rect && { x: rect.x * scale, y: rect.y * scale, w: rect.w * scale, h: rect.h * scale };

  return (
    <div
      className={cx('absolute inset-0 cursor-crosshair touch-none overflow-hidden', {
        'pointer-events-none': disabled,
      })}
      onPointerDown={handlePointerDown}
      onPointerMove={handlePointerMove}
      onPointerUp={handlePointerUp}
      onPointerCancel={handlePointerUp}
    >
      {box && (
        <div
          className="absolute border-[1.5px] border-white shadow-[0_0_0_9999px_var(--color-backdrop)]"
          style={{ left: box.x, top: box.y, width: box.w, height: box.h }}
        >
          {HANDLES.map(({ hx, hy, cursor }) => (
            <i
              key={`${hx},${hy}`}
              data-handle={`${hx},${hy}`}
              className="absolute size-2.5 -translate-1/2 rounded-[3px] bg-white"
              style={{ left: `${(hx + 1) * 50}%`, top: `${(hy + 1) * 50}%`, cursor }}
            />
          ))}
        </div>
      )}
    </div>
  );
}
