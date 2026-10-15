mod app;
mod domain;
mod shared;

use app::{autostart, disk_access, retention, shortcut, state::AppState, tray, updater, watcher, window};
use domain::{clip, settings, source_app, whats_new};
use std::sync::atomic::Ordering;
use tauri::{Manager, WindowEvent};

pub fn run() {
    tauri::Builder::default()
        // Must be first: a second launch just opens the settings window.
        .plugin(tauri_plugin_single_instance::init(|app, _argv, _cwd| window::show_settings(app)))
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
            let images_dir = data_dir.join("images");
            let opened = shared::db::open(&data_dir.join("vee.db"))
                .and_then(|conn| std::fs::create_dir_all(&images_dir).map(|()| conn).map_err(Into::into));
            let conn = match opened {
                Ok(conn) => conn,
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
            app.manage(AppState::new(conn, images_dir));
            // Windows are `create: false` so no webview can call a command before AppState exists.
            for config in app.config().app.windows.clone() {
                tauri::WebviewWindowBuilder::from_config(app.handle(), &config)?.build()?;
            }
            window::follow_active_space(app.handle());
            whats_new::commands::show_if_due(app.handle());
            disk_access::prompt_once(app.handle());
            tray::create(app.handle())?;
            retention::spawn_pruner(app.handle().clone());
            watcher::spawn(app.handle().clone());
            shortcut::register_stored_shortcut(app.handle());
            updater::spawn_periodic(app.handle().clone());
            Ok(())
        })
        .on_window_event(|window, event| match (window.label(), event) {
            (window::SETTINGS, WindowEvent::CloseRequested { api, .. }) => {
                api.prevent_close();
                let _ = window.hide();
            }
            (window::WHATS_NEW, WindowEvent::CloseRequested { api, .. }) => {
                api.prevent_close();
                whats_new::commands::dismiss(window.app_handle());
            }
            (window::PANEL, WindowEvent::Focused(false)) => {
                let (app, state) = (window.app_handle(), window.app_handle().state::<AppState>());
                if state.panel_pinned.load(Ordering::SeqCst) {
                    window::remember_front_app(app);
                } else if !state.dragging.load(Ordering::SeqCst) {
                    window::hide_panel(app, false)
                }
            }
            _ => {}
        })
        .invoke_handler(tauri::generate_handler![
            clip::commands::list_clips,
            clip::commands::list_pinned,
            source_app::commands::list_apps,
            clip::commands::delete_clip,
            clip::commands::clear_history,
            clip::commands::set_pinned,
            source_app::commands::count_app_clips,
            source_app::commands::set_app_excluded,
            source_app::commands::list_excluded_apps,
            source_app::commands::list_known_apps,
            clip::commands::get_stats,
            clip::commands::copy_clip,
            clip::commands::start_drag,
            window::commands::hide_panel,
            window::commands::set_panel_pinned,
            window::commands::resize_panel,
            window::commands::reset_panel_height,
            window::commands::reveal_panel,
            settings::commands::preview_sound,
            window::commands::open_settings,
            settings::commands::get_settings,
            settings::commands::set_setting,
            autostart::set_autostart,
            settings::commands::set_shortcut,
            settings::commands::count_prunable,
            disk_access::get_disk_access,
            disk_access::open_disk_access_settings,
            updater::check_update,
            updater::get_update_status,
            updater::install_update_and_restart,
            whats_new::commands::get_whats_new,
            whats_new::commands::dismiss_whats_new
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

