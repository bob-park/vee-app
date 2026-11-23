//! Shared app state managed by Tauri.

use rusqlite::Connection;
use std::path::PathBuf;
use std::sync::Mutex;
use std::sync::atomic::AtomicBool;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

pub struct AppState {
    pub db: Mutex<Connection>,
    /// Where image clips keep their PNGs.
    pub images_dir: PathBuf,
    /// Frontmost app when the panel opened (or, while pinned, last in front), re-activated after a copy.
    pub prev_app_pid: Mutex<Option<i32>>,
    suppress_until: Mutex<Instant>,
    /// The clip Vee knows is on the clipboard, where auto next paste continues from.
    /// `None` when the clipboard holds something Vee didn't record or write.
    paste_cursor: Mutex<Option<i64>>,
    /// A file drag out of the panel is in progress; losing focus must not hide it.
    pub dragging: AtomicBool,
    /// The user pinned the panel: it stays up until unpinned. Memory only.
    pub panel_pinned: AtomicBool,
}

impl AppState {
    pub fn new(db: Connection, images_dir: PathBuf) -> Self {
        Self {
            db: Mutex::new(db),
            images_dir,
            prev_app_pid: Mutex::new(None),
            suppress_until: Mutex::new(Instant::now()),
            paste_cursor: Mutex::new(None),
            dragging: AtomicBool::new(false),
            panel_pinned: AtomicBool::new(false),
        }
    }

    /// Ignore clipboard changes briefly after we write the clipboard ourselves.
    /// Covers the watcher's 200ms poll plus its 0.3s debounce, even after a slow write.
    pub fn suppress_watcher(&self) {
        self.suppress_watcher_for(1200);
    }

    /// Sets (never extends) the window, so a short one can replace the default.
    pub fn suppress_watcher_for(&self, ms: u64) {
        *self.suppress_until.lock().unwrap() = Instant::now() + Duration::from_millis(ms);
    }

    pub fn paste_cursor(&self) -> Option<i64> {
        *self.paste_cursor.lock().unwrap()
    }

    pub fn set_paste_cursor(&self, id: Option<i64>) {
        *self.paste_cursor.lock().unwrap() = id;
    }

    pub fn watcher_suppressed(&self) -> bool {
        Instant::now() < *self.suppress_until.lock().unwrap()
    }
}

pub fn now_ms() -> i64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_millis() as i64).unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn self_write_suppression_outlasts_a_watcher_poll() {
        // The watcher polls every 200ms and records after 0.3s of quiet, so the window
        // must cover both after the write finishes, not just from when it started.
        let state = AppState::new(crate::shared::db::open_in_memory().unwrap(), PathBuf::new());
        state.suppress_watcher();
        std::thread::sleep(Duration::from_millis(700));
        assert!(state.watcher_suppressed());
    }

    #[test]
    fn short_suppression_replaces_the_long_one() {
        let state = AppState::new(crate::shared::db::open_in_memory().unwrap(), PathBuf::new());
        state.suppress_watcher();
        state.suppress_watcher_for(0);
        assert!(!state.watcher_suppressed());
    }

    #[test]
    fn paste_cursor_starts_unknown() {
        let state = AppState::new(crate::shared::db::open_in_memory().unwrap(), PathBuf::new());
        assert_eq!(state.paste_cursor(), None);
        state.set_paste_cursor(Some(7));
        assert_eq!(state.paste_cursor(), Some(7));
    }
}
