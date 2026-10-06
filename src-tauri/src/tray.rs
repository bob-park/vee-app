//! Menu-bar / system-tray icon.

use crate::{AppState, settings, updater::{self, UpdateState}, windows};
use tauri::menu::{Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::{TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Manager, Wry};

pub const TRAY_ID: &str = "main";

pub struct Labels {
    pub open: &'static str,
    pub settings: &'static str,
    pub check_update: &'static str,
    pub restart_update: &'static str,
    pub quit: &'static str,
}

pub fn labels(locale: &str) -> Labels {
    if locale == "ko" {
        Labels { open: "열기", settings: "설정", check_update: "업데이트 확인", restart_update: "업데이트 후 재시작", quit: "종료" }
    } else {
        Labels { open: "Open", settings: "Settings", check_update: "Check for Updates", restart_update: "Restart to Update", quit: "Quit" }
    }
}

/// Same rule as the frontend: "system" follows the OS language.
pub fn resolve_locale(setting: &str) -> &'static str {
    match setting {
        "ko" => "ko",
        "en" => "en",
        _ if sys_locale::get_locale().is_some_and(|l| l.to_lowercase().starts_with("ko")) => "ko",
        _ => "en",
    }
}

fn build_menu(app: &AppHandle) -> tauri::Result<Menu<Wry>> {
    let setting = settings::get(&app.state::<AppState>().store.lock().unwrap(), "locale");
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
    TrayIconBuilder::with_id(TRAY_ID)
        .icon(app.default_window_icon().cloned().expect("bundle has an icon"))
        .tooltip("Vee")
        .menu(&menu)
        // Left click is reserved for double-click → panel; right click opens the menu.
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id().as_ref() {
            "open" => windows::show_panel(app),
            "settings" => windows::show_settings(app),
            "update" => {
                if app.state::<UpdateState>().ready_version().is_some() {
                    if let Err(e) = updater::install_and_restart(app) {
                        log::error!("update install failed: {e}");
                    }
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
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::DoubleClick { .. } = event {
                windows::show_panel(tray.app_handle());
            }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn explicit_locale_wins_over_system() {
        assert_eq!(resolve_locale("ko"), "ko");
        assert_eq!(resolve_locale("en"), "en");
        assert!(["ko", "en"].contains(&resolve_locale("system")));
        assert_eq!(labels("ko").quit, "종료");
        assert_eq!(labels("en").quit, "Quit");
    }
}
