//! Checks GitHub Releases, downloads in the background, installs only when asked.
//! Keeps checking after a download, and once more right before installing, so the newest release wins.

use crate::{
    app::{
        state::{AppState, now_ms},
        tray,
    },
    domain::whats_new,
    shared::error::{AppError, AppResult},
};
use serde::Serialize;
use std::sync::Mutex;
use std::time::Duration;
use tauri::{AppHandle, Emitter, Manager, State};
use tauri_plugin_updater::{Update, UpdaterExt};

const CHECK_EVERY: Duration = Duration::from_secs(6 * 60 * 60);
/// A stalled network (captive portal, dead Wi-Fi) gives up after this, so Restart falls back to the download in hand.
const CHECK_TIMEOUT: Duration = Duration::from_secs(20);
const DOWNLOAD_TIMEOUT: Duration = Duration::from_secs(5 * 60);

#[derive(Clone, Debug, Default, PartialEq, Serialize)]
#[serde(tag = "status", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum UpdateStatus {
    #[default]
    Idle,
    Checking,
    UpToDate { checked_at: i64 },
    Ready { version: String },
    Failed { checked_at: i64 },
}

#[derive(Default)]
pub struct UpdateState {
    status: Mutex<UpdateStatus>,
    pending: Mutex<Option<(Update, Vec<u8>)>>,
    /// Held across a check (and its download) and an install, so neither sees a half-swapped `pending`.
    busy: tauri::async_runtime::Mutex<()>,
}

impl UpdateState {
    pub fn ready_version(&self) -> Option<String> {
        match &*self.status.lock().unwrap() {
            UpdateStatus::Ready { version } => Some(version.clone()),
            _ => None,
        }
    }
}

fn set_status(app: &AppHandle, status: UpdateStatus) {
    *app.state::<UpdateState>().status.lock().unwrap() = status.clone();
    let _ = app.emit("update://status", status);
    tray::refresh(app);
}

/// `major.minor.patch` of `candidate` is strictly above `than`'s; false if either doesn't parse.
fn is_newer(candidate: &str, than: &str) -> bool {
    let parse = |v: &str| -> Option<(u64, u64, u64)> {
        let mut parts = v.split('.').map(|p| p.parse::<u64>().ok());
        let version = (parts.next()??, parts.next()??, parts.next()??);
        parts.next().is_none().then_some(version)
    };
    matches!((parse(candidate), parse(than)), (Some(c), Some(t)) if c > t)
}

/// The status after a check: a downloaded update stays ready unless a newer one was `found`.
fn settle(ready: Option<&str>, found: Result<Option<String>, ()>, now: i64) -> UpdateStatus {
    match (found, ready) {
        (Ok(Some(version)), _) => UpdateStatus::Ready { version },
        (_, Some(version)) => UpdateStatus::Ready { version: version.to_owned() },
        (Ok(None), None) => UpdateStatus::UpToDate { checked_at: now },
        (Err(()), None) => UpdateStatus::Failed { checked_at: now },
    }
}

/// Downloads the release newer than `floor` (the downloaded version), or than the running one.
async fn fetch(app: &AppHandle, floor: Option<String>) -> tauri_plugin_updater::Result<Option<(Update, Vec<u8>)>> {
    let updater = app
        .updater_builder()
        .version_comparator(move |current, release| match &floor {
            Some(floor) => is_newer(&release.version.to_string(), floor),
            None => release.version > current,
        })
        .timeout(CHECK_TIMEOUT)
        .build()?;
    let Some(mut update) = updater.check().await? else { return Ok(None) };
    update.timeout = Some(DOWNLOAD_TIMEOUT);
    let bytes = update.download(|_, _| {}, || {}).await?;
    Ok(Some((update, bytes)))
}

/// One check; the caller holds `busy`. With an update already downloaded it stays quiet
/// (no `Checking`), so the panel's "new version" chip never blinks.
async fn refresh(app: &AppHandle) -> UpdateStatus {
    let ready = app.state::<UpdateState>().ready_version();
    if ready.is_none() {
        set_status(app, UpdateStatus::Checking);
    }
    let found = match fetch(app, ready.clone()).await {
        Ok(Some((update, bytes))) => {
            let version = update.version.clone();
            whats_new::save(&app.state::<AppState>().db.lock().unwrap(), &version, update.body.as_deref());
            *app.state::<UpdateState>().pending.lock().unwrap() = Some((update, bytes));
            Ok(Some(version))
        }
        Ok(None) => Ok(None),
        Err(e) => {
            log::warn!("update check failed: {e}");
            Err(())
        }
    };
    let status = settle(ready.as_deref(), found, now_ms());
    set_status(app, status.clone());
    status
}

pub async fn check(app: &AppHandle) -> UpdateStatus {
    let state = app.state::<UpdateState>();
    let current = state.status.lock().unwrap().clone();
    if current == UpdateStatus::Checking {
        return current;
    }
    let _busy = state.busy.lock().await;
    refresh(app).await
}

/// Checks once more (a failure keeps the downloaded update), then installs and restarts.
pub async fn install_and_restart(app: &AppHandle) -> AppResult<()> {
    let state = app.state::<UpdateState>();
    let _busy = state.busy.lock().await;
    refresh(app).await;
    // Borrow, don't take: a failed install keeps the download so the user can retry.
    let pending = state.pending.lock().unwrap();
    let Some((update, bytes)) = pending.as_ref() else { return Err(AppError::Other("no update has been downloaded".into())) };
    update.install(bytes).map_err(|e| AppError::Other(e.to_string()))?;
    drop(pending);
    app.restart();
}

/// Checks at startup and every 6 hours. Skipped in debug builds.
pub fn spawn_periodic(app: AppHandle) {
    if cfg!(debug_assertions) {
        return;
    }
    std::thread::spawn(move || loop {
        tauri::async_runtime::block_on(check(&app));
        std::thread::sleep(CHECK_EVERY);
    });
}

#[tauri::command]
pub async fn check_update(app: AppHandle) -> UpdateStatus {
    check(&app).await
}

#[tauri::command]
pub fn get_update_status(state: State<UpdateState>) -> UpdateStatus {
    state.status.lock().unwrap().clone()
}

#[tauri::command]
pub async fn install_update_and_restart(app: AppHandle) -> AppResult<()> {
    install_and_restart(&app).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn status_serializes_like_the_typescript_union() {
        let json = |s: UpdateStatus| serde_json::to_value(s).unwrap();
        assert_eq!(json(UpdateStatus::Idle), serde_json::json!({ "status": "idle" }));
        assert_eq!(
            json(UpdateStatus::UpToDate { checked_at: 5 }),
            serde_json::json!({ "status": "upToDate", "checkedAt": 5 })
        );
        assert_eq!(
            json(UpdateStatus::Ready { version: "0.2.0".into() }),
            serde_json::json!({ "status": "ready", "version": "0.2.0" })
        );
    }

    #[test]
    fn only_strictly_newer_versions_replace_the_download() {
        assert!(is_newer("1.3.0", "1.2.0"));
        assert!(is_newer("1.10.0", "1.9.9"));
        assert!(!is_newer("1.2.0", "1.2.0"));
        assert!(!is_newer("1.1.0", "1.2.0")); // pulled release
        assert!(!is_newer("garbage", "1.2.0"));
    }

    #[test]
    fn a_ready_update_survives_a_failed_or_empty_check() {
        let ready = |v: &str| UpdateStatus::Ready { version: v.into() };
        assert_eq!(settle(Some("1.2.0"), Ok(Some("1.3.0".into())), 7), ready("1.3.0"));
        assert_eq!(settle(Some("1.2.0"), Ok(None), 7), ready("1.2.0"));
        assert_eq!(settle(Some("1.2.0"), Err(()), 7), ready("1.2.0")); // offline
        assert_eq!(settle(None, Ok(None), 7), UpdateStatus::UpToDate { checked_at: 7 });
        assert_eq!(settle(None, Err(()), 7), UpdateStatus::Failed { checked_at: 7 });
    }
}
