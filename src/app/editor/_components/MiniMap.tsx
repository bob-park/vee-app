import { type Pt, type Size, viewportRect } from '@/utils/edit';

/** Longest side of the map, in CSS px. */
const SIDE = 72;

interface MiniMapProps {
  src: string;
  zoomed: Size;
  view: Size;
  pan: Pt;
}

/** The whole image in miniature with the part on screen outlined. */
export default function MiniMap({ src, zoomed, view, pan }: MiniMapProps) {
  const k = SIDE / Math.max(zoomed.w, zoomed.h);
  const map = { w: zoomed.w * k, h: zoomed.h * k };
  const rect = viewportRect(pan, zoomed, view, map);

  return (
    <div className="glass pointer-events-none absolute right-2 bottom-2 z-3 rounded-lg p-0.5" aria-hidden>
      <div className="relative" style={{ width: map.w, height: map.h }}>
        <img src={src} alt="" draggable={false} className="size-full rounded-md opacity-80" />
        <div
          className="border-primary absolute rounded-xs border-[1.5px]"
          style={{ left: rect.x, top: rect.y, width: rect.w, height: rect.h }}
        />
      </div>
    </div>
  );
}
