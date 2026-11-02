export type Retention = 'off' | '7' | '30' | '90';

export const PANEL_POSITIONS = ['bottom', 'top', 'left', 'right'] as const;
/** Screen edge the panel docks to. */
export type PanelPosition = (typeof PANEL_POSITIONS)[number];

export const SOUND_NAMES = ['none', 'pop', 'click', 'chime', 'bubble', 'tap'] as const;
export type SoundName = (typeof SOUND_NAMES)[number];

export const RECORD_FORMATS = ['mp4', 'mov', 'webm', 'gif'] as const;
/** Container a screen recording is saved as. */
export type RecordFormat = (typeof RECORD_FORMATS)[number];

/** Setting keys of the OS-wide shortcuts, saved through `set_shortcut`. */
export type GlobalShortcutKey = 'shortcut' | 'captureShortcut' | 'recordShortcut';

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
  | 'videoHwAccel'
  | 'recordFolder'
  | 'recordFormat';

export interface Settings {
  theme: 'system' | 'light' | 'dark';
  locale: 'system' | 'ko' | 'en';
  shortcut: string;
  /** Global: opens the screen capture overlay. */
  captureShortcut: string;
  /** Global: starts or stops a screen recording. */
  recordShortcut: string;
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
  /** Where recordings are saved; "" means the OS Downloads folder. */
  recordFolder: string;
  recordFormat: RecordFormat;
  autostart: boolean;
  version: string;
}
