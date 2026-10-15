/** Pure geometry behind the panel's card motion; the DOM side lives in app/panel/_components/motion.ts. */

/** The floating card leans into horizontal motion and settles when it stops. */
export function nextTilt(prev: number, dx: number): number {
  return Math.max(-10, Math.min(10, prev * 0.7 + dx * 0.6));
}

type Box = { left: number; top: number; right: number; bottom: number };

/**
 * The window is about to cut more of the floating card off than it did when the card was
 * lifted (`start`): time to hand it to the OS drag. The bottom never counts — the panel sits
 * on the screen's bottom edge, so a lifted card already overhangs it and nothing is below.
 */
export function ghostLeaves(box: Box, start: Box, w: number): boolean {
  return box.top < Math.min(0, start.top) || box.left < Math.min(0, start.left) || box.right > Math.max(w, start.right);
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
