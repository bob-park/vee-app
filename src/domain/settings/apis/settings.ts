import type {
  GlobalShortcutKey,
  Retention,
  SettingKey,
  Settings,
  SoundName,
} from '@/domain/settings/apis/settings.dto';
import call from '@/shared/api';

export async function getSettings() {
  return call<Settings>('get_settings');
}

export async function setSetting(key: SettingKey, value: string) {
  return call<void>('set_setting', { key, value });
}

export async function setShortcut(key: GlobalShortcutKey, accel: string) {
  return call<void>('set_shortcut', { key, accel });
}

/** Setting keys of the global shortcuts that failed to register. */
export async function getShortcutFailures() {
  return call<GlobalShortcutKey[]>('get_shortcut_failures');
}

/** Opens the OS keyboard shortcut settings (macOS); a no-op on Windows. */
export async function openKeyboardSettings() {
  return call<void>('open_keyboard_settings');
}

export async function countPrunable(value: Retention) {
  return call<number>('count_prunable', { value });
}

export async function previewSound(name: SoundName) {
  return call<void>('preview_sound', { name });
}

export async function setAutostart(enabled: boolean) {
  return call<void>('set_autostart', { enabled });
}

/** null where the permission doesn't exist (Windows). */
export async function getDiskAccess() {
  return call<boolean | null>('get_disk_access');
}

export async function openDiskAccessSettings() {
  return call<void>('open_disk_access_settings');
}
