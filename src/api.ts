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
}

export interface Settings {
  theme: "system" | "light" | "dark";
  locale: "system" | "ko" | "en";
  shortcut: string;
  sound: "on" | "off";
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
  text: string | null;
  files: number;
  image: boolean;
}

export const api = {
  listClips: (query: string, kind: Filter, offset: number, limit: number) =>
    invoke<Clip[]>("list_clips", { query, kind, offset, limit }),
  copyClip: (id: number) => invoke<void>("copy_clip", { id }),
  deleteClip: (id: number) => invoke<void>("delete_clip", { id }),
  clearHistory: () => invoke<void>("clear_history"),
  hidePanel: () => invoke<void>("hide_panel"),
  openSettings: () => invoke<void>("open_settings"),
  getSettings: () => invoke<Settings>("get_settings"),
  setSetting: (key: "theme" | "locale" | "sound", value: string) => invoke<void>("set_setting", { key, value }),
  setAutostart: (enabled: boolean) => invoke<void>("set_autostart", { enabled }),
  setShortcut: (accel: string) => invoke<void>("set_shortcut", { accel }),
  checkUpdate: () => invoke<UpdateStatus>("check_update"),
  getUpdateStatus: () => invoke<UpdateStatus>("get_update_status"),
  installUpdate: () => invoke<void>("install_update_and_restart"),
};
