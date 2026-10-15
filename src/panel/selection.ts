/** Selection is an index into the panel's cards in screen order: pins first, then history. */

/** The newest history card, or the first pin when the history is empty. */
export function defaultSelection(pinnedCount: number, historyCount: number): number {
  return historyCount > 0 ? pinnedCount : 0;
}

export function clampSelection(index: number, total: number): number {
  return Math.max(0, Math.min(index, total - 1));
}

/** After a reload, the card that was selected wherever it moved; if it's gone, the same index. */
export function followSelection(id: number | undefined, index: number, ids: number[]): number {
  const moved = id === undefined ? -1 : ids.indexOf(id);
  return moved >= 0 ? moved : clampSelection(index, ids.length);
}

/** A reload keeps the selected card only while the panel is on screen and no new clip arrived. */
export function keepSelectionOnReload(panelOpen: boolean, added: boolean): boolean {
  return panelOpen && !added;
}

export type Point = { x: number; y: number };

/**
 * Whether the pointer really moved since `last`. WebKit fires mousemove when content scrolls
 * under a still cursor; that must not steal the selection. The first event is only a baseline.
 */
export function pointerMoved(last: Point | null, x: number, y: number): boolean {
  return last !== null && (last.x !== x || last.y !== y);
}
