//! Keeps the notes of a downloaded update so they're shown once after it's installed.

pub mod commands;

use crate::domain::settings;
use rusqlite::Connection;
use serde::Serialize;

// Not in `settings::validate`: only the backend writes these.
const VERSION_KEY: &str = "whatsNewVersion";
const NOTES_KEY: &str = "whatsNewNotes";

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WhatsNew {
    version: String,
    notes: String,
}

/// Remembers the notes of a downloaded update; blank or missing notes are skipped.
pub fn save(conn: &Connection, version: &str, notes: Option<&str>) {
    let Some(notes) = notes.filter(|n| !n.trim().is_empty()) else { return };
    if let Err(e) = settings::set_raw(conn, VERSION_KEY, version).and_then(|_| settings::set_raw(conn, NOTES_KEY, notes)) {
        log::warn!("failed to save release notes: {e}");
    }
}

/// The saved notes, once the update they belong to is the running version.
pub fn due(conn: &Connection, current: &str) -> Option<WhatsNew> {
    let get = |key| settings::raw(conn, key).ok().flatten().unwrap_or_default();
    let (version, notes) = (get(VERSION_KEY), get(NOTES_KEY));
    (version == current && !notes.trim().is_empty()).then_some(WhatsNew { version, notes })
}

/// Forgets the notes so they're shown only once.
pub fn clear(conn: &Connection) {
    for key in [VERSION_KEY, NOTES_KEY] {
        if let Err(e) = settings::set_raw(conn, key, "") {
            log::warn!("failed to clear {key}: {e}");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::shared::db;

    #[test]
    fn shows_notes_saved_for_the_running_version_only() {
        let conn = db::open_in_memory().unwrap();
        assert!(due(&conn, "0.9.0").is_none()); // fresh install
        save(&conn, "0.9.0", Some("## 새 기능\n- a"));
        assert!(due(&conn, "0.8.0").is_none()); // not installed yet
        let w = due(&conn, "0.9.0").unwrap();
        assert_eq!((w.version.as_str(), w.notes.as_str()), ("0.9.0", "## 새 기능\n- a"));
    }

    #[test]
    fn ignores_updates_without_notes() {
        let conn = db::open_in_memory().unwrap();
        save(&conn, "0.9.0", None);
        save(&conn, "0.9.0", Some("   "));
        assert!(due(&conn, "0.9.0").is_none());
    }

    #[test]
    fn clear_forgets_the_notes() {
        let conn = db::open_in_memory().unwrap();
        save(&conn, "0.9.0", Some("notes"));
        clear(&conn);
        assert!(due(&conn, "0.9.0").is_none());
    }
}
