import type { Retention, Settings, SoundName } from "@/domain/settings/apis/settings.dto";
import call from "@/shared/api";

export async function getSettings() {
  return call<Settings>("get_settings");
}

export async function setSetting(
  key: "theme" | "locale" | "sound" | "soundName" | "confirmDelete" | "retention" | "panelResize" | "panelPinShortcut" | "resetSearchShortcut",
  value: string,
) {
  return call<void>("set_setting", { key, value });
}

export async function setShortcut(accel: string) {
  return call<void>("set_shortcut", { accel });
}

export async function countPrunable(value: Retention) {
  return call<number>("count_prunable", { value });
}

export async function previewSound(name: SoundName) {
  return call<void>("preview_sound", { name });
}

export async function setAutostart(enabled: boolean) {
  return call<void>("set_autostart", { enabled });
}

/** null where the permission doesn't exist (Windows). */
export async function getDiskAccess() {
  return call<boolean | null>("get_disk_access");
}

export async function openDiskAccessSettings() {
  return call<void>("open_disk_access_settings");
}
