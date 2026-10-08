import { invoke } from "@tauri-apps/api/core";

export type Kind = "text" | "link" | "image" | "files";
export type Filter = Kind | "all";

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

export const SOUND_NAMES = ["none", "pop", "click", "chime", "bubble", "tap"] as const;
export type SoundName = (typeof SOUND_NAMES)[number];

export interface Settings {
  theme: "system" | "light" | "dark";
  locale: "system" | "ko" | "en";
  shortcut: string;
  sound: "on" | "off";
  soundName: SoundName;
  confirmDelete: "on" | "off";
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
}

export const api = {
  listClips: (query: string, kind: Filter, appId: number | null, offset: number, limit: number) =>
    invoke<Clip[]>("list_clips", { query, kind, appId, offset, limit }),
  listApps: (query: string) => invoke<App[]>("list_apps", { query }),
  copyClip: (id: number) => invoke<void>("copy_clip", { id }),
  /** `image` is the dragged card as a base64 PNG (see cardImage.ts); without it the thumbnail is dragged. */
  startDrag: (id: number, name: string, image: string | null) => invoke<void>("start_drag", { id, name, image }),
  deleteClip: (id: number) => invoke<void>("delete_clip", { id }),
  clearHistory: () => invoke<void>("clear_history"),
  hidePanel: () => invoke<void>("hide_panel"),
  revealPanel: () => invoke<void>("reveal_panel"),
  openSettings: () => invoke<void>("open_settings"),
  previewSound: (name: SoundName) => invoke<void>("preview_sound", { name }),
  getSettings: () => invoke<Settings>("get_settings"),
  setSetting: (key: "theme" | "locale" | "sound" | "soundName" | "confirmDelete", value: string) =>
    invoke<void>("set_setting", { key, value }),
  setAutostart: (enabled: boolean) => invoke<void>("set_autostart", { enabled }),
  setShortcut: (accel: string) => invoke<void>("set_shortcut", { accel }),
  /** null where the permission doesn't exist (Windows). */
  getDiskAccess: () => invoke<boolean | null>("get_disk_access"),
  openDiskAccessSettings: () => invoke<void>("open_disk_access_settings"),
  checkUpdate: () => invoke<UpdateStatus>("check_update"),
  getUpdateStatus: () => invoke<UpdateStatus>("get_update_status"),
  installUpdate: () => invoke<void>("install_update_and_restart"),
};
