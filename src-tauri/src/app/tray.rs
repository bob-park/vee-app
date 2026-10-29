//! Menu-bar / system-tray icon.

use crate::{
    app::{
        i18n::{labels, resolve_locale},
        state::AppState,
        updater::{self, UpdateState},
        window,
    },
    domain::settings,
};
use tauri::menu::{Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::TrayIconBuilder;
use tauri::{AppHandle, Manager, Wry};

pub const TRAY_ID: &str = "main";

fn build_menu(app: &AppHandle) -> tauri::Result<Menu<Wry>> {
    let setting = settings::get(&app.state::<AppState>().db.lock().unwrap(), "locale");
    let l = labels(resolve_locale(&setting));
    let ready = app.state::<UpdateState>().ready_version().is_some();
    Menu::with_items(
        app,
        &[
            &MenuItem::with_id(app, "open", l.open, true, None::<&str>)?,
            &MenuItem::with_id(app, "settings", l.settings, true, None::<&str>)?,
            &MenuItem::with_id(app, "update", if ready { l.restart_update } else { l.check_update }, true, None::<&str>)?,
            &PredefinedMenuItem::separator(app)?,
            &MenuItem::with_id(app, "quit", l.quit, true, None::<&str>)?,
        ],
    )
}

pub fn create(app: &AppHandle) -> tauri::Result<()> {
    let menu = build_menu(app)?;
    // macOS tints a black template glyph to match the menu bar; Windows keeps the colour icon.
    #[cfg(target_os = "macos")]
    let (icon, template) = (tauri::image::Image::from_bytes(include_bytes!("../../icons/tray.png"))?, true);
    #[cfg(not(target_os = "macos"))]
    let (icon, template) = (app.default_window_icon().cloned().expect("bundle has an icon"), false);
    TrayIconBuilder::with_id(TRAY_ID)
        .icon(icon)
        .icon_as_template(template)
        .tooltip("Vee")
        .menu(&menu)
        .show_menu_on_left_click(true)
        .on_menu_event(|app, event| match event.id().as_ref() {
            "open" => window::show_panel(app),
            "settings" => window::show_settings(app, "general"),
            "update" => {
                if app.state::<UpdateState>().ready_version().is_some() {
                    let app = app.clone();
                    tauri::async_runtime::spawn(async move {
                        if let Err(e) = updater::install_and_restart(&app).await {
                            log::error!("update install failed: {e}");
                        }
                    });
                } else {
                    let app = app.clone();
                    tauri::async_runtime::spawn(async move {
                        updater::check(&app).await;
                    });
                }
            }
            "quit" => app.exit(0),
            _ => {}
        })
        .build(app)?;
    Ok(())
}

/// Rebuilds the menu after the language changes.
pub fn refresh(app: &AppHandle) {
    let Some(tray) = app.tray_by_id(TRAY_ID) else { return };
    match build_menu(app) {
        Ok(menu) => {
            let _ = tray.set_menu(Some(menu));
        }
        Err(e) => log::warn!("tray menu rebuild failed: {e}"),
    }
}

