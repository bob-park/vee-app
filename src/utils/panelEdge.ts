import type { PanelPosition } from '@/domain/settings/apis/settings.dto';

/** Left and right: sized by width, cards stacked top to bottom. */
export function isSide(p: PanelPosition): boolean {
  return p === 'left' || p === 'right';
}

/** Where a resize drag started: the panel size and the pointer, in CSS px (screen coordinates). */
export interface DragStart {
  size: number;
  x: number;
  y: number;
}

/** Panel size for a drag on its inner edge: moving that edge away from the docked screen edge grows it. */
export function dragSize(p: PanelPosition, start: DragStart, x: number, y: number): number {
  switch (p) {
    case 'bottom':
      return start.size + (start.y - y);
    case 'top':
      return start.size + (y - start.y);
    case 'left':
      return start.size + (x - start.x);
    case 'right':
      return start.size + (start.x - x);
  }
}

/** Mirrors `app/window.rs`: default side width share, its clamp, the 340pt floor and the 8pt margin. */
const SIDE_WIDTH = { ratio: 0.25, min: 0.15, max: 0.5, floor: 340 };
const MARGIN = 8;
const MAX_PINNED = 3;
// ponytail: fixed layout heights measured off the side panel (padding, two-line toolbar, gaps, divider);
// a toolbar restyle that changes its height needs these updated.
const CHROME = 116; // panel padding 22 + toolbar 80 + history list padding 14
const PIN_BLOCK = 23; // pinned group padding 10 + divider 13
const GAP = 10;
const CARD_INSET = 32; // panel px-3 + list p-1, both sides

/**
 * How many pinned cards a left/right panel shows with at least one whole history card below them,
 * on a screen whose work area is `w`×`h` points. `ratio` is the saved width share (null: default).
 */
export function sidePinsThatFit(w: number, h: number, ratio: number | null): number {
  const max = Math.floor(w * SIDE_WIDTH.max);
  const min = Math.min(Math.max(Math.ceil(w * SIDE_WIDTH.min), SIDE_WIDTH.floor), max);
  const card = Math.min(Math.max(Math.round(w * (ratio ?? SIDE_WIDTH.ratio)), min), max) - CARD_INSET;
  const room = h - 2 * MARGIN - CHROME - card;
  for (let k = MAX_PINNED; k > 0; k--) {
    if (PIN_BLOCK + k * card + (k - 1) * GAP <= room) return k;
  }
  return 0;
}
