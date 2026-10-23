export type Retention = 'off' | '7' | '30' | '90';

export const PANEL_POSITIONS = ['bottom', 'top', 'left', 'right'] as const;
/** Screen edge the panel docks to. */
export type PanelPosition = (typeof PANEL_POSITIONS)[number];

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
  | 'panelPosition'
  | 'panelPinShortcut'
  | 'resetSearchShortcut'
  | 'pinnedCopy1Shortcut'
  | 'pinnedCopy2Shortcut'
  | 'pinnedCopy3Shortcut'
  | 'ocr'
  | 'dragConvert'
  | 'convertKey'
  | 'videoQuality'
  | 'videoHwAccel';

export interface Settings {
  theme: 'system' | 'light' | 'dark';
  locale: 'system' | 'ko' | 'en';
  shortcut: string;
  /** Toggles panel pinning while the panel has focus; not a global shortcut. */
  panelPinShortcut: string;
  resetSearchShortcut: string;
  /** Copy the 1st / 2nd / 3rd pinned card while the panel has focus; not global shortcuts. */
  pinnedCopy1Shortcut: string;
  pinnedCopy2Shortcut: string;
  pinnedCopy3Shortcut: string;
  sound: 'on' | 'off';
  soundName: SoundName;
  confirmDelete: 'on' | 'off';
  panelResize: 'on' | 'off';
  /** Saved panel height as a share of the screen; "" means the default height. */
  panelHeightRatio: string;
  /** Saved side-panel width as a share of the screen; "" means the default width. */
  panelWidthRatio: string;
  panelPosition: PanelPosition;
  retention: Retention;
  /** Reads text in image clips with the OS OCR so search can find it. */
  ocr: 'on' | 'off';
  /** Holding `convertKey` while dragging a card out converts it into the folder under the cursor. */
  dragConvert: 'on' | 'off';
  convertKey: 'alt' | 'ctrl' | 'shift';
  /** FFmpeg encoding quality for video conversions. */
  videoQuality: 'small' | 'normal' | 'original';
  /** Encodes MP4 / MOV / 720p with the GPU's H.264 encoder, falling back to software. */
  videoHwAccel: 'on' | 'off';
  autostart: boolean;
  version: string;
}
