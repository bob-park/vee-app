//! Settings domain: user preferences, their defaults and validation. Public API for other code.

pub mod commands;
mod model;
pub mod repository;

pub use model::{
    DEFAULT_PANEL_PIN_SHORTCUT, DEFAULT_PINNED_COPY_SHORTCUTS, DEFAULT_RESET_SEARCH_SHORTCUT, DEFAULT_SHORTCUT, SILENT, SOUND_NAMES,
    SettingsDto,
};

use crate::domain::clip;
use crate::shared::error::{AppError, AppResult};
use rusqlite::Connection;

/// Stored value, or the default for a known key.
pub fn get(conn: &Connection, key: &str) -> String {
    let default = match key {
        "theme" | "locale" => "system",
        "shortcut" => DEFAULT_SHORTCUT,
        "panelPinShortcut" => DEFAULT_PANEL_PIN_SHORTCUT,
        "resetSearchShortcut" => DEFAULT_RESET_SEARCH_SHORTCUT,
        "pinnedCopy1Shortcut" => DEFAULT_PINNED_COPY_SHORTCUTS[0],
        "pinnedCopy2Shortcut" => DEFAULT_PINNED_COPY_SHORTCUTS[1],
        "pinnedCopy3Shortcut" => DEFAULT_PINNED_COPY_SHORTCUTS[2],
        "sound" | "ocr" => "on",
        "soundName" => "pop",
        "panelPosition" => "bottom",
        "confirmDelete" => "off",
        "retention" | "panelResize" => "off",
        "dragConvert" => "on",
        "videoQuality" => "normal",
        "videoHwAccel" => "on",
        "convertKey" => {
            if cfg!(target_os = "macos") {
                "alt"
            } else {
                "ctrl"
            }
        }
        _ => "",
    };
    repository::get(conn, key).ok().flatten().unwrap_or_else(|| default.to_string())
}

/// The stored value only, `None` when the key was never written.
pub fn raw(conn: &Connection, key: &str) -> AppResult<Option<String>> {
    repository::get(conn, key)
}

/// Saves a value the user picked, after validating it.
pub fn set(conn: &Connection, key: &str, value: &str) -> AppResult<()> {
    validate(key, value)?;
    repository::set(conn, key, value)
}

/// Saves a key only the backend writes (shortcut, panel height, release notes, prompts), unvalidated.
pub fn set_raw(conn: &Connection, key: &str, value: &str) -> AppResult<()> {
    repository::set(conn, key, value)
}

/// The sound to play on copy, or `None` when copy sounds are off or set to silent.
pub fn copy_sound(conn: &Connection) -> Option<String> {
    let name = get(conn, "soundName");
    (get(conn, "sound") == "on" && name != SILENT).then_some(name)
}

const DAY_MS: i64 = 86_400_000;
/// Clips last used before this (ms) are pruned; `None` keeps everything.
pub fn cutoff_for(value: &str, now: i64) -> Option<i64> {
    let days: i64 = value.parse().ok()?;
    Some(now - days * DAY_MS)
}

/// How many clips a not-yet-saved retention value would delete right away.
pub fn count_prunable(conn: &Connection, value: &str, now: i64) -> AppResult<i64> {
    validate("retention", value)?;
    match cutoff_for(value, now) {
        Some(cutoff) => clip::prunable_count(conn, cutoff),
        None => Ok(0),
    }
}

fn validate(key: &str, value: &str) -> AppResult<()> {
    let ok = match key {
        "theme" => matches!(value, "system" | "light" | "dark"),
        "locale" => matches!(value, "system" | "ko" | "en"),
        "sound" => matches!(value, "on" | "off"),
        "soundName" => value == SILENT || SOUND_NAMES.contains(&value),
        "confirmDelete" => matches!(value, "on" | "off"),
        "panelPosition" => matches!(value, "bottom" | "top" | "left" | "right"),
        "retention" => matches!(value, "off" | "7" | "30" | "90"),
        "panelResize" | "ocr" | "dragConvert" | "videoHwAccel" => matches!(value, "on" | "off"),
        "convertKey" => matches!(value, "alt" | "ctrl" | "shift"),
        "videoQuality" => matches!(value, "small" | "normal" | "original"),
        // Modifiers and a key code joined by `+`, as built by the settings recorder.
        "panelPinShortcut" | "resetSearchShortcut" | "pinnedCopy1Shortcut" | "pinnedCopy2Shortcut" | "pinnedCopy3Shortcut" => {
            value.contains('+') && value.split('+').all(|p| !p.is_empty() && p.chars().all(|c| c.is_ascii_alphanumeric()))
        }
        // `panelHeightRatio` and `panelWidthRatio` are written only by the resize commands.
        _ => false,
    };
    if ok { Ok(()) } else { Err(AppError::InvalidInput(format!("invalid setting {key}={value}"))) }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::shared::db;

    #[test]
    fn validates_known_settings_only() {
        assert!(validate("theme", "dark").is_ok());
        assert!(validate("locale", "ko").is_ok());
        assert!(validate("theme", "purple").is_err());
        assert!(validate("shortcut", "Alt+X").is_err()); // shortcuts go through set_shortcut
        assert!(validate("unknown", "x").is_err());
    }

    #[test]
    fn invalid_setting_is_invalid_input_and_not_saved() {
        let conn = db::open_in_memory().unwrap();
        let err = set(&conn, "theme", "x").unwrap_err();
        assert_eq!(
            serde_json::to_value(&err).unwrap(),
            serde_json::json!({ "code": "INVALID_INPUT", "message": "invalid setting theme=x" })
        );
        assert_eq!(raw(&conn, "theme").unwrap(), None);
        set(&conn, "theme", "dark").unwrap();
        assert_eq!(get(&conn, "theme"), "dark");
    }

    #[test]
    fn copy_sound_defaults_on_and_accepts_on_off() {
        let conn = db::open_in_memory().unwrap();
        assert_eq!(get(&conn, "sound"), "on");
        assert!(validate("sound", "off").is_ok());
        assert!(validate("sound", "on").is_ok());
        assert!(validate("sound", "loud").is_err());
    }

    #[test]
    fn confirm_delete_defaults_off_and_accepts_on_off() {
        let conn = db::open_in_memory().unwrap();
        assert_eq!(get(&conn, "confirmDelete"), "off");
        assert!(validate("confirmDelete", "on").is_ok());
        assert!(validate("confirmDelete", "off").is_ok());
        assert!(validate("confirmDelete", "yes").is_err());
    }

    #[test]
    fn copy_sound_is_the_chosen_sound_or_none_when_off() {
        let conn = db::open_in_memory().unwrap();
        assert_eq!(copy_sound(&conn).as_deref(), Some("pop"));
        set_raw(&conn, "soundName", "chime").unwrap();
        assert_eq!(copy_sound(&conn).as_deref(), Some("chime"));
        set_raw(&conn, "soundName", "none").unwrap();
        assert_eq!(copy_sound(&conn), None);
        set_raw(&conn, "soundName", "chime").unwrap();
        set_raw(&conn, "sound", "off").unwrap();
        assert_eq!(copy_sound(&conn), None);
    }

    #[test]
    fn sound_name_accepts_only_built_in_sounds() {
        for name in SOUND_NAMES {
            assert!(validate("soundName", name).is_ok(), "{name}");
        }
        assert!(validate("soundName", "none").is_ok());
        assert!(validate("soundName", "Pop").is_err());
        assert!(validate("soundName", "../x.wav").is_err());
    }

    #[test]
    fn retention_defaults_off_and_accepts_known_periods() {
        let conn = db::open_in_memory().unwrap();
        assert_eq!(get(&conn, "retention"), "off");
        for v in ["off", "7", "30", "90"] {
            assert!(validate("retention", v).is_ok(), "{v}");
        }
        assert!(validate("retention", "14").is_err());
        assert!(validate("retention", "").is_err());
    }

    #[test]
    fn cutoff_is_none_when_off_and_days_back_otherwise() {
        const DAY: i64 = 86_400_000;
        let now = 100 * DAY;
        assert_eq!(cutoff_for("off", now), None);
        assert_eq!(cutoff_for("7", now), Some(93 * DAY));
        assert_eq!(cutoff_for("90", now), Some(10 * DAY));
        assert_eq!(cutoff_for("bogus", now), None);
    }

    #[test]
    fn panel_resize_is_a_switch_and_the_ratio_is_command_only() {
        assert!(validate("panelResize", "on").is_ok());
        assert!(validate("panelResize", "maybe").is_err());
        assert!(validate("panelHeightRatio", "0.4").is_err());
    }

    #[test]
    fn panel_position_defaults_to_bottom_and_accepts_four_edges() {
        let conn = db::open_in_memory().unwrap();
        assert_eq!(get(&conn, "panelPosition"), "bottom");
        for v in ["bottom", "top", "left", "right"] {
            assert!(validate("panelPosition", v).is_ok(), "{v}");
        }
        assert!(validate("panelPosition", "center").is_err());
        assert!(validate("panelWidthRatio", "0.3").is_err()); // command-only
    }

    #[test]
    fn each_axis_reads_its_own_ratio_key() {
        let conn = db::open_in_memory().unwrap();
        set_raw(&conn, "panelWidthRatio", "0.5000").unwrap();
        assert_eq!(get(&conn, "panelHeightRatio"), "");
        assert_eq!(get(&conn, "panelWidthRatio"), "0.5000");
    }

    #[test]
    fn panel_pin_shortcut_accepts_accelerators_only() {
        assert!(validate("panelPinShortcut", "CommandOrControl+Shift+KeyP").is_ok());
        assert!(validate("panelPinShortcut", "Alt+Digit1").is_ok());
        assert!(validate("panelPinShortcut", "KeyP").is_err());
        assert!(validate("panelPinShortcut", "Alt+").is_err());
        assert!(validate("panelPinShortcut", "").is_err());
        assert!(validate("resetSearchShortcut", "CommandOrControl+KeyK").is_ok());
        assert!(validate("resetSearchShortcut", "KeyK").is_err());
        for key in ["pinnedCopy1Shortcut", "pinnedCopy2Shortcut", "pinnedCopy3Shortcut"] {
            assert!(validate(key, "CommandOrControl+Digit1").is_ok(), "{key}");
            assert!(validate(key, "Digit1").is_err(), "{key}");
            assert!(validate(key, "").is_err(), "{key}");
        }
    }

    #[test]
    fn missing_settings_fall_back_to_defaults() {
        let conn = db::open_in_memory().unwrap();
        assert_eq!(get(&conn, "theme"), "system");
        assert_eq!(get(&conn, "shortcut"), DEFAULT_SHORTCUT);
        assert_eq!(get(&conn, "panelPinShortcut"), DEFAULT_PANEL_PIN_SHORTCUT);
        assert_eq!(get(&conn, "resetSearchShortcut"), DEFAULT_RESET_SEARCH_SHORTCUT);
        assert_eq!(get(&conn, "pinnedCopy1Shortcut"), "CommandOrControl+Digit1");
        assert_eq!(get(&conn, "pinnedCopy2Shortcut"), "CommandOrControl+Digit2");
        assert_eq!(get(&conn, "pinnedCopy3Shortcut"), "CommandOrControl+Digit3");
        assert_eq!(get(&conn, "panelResize"), "off");
        assert_eq!(get(&conn, "panelHeightRatio"), "");
        set_raw(&conn, "theme", "dark").unwrap();
        assert_eq!(get(&conn, "theme"), "dark");
    }

    #[test]
    fn convert_settings_default_and_validate() {
        let conn = db::open_in_memory().unwrap();
        assert_eq!(get(&conn, "dragConvert"), "on");
        assert_eq!(get(&conn, "convertKey"), if cfg!(target_os = "macos") { "alt" } else { "ctrl" });
        for v in ["alt", "ctrl", "shift"] {
            assert!(validate("convertKey", v).is_ok());
        }
        assert!(validate("convertKey", "cmd").is_err());
        assert!(validate("dragConvert", "off").is_ok());
        assert!(validate("dragConvert", "yes").is_err());
        assert_eq!(get(&conn, "videoQuality"), "normal");
        for v in ["small", "normal", "original"] {
            assert!(validate("videoQuality", v).is_ok());
        }
        assert!(validate("videoQuality", "best").is_err());
        assert_eq!(get(&conn, "videoHwAccel"), "on");
        assert!(validate("videoHwAccel", "off").is_ok());
        assert!(validate("videoHwAccel", "auto").is_err());
    }

    #[test]
    fn ocr_defaults_on_and_accepts_on_off() {
        let conn = db::open_in_memory().unwrap();
        assert_eq!(get(&conn, "ocr"), "on");
        assert!(set(&conn, "ocr", "maybe").is_err());
        set(&conn, "ocr", "off").unwrap();
        assert_eq!(get(&conn, "ocr"), "off");
    }
}
