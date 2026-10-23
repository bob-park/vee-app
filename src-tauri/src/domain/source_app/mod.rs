//! Source-app domain: the apps clips came from, and which of them are excluded. Public API for other code.

pub mod commands;
mod model;
pub mod repository;

pub use model::AppDto;
pub use repository::{app_known, apps, excluded_apps, is_excluded, known_apps, upsert_app};

use crate::domain::clip;
use crate::shared::error::AppResult;
use rusqlite::Connection;

/// Excluding an app also deletes its unpinned clips; returns how many.
pub fn set_excluded(conn: &Connection, app_id: i64, excluded: bool) -> AppResult<usize> {
    repository::set_excluded(conn, app_id, excluded)?;
    if !excluded {
        return Ok(0);
    }
    clip::delete_unpinned_from_app(conn, app_id)
}

/// Unpinned clips from this app, i.e. what excluding it would delete.
pub fn app_clip_count(conn: &Connection, app_id: i64) -> AppResult<i64> {
    clip::unpinned_count_from_app(conn, app_id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::clip::{ClipDto, NewClip};
    use crate::shared::db;

    fn db() -> (Connection, tempfile::TempDir) {
        (db::open_in_memory().unwrap(), tempfile::tempdir().unwrap())
    }

    fn add(c: &Connection, d: &tempfile::TempDir, s: &str, app_id: Option<i64>, now: i64) -> i64 {
        clip::upsert(c, d.path(), NewClip::Text(s.to_string()), app_id, now).unwrap()
    }

    fn texts(clips: Vec<ClipDto>) -> Vec<String> {
        clips.into_iter().map(|c| c.text_preview.unwrap_or_default()).collect()
    }

    fn history(c: &Connection, query: &str, app_id: Option<i64>) -> Vec<String> {
        texts(clip::list(c, query, "all", app_id, 0, 50).unwrap())
    }

    #[test]
    fn upsert_app_keeps_existing_icon() {
        let (c, d) = db();
        assert!(!app_known(&c, "com.a").unwrap());
        let first = upsert_app(&c, "com.a", "A", Some(&[1, 2])).unwrap();
        let second = upsert_app(&c, "com.a", "A2", None).unwrap();
        assert_eq!(first, second);
        assert!(app_known(&c, "com.a").unwrap());
        add(&c, &d, "x", Some(first), 1);
        let clip = &clip::list(&c, "", "all", None, 0, 50).unwrap()[0];
        assert_eq!(clip.app_name.as_deref(), Some("A2"));
        assert!(clip.app_icon.is_some());
    }

    #[test]
    fn list_filters_by_app() {
        let (c, d) = db();
        let a = upsert_app(&c, "com.a", "A", None).unwrap();
        let b = upsert_app(&c, "com.b", "B", None).unwrap();
        add(&c, &d, "from a", Some(a), 1);
        add(&c, &d, "from b", Some(b), 2);
        add(&c, &d, "also a", Some(a), 3);
        assert_eq!(history(&c, "", Some(a)), vec!["also a", "from a"]);
        assert_eq!(history(&c, "also", Some(a)), vec!["also a"]);
        assert!(history(&c, "", Some(999)).is_empty());
    }

    #[test]
    fn apps_match_name_and_rank_by_clip_count() {
        let (c, d) = db();
        let safari = upsert_app(&c, "com.apple.Safari", "Safari", Some(&[1])).unwrap();
        let notes = upsert_app(&c, "com.apple.Notes", "메모", None).unwrap();
        let slack = upsert_app(&c, "com.tinyspeck.slackmacgap", "Slack", None).unwrap();
        upsert_app(&c, "com.unused", "Sapling", None).unwrap(); // no clips → never suggested
        add(&c, &d, "a", Some(safari), 1);
        add(&c, &d, "b", Some(slack), 2);
        add(&c, &d, "c", Some(slack), 3);
        add(&c, &d, "d", Some(notes), 4);
        let names = |q: &str| -> Vec<(String, i64)> {
            apps(&c, q).unwrap().into_iter().map(|a| (a.name, a.count)).collect()
        };
        assert_eq!(names("s"), vec![("Slack".to_string(), 2), ("Safari".to_string(), 1)]);
        assert_eq!(names("SA"), vec![("Safari".to_string(), 1)]);
        assert_eq!(names("메"), vec![("메모".to_string(), 1)]);
        assert_eq!(names(""), vec![("Slack".to_string(), 2), ("Safari".to_string(), 1), ("메모".to_string(), 1)]);
        assert!(names("%").is_empty());
        assert!(apps(&c, "safari").unwrap()[0].icon.as_deref().unwrap().starts_with("data:image/png;base64,"));
    }

    #[test]
    fn excluding_an_app_deletes_its_unpinned_clips_and_marks_it() {
        let (c, d) = db();
        let a = upsert_app(&c, "com.agilebits.onepassword", "1Password", None).unwrap();
        let b = upsert_app(&c, "com.apple.Notes", "Notes", None).unwrap();
        add(&c, &d, "secret", Some(a), 1);
        let kept = add(&c, &d, "pinned secret", Some(a), 2);
        clip::set_pinned(&c, kept, true, 1).unwrap();
        add(&c, &d, "note", Some(b), 3);
        assert!(!is_excluded(&c, "com.agilebits.onepassword").unwrap());
        assert!(!is_excluded(&c, "never.seen.app").unwrap());
        assert_eq!(app_clip_count(&c, a).unwrap(), 1);
        assert_eq!(set_excluded(&c, a, true).unwrap(), 1);
        assert!(is_excluded(&c, "com.agilebits.onepassword").unwrap());
        assert_eq!(history(&c, "", None), vec!["note"]);
        assert_eq!(texts(clip::list_pinned(&c).unwrap()), vec!["pinned secret"]);
        let ex = excluded_apps(&c).unwrap();
        assert_eq!((ex.len(), ex[0].name.as_str(), ex[0].count), (1, "1Password", 1));
        assert_eq!(set_excluded(&c, a, false).unwrap(), 0);
        assert!(!is_excluded(&c, "com.agilebits.onepassword").unwrap());
        assert!(excluded_apps(&c).unwrap().is_empty());
    }

    #[test]
    fn known_apps_include_apps_without_clips_but_not_excluded_ones() {
        let (c, d) = db();
        let a = upsert_app(&c, "com.agilebits.onepassword", "1Password", None).unwrap();
        let b = upsert_app(&c, "com.apple.Notes", "Notes", None).unwrap();
        upsert_app(&c, "com.apple.Terminal", "Terminal", None).unwrap();
        add(&c, &d, "note", Some(b), 1);
        add(&c, &d, "secret", Some(a), 2);
        set_excluded(&c, a, true).unwrap();
        let names = |apps: Vec<AppDto>| apps.into_iter().map(|a| a.name).collect::<Vec<_>>();
        assert_eq!(names(known_apps(&c).unwrap()), vec!["Notes", "Terminal"]);
        // Un-excluding an app whose clips were all deleted must still offer it again.
        set_excluded(&c, a, false).unwrap();
        assert_eq!(names(known_apps(&c).unwrap()), vec!["Notes", "1Password", "Terminal"]);
    }
}
