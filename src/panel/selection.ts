/** Selection is an index into the panel's cards in screen order: pins first, then history. */

/** The newest history card, or the first pin when the history is empty. */
export function defaultSelection(pinnedCount: number, historyCount: number): number {
  return historyCount > 0 ? pinnedCount : 0;
}

export function clampSelection(index: number, total: number): number {
  return Math.max(0, Math.min(index, total - 1));
}
