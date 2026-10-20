/** Geometry of the convert window's format donut; the window is `size` CSS px square. */
export const DONUT = { size: 260, outer: 112, inner: 44 } as const;

const C = DONUT.size / 2;
const TAU = 2 * Math.PI;

/**
 * The slice under (`x`, `y`), CSS px from the window's top-left. Slices run clockwise from
 * 12 o'clock at equal angles; the hole, the outside and an empty donut hit nothing.
 */
export function sliceAt(x: number, y: number, count: number): number | null {
  const dx = x - C;
  const dy = y - C;
  const d = Math.hypot(dx, dy);
  if (count === 0 || d < DONUT.inner || d > DONUT.outer) return null;
  // Angle clockwise from straight up, in [0, TAU).
  const a = (Math.atan2(dx, -dy) + TAU) % TAU;
  return Math.min(count - 1, Math.floor((a / TAU) * count));
}

/** Point on a circle of radius `r` at angle `a` (0 = 3 o'clock), rounded for a short path. */
function at(r: number, a: number) {
  return `${+(r * Math.cos(a)).toFixed(2)} ${+(r * Math.sin(a)).toFixed(2)}`;
}

/** Slice `i` of `count` as an SVG path in a `-130..130` viewBox, trimmed by `gap` radians each side. */
export function slicePath(i: number, count: number, gap = 0.03): string {
  const { outer: R, inner: r } = DONUT;
  const a0 = -Math.PI / 2 + (i * TAU) / count + gap;
  const a1 = a0 + TAU / count - 2 * gap;
  const large = a1 - a0 > Math.PI ? 1 : 0;
  return `M${at(R, a0)}A${R} ${R} 0 ${large} 1 ${at(R, a1)}L${at(r, a1)}A${r} ${r} 0 ${large} 0 ${at(r, a0)}Z`;
}

/** Where slice `i`'s label sits: mid-angle, halfway through the ring. */
export function labelAt(i: number, count: number) {
  const a = -Math.PI / 2 + ((i + 0.5) * TAU) / count;
  const m = (DONUT.outer + DONUT.inner) / 2;
  return { x: +(m * Math.cos(a)).toFixed(2), y: +(m * Math.sin(a)).toFixed(2) };
}
