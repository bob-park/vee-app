export type KeyAction =
  | { type: 'move'; delta: 1 | -1 }
  | { type: 'copy'; plain: boolean }
  | { type: 'copyPinned'; index: PinnedSlot }
  | { type: 'togglePin' }
  | { type: 'togglePanelPin' }
  | { type: 'resetSearch' }
  | { type: 'openSettings' }
  | { type: 'hide' }
  | { type: 'cycleFilter'; delta: 1 | -1 }
  | { type: 'delete' }
  | { type: 'confirmDelete' }
  | { type: 'cancelDelete' }
  | { type: 'suggestMove'; delta: 1 | -1 }
  | { type: 'suggestPick' }
  | { type: 'suggestClose' }
  | { type: 'clearTag' }
  | null;

/** Which of the (at most three) pinned cards, left to right. */
export type PinnedSlot = 0 | 1 | 2;

export interface KeyInput {
  key: string;
  shiftKey: boolean;
  /** True while Hangul/IME composition owns the keyboard. */
  isComposing: boolean;
  queryEmpty: boolean;
  /** Auto-repeat from a held key. */
  repeat: boolean;
  /** A delete confirmation is showing on the selected card. */
  confirming: boolean;
  /** The `@app` suggestion list is open. */
  suggesting: boolean;
  /** An app tag is set in the search box. */
  hasTag: boolean;
  /** Cmd on macOS or Ctrl on Windows is held. */
  metaOrCtrl: boolean;
  /** Physical key (`KeyboardEvent.code`); `key` is a Hangul jamo on the Korean input source. */
  code: string;
  /** The keys match the panel pin shortcut from settings. */
  panelPinCombo: boolean;
  /** The keys match the reset-search shortcut from settings. */
  resetCombo: boolean;
  /** The pinned card whose copy shortcut from settings the keys match. */
  pinnedCopy: PinnedSlot | null;
  /** A card drag was cancelled a moment ago; macOS hands the cancelling Escape to the panel too. */
  justCancelledDrag: boolean;
}

const MODIFIERS = ['Shift', 'Meta', 'Control', 'Alt'];

/** Fixed panel combos (card pin, settings) as accelerators, so settings can refuse them. */
export const PANEL_KEYS = ['CommandOrControl+KeyP', 'CommandOrControl+Comma'];

export function panelKeyAction(e: KeyInput): KeyAction {
  if (e.isComposing) return null;
  if (e.confirming) {
    // Auto-repeat from the Delete that opened the confirmation must not dismiss it.
    if (e.repeat || MODIFIERS.includes(e.key)) return null;
    return e.key === 'Enter' ? { type: 'confirmDelete' } : { type: 'cancelDelete' };
  }
  if (e.suggesting) {
    switch (e.key) {
      case 'ArrowDown':
        return { type: 'suggestMove', delta: 1 };
      case 'ArrowUp':
        return { type: 'suggestMove', delta: -1 };
      case 'Enter':
        return { type: 'suggestPick' };
      case 'Escape':
        return { type: 'suggestClose' };
    }
  }
  // Before Cmd+P: the default Cmd+Shift+P would otherwise pin the card.
  if (e.panelPinCombo) return e.repeat ? null : { type: 'togglePanelPin' };
  if (e.resetCombo) return e.repeat ? null : { type: 'resetSearch' };
  if (e.pinnedCopy !== null) return e.repeat ? null : { type: 'copyPinned', index: e.pinnedCopy };
  if (e.metaOrCtrl && e.code === 'KeyP') return e.repeat ? null : { type: 'togglePin' };
  if (e.metaOrCtrl && e.code === 'Comma') return e.repeat ? null : { type: 'openSettings' };
  switch (e.key) {
    case 'ArrowRight':
      return { type: 'move', delta: 1 };
    case 'ArrowLeft':
      return { type: 'move', delta: -1 };
    case 'Enter':
      return { type: 'copy', plain: e.shiftKey };
    case 'Escape':
      return e.justCancelledDrag ? null : { type: 'hide' };
    case 'Tab':
      return { type: 'cycleFilter', delta: e.shiftKey ? -1 : 1 };
    // A held key deletes at most one card: holding Backspace is how people clear a field.
    case 'Delete':
      return e.repeat ? null : { type: 'delete' };
    case 'Backspace':
      if (!e.queryEmpty || e.repeat) return null;
      return e.hasTag ? { type: 'clearTag' } : { type: 'delete' };
    default:
      return null;
  }
}
