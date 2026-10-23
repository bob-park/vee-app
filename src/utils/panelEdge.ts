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

const MAX_PINNED = 3;
// ponytail: fixed layout heights measured off the side panel (padding, two-line toolbar, gaps, divider);
// a toolbar restyle that changes its height needs these updated.
const CHROME = 116; // panel padding 22 + toolbar 80 + history list padding 14
const PIN_BLOCK = 23; // pinned group padding 10 + divider 13
const GAP = 10;
const CARD_INSET = 32; // panel px-3 + list p-1, both sides

/**
 * How many pinned (square) cards a left/right panel of `w`×`h` CSS px shows with at least one whole
 * history card below them.
 */
export function pinsThatFit(w: number, h: number): number {
  const card = w - CARD_INSET;
  const room = h - CHROME - card;
  for (let k = MAX_PINNED; k > 0; k--) {
    if (PIN_BLOCK + k * card + (k - 1) * GAP <= room) return k;
  }
  return 0;
}
