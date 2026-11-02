import type { Point, Rect, WinInfo } from '@/domain/capture/apis/capture.dto';

/** Smallest drag that counts as a selection, per side. */
const MIN_SIDE = 4;

/** The rect spanned by a drag from `a` to `b`, or null when it is too thin to be meant. */
export function normalizeRect(a: Point, b: Point): Rect | null {
  const width = Math.abs(a.x - b.x);
  const height = Math.abs(a.y - b.y);
  if (width < MIN_SIDE || height < MIN_SIDE) return null;
  return { x: Math.min(a.x, b.x), y: Math.min(a.y, b.y), width, height };
}

/** The frontmost window under `p`; `windows` comes front to back. */
export function windowAt(windows: WinInfo[], p: Point): WinInfo | null {
  return windows.find((w) => p.x >= w.x && p.x < w.x + w.width && p.y >= w.y && p.y < w.y + w.height) ?? null;
}
