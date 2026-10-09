export type KeyAction =
  | { type: "move"; delta: 1 | -1 }
  | { type: "copy"; plain: boolean }
  | { type: "togglePin" }
  | { type: "hide" }
  | { type: "cycleFilter"; delta: 1 | -1 }
  | { type: "delete" }
  | { type: "confirmDelete" }
  | { type: "cancelDelete" }
  | { type: "suggestMove"; delta: 1 | -1 }
  | { type: "suggestPick" }
  | { type: "suggestClose" }
  | { type: "clearTag" }
  | null;

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
}

const MODIFIERS = ["Shift", "Meta", "Control", "Alt"];

export function panelKeyAction(e: KeyInput): KeyAction {
  if (e.isComposing) return null;
  if (e.confirming) {
    // Auto-repeat from the Delete that opened the confirmation must not dismiss it.
    if (e.repeat || MODIFIERS.includes(e.key)) return null;
    return e.key === "Enter" ? { type: "confirmDelete" } : { type: "cancelDelete" };
  }
  if (e.suggesting) {
    switch (e.key) {
      case "ArrowDown":
        return { type: "suggestMove", delta: 1 };
      case "ArrowUp":
        return { type: "suggestMove", delta: -1 };
      case "Enter":
        return { type: "suggestPick" };
      case "Escape":
        return { type: "suggestClose" };
    }
  }
  if (e.metaOrCtrl && e.key.toLowerCase() === "p") return e.repeat ? null : { type: "togglePin" };
  switch (e.key) {
    case "ArrowRight":
      return { type: "move", delta: 1 };
    case "ArrowLeft":
      return { type: "move", delta: -1 };
    case "Enter":
      return { type: "copy", plain: e.shiftKey };
    case "Escape":
      return { type: "hide" };
    case "Tab":
      return { type: "cycleFilter", delta: e.shiftKey ? -1 : 1 };
    // A held key deletes at most one card: holding Backspace is how people clear a field.
    case "Delete":
      return e.repeat ? null : { type: "delete" };
    case "Backspace":
      if (!e.queryEmpty || e.repeat) return null;
      return e.hasTag ? { type: "clearTag" } : { type: "delete" };
    default:
      return null;
  }
}
