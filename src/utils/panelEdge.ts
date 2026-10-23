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
