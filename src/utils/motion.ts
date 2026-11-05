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
