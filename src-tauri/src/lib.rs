mod source_app;
mod store;
mod watcher;

use std::sync::Mutex;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use store::{ClipDto, Kind, Store};
use tauri::{AppHandle, Emitter, Manager, State};

pub struct AppState {
    pub store: Mutex<Store>,
    /// Frontmost app when the panel opened, re-activated when it closes (macOS).
    pub prev_app_pid: Mutex<Option<i32>>,
    suppress_until: Mutex<Instant>,
}

impl AppState {
    fn new(store: Store) -> Self {
        Self { store: Mutex::new(store), prev_app_pid: Mutex::new(None), suppress_until: Mutex::new(Instant::now()) }
    }

    /// Ignore clipboard changes briefly after we write the clipboard ourselves.
    pub fn suppress_watcher(&self) {
        *self.suppress_until.lock().unwrap() = Instant::now() + Duration::from_millis(500);
    }

    pub fn watcher_suppressed(&self) -> bool {
        Instant::now() < *self.suppress_until.lock().unwrap()
    }
}

pub fn now_ms() -> i64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_millis() as i64).unwrap_or(0)
}

#[tauri::command]
fn list_clips(state: State<AppState>, query: String, kind: String, offset: i64, limit: i64) -> Result<Vec<ClipDto>, String> {
    let kind = match kind.as_str() {
        "all" => None,
        k => Some(Kind::parse(k).ok_or_else(|| format!("unknown kind: {k}"))?),
    };
    state.store.lock().unwrap().list(&query, kind, offset.max(0), limit.clamp(1, 200)).map_err(|e| e.to_string())
}

#[tauri::command]
fn delete_clip(app: AppHandle, state: State<AppState>, id: i64) -> Result<(), String> {
    state.store.lock().unwrap().delete(id).map_err(|e| e.to_string())?;
    let _ = app.emit("clips://changed", ());
    Ok(())
}

#[tauri::command]
fn clear_history(app: AppHandle, state: State<AppState>) -> Result<(), String> {
    state.store.lock().unwrap().clear().map_err(|e| e.to_string())?;
    let _ = app.emit("clips://changed", ());
    Ok(())
}

pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            let data_dir = app.path().app_data_dir()?;
            let store = Store::open(&data_dir.join("vee.db"), &data_dir.join("images")).map_err(|e| e as Box<dyn std::error::Error>)?;
            app.manage(AppState::new(store));
            watcher::spawn(app.handle().clone());
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![list_clips, delete_clip, clear_history])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
