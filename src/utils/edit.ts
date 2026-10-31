import type { EditOp } from '@/domain/edit/apis/edit.dto';

export interface Pt {
  x: number;
  y: number;
}

export interface Size {
  w: number;
  h: number;
}

export type Rect = Pt & Size;

/** Largest side a resize may ask for; mirrors Rust `MAX_SIDE`. */
export const MAX_SIDE = 16384;
/** Largest long side of a rendered preview; mirrors Rust `PREVIEW_SIDE`. */
const PREVIEW_SIDE = 2048;
/** Largest long side an upscale produces; mirrors Rust `UPSCALE_CAP`. */
const UPSCALE_CAP = 4096;

const clamp = (v: number, max: number) => Math.min(Math.max(v, 0), max);

/** The box between two drag points in `view` px, as image px: normalised, clamped to the image, rounded. */
export function cropFromDrag(start: Pt, end: Pt, view: Size, image: Size): Rect {
  const sx = image.w / view.w;
  const sy = image.h / view.h;
  const x0 = Math.round(clamp(Math.min(start.x, end.x), view.w) * sx);
  const x1 = Math.round(clamp(Math.max(start.x, end.x), view.w) * sx);
  const y0 = Math.round(clamp(Math.min(start.y, end.y), view.h) * sy);
  const y1 = Math.round(clamp(Math.max(start.y, end.y), view.h) * sy);
  return { x: x0, y: y0, w: x1 - x0, h: y1 - y0 };
}

/** A point in `view` px as image px: rounded, clamped to `[0, image-1]`. */
export function toImagePx(p: Pt, view: Size, image: Size): Pt {
  return {
    x: clamp(Math.round((p.x * image.w) / view.w), image.w - 1),
    y: clamp(Math.round((p.y * image.h) / view.h), image.h - 1),
  };
}

/** A brush radius in `view` px as image px, at least 1. */
export function brushRadius(viewRadius: number, view: Size, image: Size): number {
  return Math.max(1, Math.round((viewRadius * image.w) / view.w));
}

/** `points` without those closer than `minGap` to the last kept one; the first and last always stay. */
export function thinPoints(points: [number, number][], minGap: number): [number, number][] {
  const kept: [number, number][] = [];
  points.forEach((p, i) => {
    const last = kept.at(-1);
    if (!last || i === points.length - 1 || Math.hypot(p[0] - last[0], p[1] - last[1]) >= minGap) kept.push(p);
  });
  return kept;
}

/** `next` filled in from `prev`: the missing side follows `prev`'s ratio when `locked`. Rounded, at least 1. */
export function resizeKeep(prev: Size, next: Partial<Size>, locked: boolean): Size {
  let { w = prev.w, h = prev.h } = next;
  if (locked && next.w !== undefined && next.h === undefined) h = (w * prev.h) / prev.w;
  if (locked && next.h !== undefined && next.w === undefined) w = (h * prev.w) / prev.h;
  return { w: Math.max(1, Math.round(w)), h: Math.max(1, Math.round(h)) };
}

/** `w*scale × h*scale` with the long side capped, the short side floored; mirrors Rust `upscale_size`. */
function upscaleSize({ w, h }: Size, scale: number): Size {
  [w, h] = [w * scale, h * scale];
  if (w >= h && w > UPSCALE_CAP) [w, h] = [UPSCALE_CAP, Math.floor((h * UPSCALE_CAP) / w)];
  else if (h > w && h > UPSCALE_CAP) [w, h] = [Math.floor((w * UPSCALE_CAP) / h), UPSCALE_CAP];
  return { w: Math.max(1, w), h: Math.max(1, h) };
}

/** The size `ops` turn `base` into; mirrors Rust's `render`. */
export function sizeAfter(base: Size, ops: EditOp[]): Size {
  return ops.reduce<Size>((s, op) => {
    switch (op.type) {
      case 'crop': {
        const x = Math.min(op.x, s.w);
        const y = Math.min(op.y, s.h);
        return { w: Math.min(op.w, s.w - x), h: Math.min(op.h, s.h - y) };
      }
      case 'rotate':
        return { w: s.h, h: s.w };
      case 'resize':
        return { w: op.w, h: op.h };
      case 'upscale':
        return upscaleSize(s, op.scale);
      case 'removeBg':
      case 'subject':
      case 'brush':
        return s;
    }
  }, base);
}

/** Preview px per full-res px for an image of `size`: below 1 once its long side passes `PREVIEW_SIDE`. */
export function previewScale(size: Size): number {
  return Math.min(1, PREVIEW_SIDE / Math.max(size.w, size.h));
}

/** `pan` (the view's top-left in `zoomed` px) kept so the view never leaves the image; `0` on an axis that fits. */
export function clampPan(pan: Pt, zoomed: Size, view: Size): Pt {
  return { x: clamp(pan.x, Math.max(0, zoomed.w - view.w)), y: clamp(pan.y, Math.max(0, zoomed.h - view.h)) };
}

/** The part of `zoomed` that `view` shows at `pan`, scaled onto a `map` of the whole image. */
export function viewportRect(pan: Pt, zoomed: Size, view: Size, map: Size): Rect {
  const sx = map.w / zoomed.w;
  const sy = map.h / zoomed.h;
  return {
    x: pan.x * sx,
    y: pan.y * sy,
    w: Math.min(view.w, zoomed.w) * sx,
    h: Math.min(view.h, zoomed.h) * sy,
  };
}
