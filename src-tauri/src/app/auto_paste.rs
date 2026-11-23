//! Auto next paste: after each paste, the clipboard moves to the next card in panel order.

use crate::{
    app::{state::AppState, window},
    domain::clip::{self, ClipContent},
    shared::error::AppResult,
};
use rusqlite::Connection;
use std::sync::{Mutex, mpsc::Receiver};
use std::time::Duration;
use tauri::{AppHandle, Manager};

/// How long our own advance keeps the watcher quiet: its 200ms poll, with room to spare.
/// Short on purpose, so a real copy made right after a paste is still recorded.
const SUPPRESS_MS: u64 = 500;

/// Writes the first writable card after `cursor` and returns its id; `None` when nothing was written
/// (no cursor, or no card left). The db is locked only for the lookups, never across a write.
pub fn advance_with(
    db: &Mutex<Connection>,
    cursor: Option<i64>,
    mut write: impl FnMut(&ClipContent, Vec<(String, Vec<u8>)>) -> AppResult<()>,
) -> AppResult<Option<i64>> {
    let Some(mut at) = cursor else { return Ok(None) };
    loop {
        let (id, content, formats) = {
            let conn = db.lock().unwrap();
            let Some(id) = clip::next_after(&conn, at)? else { return Ok(None) };
            (id, clip::content(&conn, id)?, clip::formats(&conn, id)?)
        };
        // A clip deleted between the two lookups counts as unwritable.
        match content.map(|content| write(&content, formats)) {
            Some(Ok(())) => return Ok(Some(id)),
            Some(Err(e)) => log::warn!("auto next paste skipped clip {id}: {e}"),
            None => {}
        }
        at = id;
    }
}

/// Moves the clipboard to the card after the current one. Silent: no touch, toast, sound or event.
pub fn advance(app: &AppHandle) {
    // A paste into Vee's own windows (the panel search) is not part of the sequence.
    if app.webview_windows().values().any(|w| w.is_focused().unwrap_or(false)) {
        return;
    }
    let state = app.state::<AppState>();
    match advance_with(&state.db, state.paste_cursor(), |content, formats| window::write_clipboard(&state, content, formats)) {
        Ok(Some(id)) => {
            state.set_paste_cursor(Some(id));
            state.suppress_watcher_for(SUPPRESS_MS);
        }
        Ok(None) => {}
        Err(e) => log::warn!("auto next paste failed: {e}"),
    }
}

/// Waits `delay` so the pasting app reads the clipboard first, then drops every press made meanwhile:
/// they pasted the same card, so they must not advance again.
pub fn settle(rx: &Receiver<()>, delay: Duration) {
    std::thread::sleep(delay);
    while rx.try_recv().is_ok() {}
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::clip::NewClip;
    use crate::shared::{db, error::AppError};

    /// Clips a, b, c (c newest) and their ids.
    fn history() -> (Mutex<Connection>, tempfile::TempDir, [i64; 3]) {
        let (conn, dir) = (db::open_in_memory().unwrap(), tempfile::tempdir().unwrap());
        let ids = ["a", "b", "c"].map(|t| {
            let now = i64::from(t.as_bytes()[0]);
            clip::upsert(&conn, dir.path(), NewClip::Text(t.into()), None, now).unwrap()
        });
        (Mutex::new(conn), dir, ids)
    }

    fn text(content: &ClipContent) -> String {
        match content {
            ClipContent::Text(t) => t.clone(),
            other => panic!("expected text, got {other:?}"),
        }
    }

    #[test]
    fn writes_the_card_after_the_cursor() {
        let (conn, _dir, [_, b, c]) = history();
        let mut written = Vec::new();
        let cursor = advance_with(&conn, Some(c), |content, _| {
            written.push(text(content));
            Ok(())
        });
        assert_eq!(cursor.unwrap(), Some(b));
        assert_eq!(written, ["b"]);
    }

    #[test]
    fn no_cursor_means_no_advance() {
        let (conn, _dir, _) = history();
        let mut calls = 0;
        let cursor = advance_with(&conn, None, |_, _| {
            calls += 1;
            Ok(())
        });
        assert_eq!(cursor.unwrap(), None);
        assert_eq!(calls, 0);
    }

    #[test]
    fn stays_on_the_last_card() {
        let (conn, _dir, [a, ..]) = history();
        let mut calls = 0;
        let cursor = advance_with(&conn, Some(a), |_, _| {
            calls += 1;
            Ok(())
        });
        assert_eq!(cursor.unwrap(), None);
        assert_eq!(calls, 0);
    }

    #[test]
    fn skips_cards_that_fail_to_write() {
        let (conn, _dir, [a, _, c]) = history();
        let mut written = Vec::new();
        let cursor = advance_with(&conn, Some(c), |content, _| {
            if text(content) == "b" {
                return Err(AppError::Other("gone".into()));
            }
            written.push(text(content));
            Ok(())
        });
        assert_eq!(cursor.unwrap(), Some(a));
        assert_eq!(written, ["a"]);
    }

    #[test]
    fn passes_the_saved_formats_to_the_writer() {
        let (conn, _dir, [_, b, c]) = history();
        clip::set_formats(&conn.lock().unwrap(), b, &[("public.html".to_string(), b"<b>b</b>".to_vec())]).unwrap();
        let mut formats = Vec::new();
        advance_with(&conn, Some(c), |_, f| {
            formats = f;
            Ok(())
        })
        .unwrap();
        assert_eq!(formats, [("public.html".to_string(), b"<b>b</b>".to_vec())]);
    }

    #[test]
    fn settle_drops_presses_made_during_the_wait() {
        let (tx, rx) = std::sync::mpsc::channel();
        tx.send(()).unwrap();
        tx.send(()).unwrap();
        settle(&rx, Duration::from_millis(10));
        assert!(rx.try_recv().is_err());
    }
}
