import call from "@/shared/api";

export type Kind = "text" | "link" | "image" | "files";
export type Filter = Kind | "all";
export type Retention = "off" | "7" | "30" | "90";

export interface Clip {
  id: number;
  kind: Kind;
  textPreview: string | null;
  charCount: number;
  thumb: string | null;
  meta: string | null;
  appName: string | null;
  appIcon: string | null;
  lastUsedAt: number;
  missing: boolean;
  isDir: boolean;
  pinned: boolean;
  /** Preview layers of a files clip: data URL per image file, null for other files. */
  stack: (string | null)[];
}

export interface App {
  id: number;
  name: string;
  icon: string | null;
  /** How many clips came from this app. */
  count: number;
}

export interface Stats {
  count: number;
  pinned: number;
  imageBytes: number;
}

export const SOUND_NAMES = ["none", "pop", "click", "chime", "bubble", "tap"] as const;
export type SoundName = (typeof SOUND_NAMES)[number];

export interface Settings {
  theme: "system" | "light" | "dark";
  locale: "system" | "ko" | "en";
  shortcut: string;
  /** Toggles panel pinning while the panel has focus; not a global shortcut. */
  panelPinShortcut: string;
  resetSearchShortcut: string;
  sound: "on" | "off";
  soundName: SoundName;
  confirmDelete: "on" | "off";
  panelResize: "on" | "off";
  /** Saved panel height as a share of the screen; "" means the default height. */
  panelHeightRatio: string;
  retention: Retention;
  autostart: boolean;
  version: string;
}

export type UpdateStatus =
  | { status: "idle" }
  | { status: "checking" }
  | { status: "upToDate"; checkedAt: number }
  | { status: "ready"; version: string }
  | { status: "failed"; checkedAt: number };

export interface ToastPayload {
  ok: boolean;
  /** What was copied, for the toast's icon. */
  kind: Kind;
  text: string | null;
  files: number;
  /** Copied as plain text, without formatting. */
  plain: boolean;
}

export const api = {
  listClips: (query: string, kind: Filter, appId: number | null, offset: number, limit: number) =>
    call<Clip[]>("list_clips", { query, kind, appId, offset, limit }),
  listApps: (query: string) => call<App[]>("list_apps", { query }),
  /** Apps that aren't excluded, including ones with no clips left. */
  listKnownApps: () => call<App[]>("list_known_apps"),
  copyClip: (id: number, plain = false) => call<void>("copy_clip", { id, plain }),
  /** Pinned clips in pin order; searches and filters never apply. */
  listPinned: () => call<Clip[]>("list_pinned"),
  /** Rejects with code "PIN_LIMIT" when 3 clips are already pinned. */
  setPinned: (id: number, pinned: boolean) => call<void>("set_pinned", { id, pinned }),
  countPrunable: (value: Retention) => call<number>("count_prunable", { value }),
  countAppClips: (appId: number) => call<number>("count_app_clips", { appId }),
  setAppExcluded: (appId: number, excluded: boolean) => call<void>("set_app_excluded", { appId, excluded }),
  listExcludedApps: () => call<App[]>("list_excluded_apps"),
  getStats: () => call<Stats>("get_stats"),
  /** `image` is the dragged card as a base64 PNG (see cardImage.ts); without it the thumbnail is dragged. */
  startDrag: (id: number, name: string, image: string | null) => call<void>("start_drag", { id, name, image }),
  deleteClip: (id: number) => call<void>("delete_clip", { id }),
  clearHistory: () => call<void>("clear_history"),
  hidePanel: () => call<void>("hide_panel"),
  /** A pinned panel stays up when focus leaves it; unpinning leaves it open. */
  setPanelPinned: (pinned: boolean) => call<void>("set_panel_pinned", { pinned }),
  /** `height` in CSS px; the backend clamps it and keeps the bottom edge put. Rejects with code "RESIZE_OFF". */
  resizePanel: (height: number, save: boolean) => call<void>("resize_panel", { height, save }),
  resetPanelHeight: () => call<void>("reset_panel_height"),
  revealPanel: () => call<void>("reveal_panel"),
  openSettings: () => call<void>("open_settings"),
  previewSound: (name: SoundName) => call<void>("preview_sound", { name }),
  getSettings: () => call<Settings>("get_settings"),
  setSetting: (key: "theme" | "locale" | "sound" | "soundName" | "confirmDelete" | "retention" | "panelResize" | "panelPinShortcut" | "resetSearchShortcut", value: string) =>
    call<void>("set_setting", { key, value }),
  setAutostart: (enabled: boolean) => call<void>("set_autostart", { enabled }),
  setShortcut: (accel: string) => call<void>("set_shortcut", { accel }),
  /** null where the permission doesn't exist (Windows). */
  getDiskAccess: () => call<boolean | null>("get_disk_access"),
  openDiskAccessSettings: () => call<void>("open_disk_access_settings"),
  checkUpdate: () => call<UpdateStatus>("check_update"),
  getUpdateStatus: () => call<UpdateStatus>("get_update_status"),
  installUpdate: () => call<void>("install_update_and_restart"),
  /** The release notes to show once after an update; null when there's nothing to show. */
  getWhatsNew: () => call<{ version: string; notes: string } | null>("get_whats_new"),
  /** Forgets the notes and hides the window. */
  dismissWhatsNew: () => call<void>("dismiss_whats_new"),
};
