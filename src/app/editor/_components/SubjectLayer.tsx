import type { SubjectPoint } from '@/domain/edit/apis/edit.dto';
import { type Size, toImagePx } from '@/utils/edit';

import cx from 'classnames';

interface SubjectLayerProps {
  /** Displayed size of the image, in CSS px. */
  view: Size;
  /** Full-resolution size the points are in. */
  image: Size;
  points: SubjectPoint[];
  onAdd: (point: SubjectPoint) => void;
  disabled: boolean;
}

/** Click to mark what to keep, ⌥/Alt-click to mark background; each point shows as a `+`/`−` dot. */
export default function SubjectLayer({ view, image, points, onAdd, disabled }: SubjectLayerProps) {
  // handle
  const handleClick = (e: React.MouseEvent<HTMLDivElement>) => {
    const r = e.currentTarget.getBoundingClientRect();
    onAdd({ ...toImagePx({ x: e.clientX - r.left, y: e.clientY - r.top }, view, image), keep: !e.altKey });
  };

  const scale = view.w / image.w;

  return (
    <div className={cx('absolute inset-0 cursor-crosshair', { 'pointer-events-none': disabled })} onClick={handleClick}>
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
