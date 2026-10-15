export type Retention = 'off' | '7' | '30' | '90';

export const SOUND_NAMES = ['none', 'pop', 'click', 'chime', 'bubble', 'tap'] as const;
export type SoundName = (typeof SOUND_NAMES)[number];

/** Keys `set_setting` accepts. */
export type SettingKey =
  | 'theme'
  | 'locale'
  | 'sound'
  | 'soundName'
  | 'confirmDelete'
  | 'retention'
  | 'panelResize'
  | 'panelPinShortcut'
  | 'resetSearchShortcut';

export interface Settings {
  theme: 'system' | 'light' | 'dark';
  locale: 'system' | 'ko' | 'en';
  shortcut: string;
  /** Toggles panel pinning while the panel has focus; not a global shortcut. */
  panelPinShortcut: string;
  resetSearchShortcut: string;
  sound: 'on' | 'off';
  soundName: SoundName;
  confirmDelete: 'on' | 'off';
  panelResize: 'on' | 'off';
  /** Saved panel height as a share of the screen; "" means the default height. */
  panelHeightRatio: string;
  retention: Retention;
  autostart: boolean;
  version: string;
}
