const MODIFIER_KEYS = new Set(["Meta", "Control", "Alt", "Shift", "OS"]);

export interface KeyLike {
  key: string;
  code: string;
  metaKey: boolean;
  ctrlKey: boolean;
  altKey: boolean;
  shiftKey: boolean;
}

/** Builds a Tauri global-shortcut accelerator, or null while the combo is incomplete/invalid. */
export function acceleratorFromEvent(e: KeyLike, mac: boolean): string | null {
  if (MODIFIER_KEYS.has(e.key)) return null;
  const mods: string[] = [];
  if (mac ? e.metaKey : e.ctrlKey) mods.push("CommandOrControl");
  if (mac && e.ctrlKey) mods.push("Control");
  if (!mac && e.metaKey) mods.push("Super");
  if (e.altKey) mods.push("Alt");
  if (e.shiftKey) mods.push("Shift");
  const realModifiers = mods.filter((m) => m !== "Shift");
  if (realModifiers.length === 0) return null;
  return [...mods, e.code].join("+");
}

export function formatAccelerator(accel: string, mac: boolean): string {
  const names: Record<string, string> = mac
    ? { CommandOrControl: "⌘", Command: "⌘", Super: "⌘", Control: "⌃", Alt: "⌥", Shift: "⇧" }
    : { CommandOrControl: "Ctrl", Command: "Win", Super: "Win", Control: "Ctrl", Alt: "Alt", Shift: "Shift" };
  return accel
    .split("+")
    .map((part) => names[part] ?? part.replace(/^(Key|Digit)/, ""))
    .join(mac ? " " : " + ");
}
