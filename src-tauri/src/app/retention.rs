//! Background pruning of clips past the retention period.

use crate::{
    app::state::{AppState, now_ms},
    domain::{clip, settings},
    shared::error::AppResult,
};
use tauri::{AppHandle, Emitter, Manager};

const PRUNE_EVERY: std::time::Duration = std::time::Duration::from_secs(60 * 60);

/// Prunes by the saved retention and tells the panel when anything went.
pub fn prune_now(app: &AppHandle) -> AppResult<usize> {
    let state = app.state::<AppState>();
    let n = {
        let conn = state.db.lock().unwrap();
        match settings::cutoff_for(&settings::get(&conn, "retention"), now_ms()) {
            Some(cutoff) => clip::prune(&conn, cutoff)?,
            None => 0,
        }
    };
    if n > 0 {
        log::info!("pruned {n} clips");
        let _ = app.emit("clips://changed", ());
    }
    Ok(n)
}

/// Prunes now and then every hour.
pub fn spawn_pruner(app: AppHandle) {
    std::thread::spawn(move || loop {
        if let Err(e) = prune_now(&app) {
            log::warn!("pruning failed: {e}");
        }
        std::thread::sleep(PRUNE_EVERY);
    });
}
