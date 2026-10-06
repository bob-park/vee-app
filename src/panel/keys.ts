export type KeyAction =
  | { type: "move"; delta: 1 | -1 }
  | { type: "copy" }
  | { type: "hide" }
  | { type: "cycleFilter"; delta: 1 | -1 }
  | { type: "delete" }
  | null;

export interface KeyInput {
  key: string;
  shiftKey: boolean;
  /** True while Hangul/IME composition owns the keyboard. */
  isComposing: boolean;
  queryEmpty: boolean;
  /** Auto-repeat from a held key. */
  repeat: boolean;
}

export function panelKeyAction(e: KeyInput): KeyAction {
  if (e.isComposing) return null;
  switch (e.key) {
    case "ArrowRight":
      return { type: "move", delta: 1 };
    case "ArrowLeft":
      return { type: "move", delta: -1 };
    case "Enter":
      return { type: "copy" };
    case "Escape":
      return { type: "hide" };
    case "Tab":
      return { type: "cycleFilter", delta: e.shiftKey ? -1 : 1 };
    // A held key deletes at most one card: holding Backspace is how people clear a field.
    case "Delete":
      return e.repeat ? null : { type: "delete" };
    case "Backspace":
      return e.queryEmpty && !e.repeat ? { type: "delete" } : null;
    default:
      return null;
  }
}
