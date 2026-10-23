use serde::Serialize;

pub const DEFAULT_SHORTCUT: &str = "CommandOrControl+Shift+V";
/// Panel-only, so it is matched in the panel instead of being registered with the OS.
pub const DEFAULT_PANEL_PIN_SHORTCUT: &str = "CommandOrControl+Shift+KeyP";
/// Panel-only, like the pin shortcut.
pub const DEFAULT_RESET_SEARCH_SHORTCUT: &str = "CommandOrControl+KeyK";
/// Panel-only: copy the 1st / 2nd / 3rd pinned card.
pub const DEFAULT_PINNED_COPY_SHORTCUTS: [&str; 3] =
    ["CommandOrControl+Digit1", "CommandOrControl+Digit2", "CommandOrControl+Digit3"];

/// Built-in copy sounds; `app::sound` holds their data.
pub const SOUND_NAMES: [&str; 5] = ["pop", "click", "chime", "bubble", "tap"];
/// `soundName` value that keeps copies silent while the sound switch stays on.
pub const SILENT: &str = "none";

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SettingsDto {
    pub theme: String,
    pub locale: String,
    pub shortcut: String,
    pub panel_pin_shortcut: String,
    pub reset_search_shortcut: String,
    pub pinned_copy1_shortcut: String,
    pub pinned_copy2_shortcut: String,
    pub pinned_copy3_shortcut: String,
    pub sound: String,
    pub sound_name: String,
    pub confirm_delete: String,
    pub retention: String,
    pub panel_resize: String,
    pub panel_height_ratio: String,
    pub panel_width_ratio: String,
    pub panel_position: String,
    pub ocr: String,
    pub drag_convert: String,
    pub convert_key: String,
    pub video_quality: String,
    pub video_hw_accel: String,
    pub autostart: bool,
    pub version: String,
}
