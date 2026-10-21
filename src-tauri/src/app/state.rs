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
            dragging: AtomicBool::new(false),
            panel_pinned: AtomicBool::new(false),
        }
    }

    /// Ignore clipboard changes briefly after we write the clipboard ourselves.
    /// Covers the watcher's 200ms poll plus its 0.3s debounce, even after a slow write.
    pub fn suppress_watcher(&self) {
        *self.suppress_until.lock().unwrap() = Instant::now() + Duration::from_millis(1200);
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
}
