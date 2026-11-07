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

/** The windows in front of `win` that overlap it, whole: drawn rounded, their corners leave `win` showing.
 * `windows` comes front to back. */
export function coveredParts(windows: WinInfo[], win: WinInfo): Rect[] {
  const parts: Rect[] = [];
  for (const w of windows) {
    if (w.id === win.id) break;
    const overlaps =
      w.x < win.x + win.width && win.x < w.x + w.width && w.y < win.y + win.height && win.y < w.y + w.height;
    if (overlaps) parts.push({ x: w.x, y: w.y, width: w.width, height: w.height });
  }
  return parts;
}

/** What the overlay draws; every rect is monitor-local. */
export interface OverlayShapes {
  /** Darken the screen around `hole` (region mode). */
  dim: boolean;
  hole: Rect | null;
  /** The window under the cursor, tinted (window mode). */
  tint: Rect | null;
  /** The thin outline of a drag in progress. */
  outline: Rect | null;
  /** A record selection waiting for Record: the dashed box of the area to be recorded. */
  planned: Rect | null;
}

/** The overlay's shapes from its mode, the window under the cursor, a drag in progress and a waiting record selection. */
export function overlayShapes(
  windowMode: boolean,
  hover: Rect | null,
  drag: Rect | null,
  pending: Rect | null,
): OverlayShapes {
  // A window picked for recording stands out like a dragged area: the rest dims.
  if (windowMode) return { dim: pending !== null, hole: pending, tint: hover, outline: null, planned: pending };
  return { dim: true, hole: drag ?? pending, tint: null, outline: drag, planned: drag ? null : pending };
}

/** An SVG subpath tracing `rect` with corners of radius `r` (capped at half the shorter side); 0 is square. */
export function rectPath({ x, y, width: w, height: h }: Rect, r: number): string {
  const c = Math.min(r, w / 2, h / 2);
  if (c <= 0) return `M${x} ${y}h${w}v${h}h${-w}Z`;
  const arc = (dx: number, dy: number) => `a${c} ${c} 0 0 1 ${dx} ${dy}`;
  return `M${x + c} ${y}h${w - 2 * c}${arc(c, c)}v${h - 2 * c}${arc(-c, c)}h${2 * c - w}${arc(-c, -c)}v${2 * c - h}${arc(c, -c)}Z`;
}
