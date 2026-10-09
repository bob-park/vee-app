mod disk_access;
mod settings;
mod sound;
mod source_app;
mod store;
mod tray;
mod updater;
mod watcher;
mod windows;

use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use store::{AppDto, ClipDto, Kind, StatsDto, Store};
use tauri::{AppHandle, Emitter, Manager, State, WindowEvent};

pub struct AppState {
    pub store: Mutex<Store>,
    /// Frontmost app when the panel opened, re-activated when it closes (macOS).
    pub prev_app_pid: Mutex<Option<i32>>,
    suppress_until: Mutex<Instant>,
    /// A file drag out of the panel is in progress; losing focus must not hide it.
    pub dragging: AtomicBool,
}

impl AppState {
    fn new(store: Store) -> Self {
        Self {
            store: Mutex::new(store),
            prev_app_pid: Mutex::new(None),
            suppress_until: Mutex::new(Instant::now()),
            dragging: AtomicBool::new(false),
        }
    }

    /// Ignore clipboard changes briefly after we write the clipboard ourselves.
    /// Longer than clipboard-rs's 500ms macOS poll so the poll after a slow write is covered.
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

#[tauri::command]
fn list_clips(
    state: State<AppState>,
    query: String,
    kind: String,
    app_id: Option<i64>,
    offset: i64,
    limit: i64,
) -> Result<Vec<ClipDto>, String> {
    let (kind, pinned_only) = match kind.as_str() {
        "all" => (None, false),
        "pinned" => (None, true),
        k => (Some(Kind::parse(k).ok_or_else(|| format!("unknown kind: {k}"))?), false),
    };
    state
        .store
        .lock()
        .unwrap()
        .list(&query, kind, app_id, pinned_only, offset.max(0), limit.clamp(1, 200))
        .map_err(|e| e.to_string())
}

#[tauri::command]
fn list_apps(state: State<AppState>, query: String) -> Result<Vec<AppDto>, String> {
    state.store.lock().unwrap().apps(&query, 8).map_err(|e| e.to_string())
}

#[tauri::command]
fn list_known_apps(state: State<AppState>) -> Result<Vec<AppDto>, String> {
    state.store.lock().unwrap().known_apps().map_err(|e| e.to_string())
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

#[tauri::command]
fn copy_clip(app: AppHandle, id: i64, plain: bool) -> Result<(), String> {
    windows::copy_clip(&app, id, plain)
}

#[tauri::command]
fn set_pinned(app: AppHandle, state: State<AppState>, id: i64, pinned: bool) -> Result<(), String> {
    state.store.lock().unwrap().set_pinned(id, pinned).map_err(|e| e.to_string())?;
    let _ = app.emit("clips://changed", ());
    Ok(())
}

#[tauri::command]
fn count_app_clips(state: State<AppState>, app_id: i64) -> Result<i64, String> {
    state.store.lock().unwrap().app_clip_count(app_id).map_err(|e| e.to_string())
}

#[tauri::command]
fn set_app_excluded(app: AppHandle, state: State<AppState>, app_id: i64, excluded: bool) -> Result<(), String> {
    let n = state.store.lock().unwrap().set_excluded(app_id, excluded).map_err(|e| e.to_string())?;
    if n > 0 {
        log::info!("deleted {n} clips from an excluded app");
    }
    let _ = app.emit("clips://changed", ());
    let _ = app.emit("settings://changed", ());
    Ok(())
}

#[tauri::command]
fn list_excluded_apps(state: State<AppState>) -> Result<Vec<AppDto>, String> {
    state.store.lock().unwrap().excluded_apps().map_err(|e| e.to_string())
}

#[tauri::command]
fn get_stats(state: State<AppState>) -> Result<StatsDto, String> {
    state.store.lock().unwrap().stats().map_err(|e| e.to_string())
}

/// Plays a built-in copy sound for the settings preview, regardless of the on/off setting.
#[tauri::command]
fn preview_sound(name: String) -> Result<(), String> {
    if name == settings::SILENT {
        return Ok(());
    }
    if sound::wav(&name).is_none() {
        return Err(format!("unknown sound: {name}"));
    }
    sound::play(&name);
    Ok(())
}

#[tauri::command]
fn start_drag(app: AppHandle, id: i64, name: String, image: Option<String>) -> Result<(), String> {
    windows::start_drag(&app, id, &name, image.as_deref())
}

#[tauri::command]
fn hide_panel(app: AppHandle) {
    windows::hide_panel(&app, true);
}

#[tauri::command]
fn reveal_panel(app: AppHandle) {
    windows::reveal_panel(&app);
}

#[tauri::command]
fn open_settings(app: AppHandle) {
    windows::hide_panel(&app, false);
    windows::show_settings(&app);
}

pub fn run() {
    tauri::Builder::default()
        // Must be first: a second launch just opens the settings window.
        .plugin(tauri_plugin_single_instance::init(|app, _argv, _cwd| windows::show_settings(app)))
        .plugin(tauri_plugin_log::Builder::new().level(log::LevelFilter::Info).build())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .plugin(tauri_plugin_autostart::init(tauri_plugin_autostart::MacosLauncher::LaunchAgent, None))
        .plugin(tauri_plugin_updater::Builder::new().build())
        .manage(updater::UpdateState::default())
        .setup(|app| {
            #[cfg(target_os = "macos")]
            app.set_activation_policy(tauri::ActivationPolicy::Accessory);

            let data_dir = app.path().app_data_dir()?;
            let store = match Store::open(&data_dir.join("vee.db"), &data_dir.join("images")) {
                Ok(store) => store,
                Err(e) => {
                    // Never wipe the history automatically; tell the user and quit.
                    use tauri_plugin_dialog::{DialogExt, MessageDialogKind};
                    log::error!("failed to open database: {e}");
                    app.dialog()
                        .message(format!("Vee couldn't open its history database.\n\n{e}"))
                        .title("Vee")
                        .kind(MessageDialogKind::Error)
                        .show(|_| std::process::exit(1));
                    return Ok(());
                }
            };
            app.manage(AppState::new(store));
            // Windows are `create: false` so no webview can call a command before AppState exists.
            for config in app.config().app.windows.clone() {
                tauri::WebviewWindowBuilder::from_config(app.handle(), &config)?.build()?;
            }
            disk_access::prompt_once(app.handle());
            tray::create(app.handle())?;
            settings::spawn_pruner(app.handle().clone());
            watcher::spawn(app.handle().clone());
            settings::register_stored_shortcut(app.handle());
            updater::spawn_periodic(app.handle().clone());
            Ok(())
        })
        .on_window_event(|window, event| match (window.label(), event) {
            (windows::SETTINGS, WindowEvent::CloseRequested { api, .. }) => {
                api.prevent_close();
                let _ = window.hide();
            }
            (windows::PANEL, WindowEvent::Focused(false)) => {
                if !window.app_handle().state::<AppState>().dragging.load(Ordering::SeqCst) {
                    windows::hide_panel(window.app_handle(), false)
                }
            }
            _ => {}
        })
        .invoke_handler(tauri::generate_handler![
            list_clips,
            list_apps,
            delete_clip,
            clear_history,
            set_pinned,
            count_app_clips,
            set_app_excluded,
            list_excluded_apps,
            list_known_apps,
            get_stats,
            copy_clip,
            start_drag,
            hide_panel,
            reveal_panel,
            preview_sound,
            open_settings,
            settings::get_settings,
            settings::set_setting,
            settings::set_autostart,
            settings::set_shortcut,
            settings::count_prunable,
            disk_access::get_disk_access,
            disk_access::open_disk_access_settings,
            updater::check_update,
            updater::get_update_status,
            updater::install_update_and_restart
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn self_write_suppression_outlasts_a_watcher_poll() {
        // clipboard-rs polls macOS every 500ms, so the window must cover a full
        // poll after the write finishes, not just 500ms from when it started.
        let dir = tempfile::tempdir().unwrap();
        let state = AppState::new(Store::open_in_memory(dir.path()).unwrap());
        state.suppress_watcher();
        std::thread::sleep(Duration::from_millis(700));
        assert!(state.watcher_suppressed());
    }
}
