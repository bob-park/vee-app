//! Clip domain: the clipboard history, pins, retention pruning and image files. Public API for other code.

pub mod commands;
mod model;
pub mod repository;

pub use model::{ClipContent, ClipDto, Kind, NewClip, STACK_LAYERS, StatsDto, classify_text};
pub use repository::{
    clear, content, delete, delete_unpinned_from_app, formats, list_pinned, next_ocr_pending, preview_png, prunable_count, prune,
    set_formats, set_ocr_text, stats, touch, unpinned_count_from_app, upsert,
};

use crate::shared::error::{AppError, AppResult};
use rusqlite::Connection;

/// The history row for a `kind` filter (`"all"` or a `Kind`), with paging clamped to sane bounds.
pub fn list(conn: &Connection, query: &str, kind: &str, app_id: Option<i64>, offset: i64, limit: i64) -> AppResult<Vec<ClipDto>> {
    let kind = match kind {
        "all" => None,
        k => Some(Kind::parse(k).ok_or_else(|| AppError::InvalidInput(format!("unknown kind: {k}")))?),
    };
    repository::list(conn, query, kind, app_id, offset.max(0), limit.clamp(1, 200))
}

/// Pins or unpins a clip. Past `MAX_PINNED` it fails with `PinLimit` and changes nothing.
pub fn set_pinned(conn: &Connection, id: i64, pinned: bool, now: i64) -> AppResult<()> {
    if repository::set_pinned(conn, id, pinned, now)? { Ok(()) } else { Err(AppError::PinLimit) }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::shared::db;

    fn db() -> (Connection, tempfile::TempDir) {
        (db::open_in_memory().unwrap(), tempfile::tempdir().unwrap())
    }

    #[test]
    fn fourth_pin_is_pin_limit() {
        let (c, d) = db();
        let ids: Vec<i64> = (1..=4).map(|n| upsert(&c, d.path(), NewClip::Text(format!("c{n}")), None, n).unwrap()).collect();
        for &id in &ids[..3] {
            set_pinned(&c, id, true, 100).unwrap();
        }
        assert!(matches!(set_pinned(&c, ids[3], true, 101), Err(AppError::PinLimit)));
        let pinned: Vec<i64> = list_pinned(&c).unwrap().iter().map(|p| p.id).collect();
        assert_eq!(pinned, ids[..3]);
        assert_eq!(list(&c, "", "all", None, 0, 50).unwrap()[0].id, ids[3]);
    }

    #[test]
    fn unknown_kind_is_invalid_input() {
        let (c, _d) = db();
        assert!(matches!(list(&c, "", "bogus", None, 0, 50), Err(AppError::InvalidInput(m)) if m == "unknown kind: bogus"));
        assert!(list(&c, "", "text", None, 0, 50).unwrap().is_empty());
    }
}
