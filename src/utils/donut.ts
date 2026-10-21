/** Geometry of the convert window's format donut; the window is `size` CSS px square. */
export const DONUT = { size: 260, disc: 106, outer: 98, inner: 54, corner: 10, gap: 10 } as const;

const C = DONUT.size / 2;
const TAU = 2 * Math.PI;

/** A donut is never cut finer than this, so a short list keeps slices of the same size. */
const MIN_SLOTS = 6;

/**
 * Slice `i` of `count` as angles clockwise from 12 o'clock. Six or more formats share the whole ring
 * from 12 o'clock; fewer keep 60° slices and sit centred on 12 o'clock, leaving the rest empty.
 */
function span(i: number, count: number): [number, number] {
  const w = TAU / Math.max(count, MIN_SLOTS);
  const start = count < MIN_SLOTS ? (-w * count) / 2 : 0;
  return [start + i * w, start + (i + 1) * w];
}

/**
 * The slice under (`x`, `y`), CSS px from the window's top-left (see `span` for the layout);
 * the hole, the outside, the empty rest of a short donut and an empty donut hit nothing.
 */
export function sliceAt(x: number, y: number, count: number): number | null {
  const dx = x - C;
  const dy = y - C;
  const d = Math.hypot(dx, dy);
  if (count === 0 || d < DONUT.inner || d > DONUT.outer) return null;
  // Angle clockwise from the first slice's start, in [0, TAU).
  const a = (Math.atan2(dx, -dy) - span(0, count)[0] + 2 * TAU) % TAU;
  const i = Math.floor(a / (TAU / Math.max(count, MIN_SLOTS)));
  return i < count ? i : null;
}

/** Point on a circle of radius `r` at angle `a` (0 = 3 o'clock), rounded for a short path. */
function at(r: number, a: number) {
  return `${+(r * Math.cos(a)).toFixed(2)} ${+(r * Math.sin(a)).toFixed(2)}`;
}

/**
 * The ring sector between `a0` and `a1` (clockwise from 12 o'clock) as an SVG path in a `-130..130`
 * viewBox. Its radial edges sit `gap / 2` px off the boundary, so the space between two sectors is
 * `gap` px wide at every radius; its four corners are rounded by `DONUT.corner`.
 */
function sectorPath(a0: number, a1: number, gap: number): string {
  const { outer: R, inner: r, corner: k } = DONUT;
  const b0 = a0 - Math.PI / 2;
  const b1 = a1 - Math.PI / 2;
  // Angular trim that puts a point at radius `rho` gap/2 px (perpendicular) off the boundary ray.
  const trim = (rho: number) => Math.asin(Math.min(1, gap / 2 / rho));
  // Each corner: a quadratic curve from the radial edge (k in from the ring) to the arc (k along it).
  const [o, ii] = [k / R + trim(R), k / r + trim(r)];
  const large = (inset: number) => (b1 - b0 - 2 * inset > Math.PI ? 1 : 0);
  return [
    `M${at(R - k, b0 + trim(R - k))}`,
    `Q${at(R, b0 + trim(R))} ${at(R, b0 + o)}`,
    `A${R} ${R} 0 ${large(o)} 1 ${at(R, b1 - o)}`,
    `Q${at(R, b1 - trim(R))} ${at(R - k, b1 - trim(R - k))}`,
    `L${at(r + k, b1 - trim(r + k))}`,
    `Q${at(r, b1 - trim(r))} ${at(r, b1 - ii)}`,
    `A${r} ${r} 0 ${large(ii)} 0 ${at(r, b0 + ii)}`,
    `Q${at(r, b0 + trim(r))} ${at(r + k, b0 + trim(r + k))}Z`,
  ].join('');
}

/** Slice `i` of `count` as an SVG path (see `span` for the layout and `sectorPath` for the shape). */
export function slicePath(i: number, count: number, gap: number = DONUT.gap): string {
  const [a0, a1] = span(i, count);
  return sectorPath(a0, a1, gap);
}

/** The empty rest of a donut with fewer than six slices as one path, or null when the ring is full. */
export function restPath(count: number, gap: number = DONUT.gap): string | null {
  if (count === 0 || count >= MIN_SLOTS) return null;
  return sectorPath(span(count - 1, count)[1], span(0, count)[0] + TAU, gap);
}

/** Where slice `i`'s label sits: mid-angle, halfway through the ring. */
export function labelAt(i: number, count: number) {
  const [a0, a1] = span(i, count);
  const a = (a0 + a1) / 2 - Math.PI / 2;
  const m = (DONUT.outer + DONUT.inner) / 2;
  return { x: +(m * Math.cos(a)).toFixed(2), y: +(m * Math.sin(a)).toFixed(2) };
}

/**
 * A drag-drop `position` in CSS px. Tauri labels it physical, but on macOS wry passes the
 * webview's own point coordinates; only Windows sends device pixels.
 */
export function dropPoint(p: { x: number; y: number }, dpr: number, points: boolean) {
  return points ? { x: p.x, y: p.y } : { x: p.x / dpr, y: p.y / dpr };
}
