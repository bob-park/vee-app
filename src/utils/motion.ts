/** Pure geometry behind the panel's card motion; the DOM side lives in app/panel/_components/motion.ts. */

/** The floating card leans into horizontal motion and settles when it stops. */
export function nextTilt(prev: number, dx: number): number {
  return Math.max(-10, Math.min(10, prev * 0.7 + dx * 0.6));
}

type Box = { left: number; top: number; right: number; bottom: number };

/**
 * The `w`×`h` window is about to cut more of the floating card off than it did when it was
 * lifted at `start`, now that it has moved by (`dx`, `dy`): time to hand it to the OS drag.
 * Every edge counts; one the lifted card already overhung (a partly scrolled-in card) counts
 * only past that overhang. The card's tilt is left out on purpose: its rotated box grows
 * taller, so a sideways flick alone would cross an edge.
 */
export function ghostLeaves(start: Box, dx: number, dy: number, w: number, h: number): boolean {
  return (
    start.top + dy < Math.min(0, start.top) ||
    start.left + dx < Math.min(0, start.left) ||
    start.right + dx > Math.max(w, start.right) ||
    start.bottom + dy > Math.max(h, start.bottom)
  );
}

/**
 * The canvas for a `w`×`h` drag image grabbed at (`ox`, `oy`), padded so that point is its
 * centre, and where in it the card goes.
 */
export function cursorCentredFrame(w: number, h: number, ox: number, oy: number) {
  const halfW = Math.max(ox, w - ox);
  const halfH = Math.max(oy, h - oy);
  return { width: halfW * 2, height: halfH * 2, x: halfW - ox, y: halfH - oy };
}

/**
 * Pixel density and canvas for a `w`×`h` drag image grabbed at (`ox`, `oy`). Both platforms
 * hold the image by its centre (Windows through the patched drag crate in src-tauri/vendor), so
 * it is padded the same way. macOS sizes it from its 144 dpi, so it is drawn at 2x; Windows
 * draws it 1:1 in device pixels, so it is drawn at the screen's density.
 */
export function dragImageLayout(mac: boolean, w: number, h: number, ox: number, oy: number, dpr: number) {
  return { px: mac ? 2 : dpr, frame: cursorCentredFrame(w, h, ox, oy) };
}

type Rect = { x: number; y: number; width: number; height: number };

/** CSS `cubic-bezier(x1, y1, x2, y2)` as a function of time in [0, 1], for motion driven from JS. */
export function cubicBezier(x1: number, y1: number, x2: number, y2: number): (t: number) => number {
  // Each axis is 3(1-s)²s·p1 + 3(1-s)s²·p2 + s³; solve x(s) = t, then read y(s).
  const at = (s: number, p1: number, p2: number) =>
    ((1 - 3 * p2 + 3 * p1) * s + (3 * p2 - 6 * p1)) * s * s + 3 * p1 * s;
  const slope = (s: number, p1: number, p2: number) =>
    3 * (1 - 3 * p2 + 3 * p1) * s * s + 2 * (3 * p2 - 6 * p1) * s + 3 * p1;
  return (t) => {
    if (t <= 0) return 0;
    if (t >= 1) return 1;
    let s = t;
    for (let i = 0; i < 8; i++) {
      const d = slope(s, x1, x2);
      if (Math.abs(d) < 1e-6) break;
      s -= (at(s, x1, x2) - t) / d;
    }
    if (Math.abs(at(s, x1, x2) - t) > 1e-6 || s < 0 || s > 1) {
      let [lo, hi] = [0, 1];
      for (let i = 0; i < 20; i++) {
        s = (lo + hi) / 2;
        if (at(s, x1, x2) < t) lo = s;
        else hi = s;
      }
    }
    return at(s, y1, y2);
  };
}

/** `EASE_MOVE` (app/panel/_components/motion.ts): no overshoot, for things that glide to a new spot. */
export const easeMove = cubicBezier(0.32, 0.72, 0, 1);

/** `a` moved fraction `k` of the way to `b`. */
export function lerpRect(a: Rect, b: Rect, k: number): Rect {
  const mix = (p: number, q: number) => p + (q - p) * k;
  return { x: mix(a.x, b.x), y: mix(a.y, b.y), width: mix(a.width, b.width), height: mix(a.height, b.height) };
}

/**
 * A glide from `from` to `to`, now drawn at `drawn`, with the `covered` parts of its target left out. Hidden (`shown`
 * false) it keeps its last rect and parts, so it can fade out as it was.
 */
export type Glide = { from: Rect; to: Rect; drawn: Rect; shown: boolean; covered: Rect[] };

const sameRect = (a: Rect, b: Rect) => a.x === b.x && a.y === b.y && a.width === b.width && a.height === b.height;

/**
 * What a glide does when its target becomes `target`: hides on null, jumps when it was hidden (so it fades in
 * in place), and otherwise restarts from where it is drawn now, so a quick string of targets never snaps back.
 */
export function nextGlide(
  prev: Glide | null,
  target: Rect | null,
  covered: Rect[],
): (Glide & { restart: boolean }) | null {
  if (!target) return prev && { ...prev, shown: false, restart: false };
  if (!prev?.shown) return { from: target, to: target, drawn: target, shown: true, covered, restart: false };
  if (sameRect(prev.to, target)) return { ...prev, covered, restart: false };
  return { from: prev.drawn, to: target, drawn: prev.drawn, shown: true, covered, restart: true };
}
