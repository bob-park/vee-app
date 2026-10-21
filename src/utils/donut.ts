/** Geometry of the convert window's format donut; the window is `size` CSS px square. */
export const DONUT = { size: 260, disc: 106, outer: 98, inner: 54, corner: 10, gap: 10 } as const;

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

/**
 * Slice `i` of `count` as an SVG path in a `-130..130` viewBox: a ring sector whose radial edges sit
 * `gap / 2` px off the boundary, so the space between two slices is `gap` px wide at every radius.
 * Its four corners are rounded by `DONUT.corner`.
 */
export function slicePath(i: number, count: number, gap: number = DONUT.gap): string {
  const { outer: R, inner: r, corner: k } = DONUT;
  const b0 = -Math.PI / 2 + (i * TAU) / count;
  const b1 = -Math.PI / 2 + ((i + 1) * TAU) / count;
  // Angular trim that puts a point at radius `rho` gap/2 px (perpendicular) off the boundary ray.
  const trim = (rho: number) => Math.asin(Math.min(1, gap / 2 / rho));
  // Each corner: a quadratic curve from the radial edge (k in from the ring) to the arc (k along it).
  const [o, ii] = [k / R + trim(R), k / r + trim(r)];
  const large = (span: number) => (b1 - b0 - 2 * span > Math.PI ? 1 : 0);
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

/** Where slice `i`'s label sits: mid-angle, halfway through the ring. */
export function labelAt(i: number, count: number) {
  const a = -Math.PI / 2 + ((i + 0.5) * TAU) / count;
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
