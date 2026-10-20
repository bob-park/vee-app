//! Optional engines the user downloads from Settings. Today only FFmpeg, kept under
//! `<app data>/engines/ffmpeg/`. The network is used only when the user asks for a download.

use crate::{app::state::now_ms, domain::convert::command};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::io::{self, Read};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::process::{Child, Stdio};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter, Manager};

pub const FFMPEG_VERSION: &str = "9.0.2";
const EVENT: &str = "engines://ffmpeg";
const POLL: Duration = Duration::from_millis(250);
/// `ffmpeg`, `ffprobe` as named on this platform.
const BINARIES: [&str; 2] = if cfg!(windows) { ["ffmpeg.exe", "ffprobe.exe"] } else { ["ffmpeg", "ffprobe"] };

/// One archive to download; `binaries` are the file names found anywhere inside it after unpacking.
#[derive(Debug)]
pub struct Asset {
    pub url: &'static str,
    pub sha256: &'static str,
    pub size: u64,
    pub binaries: &'static [&'static str],
}

#[cfg(target_os = "macos")]
const ASSETS: &[Asset] = &[
    Asset {
        url: "https://ffmpeg.martin-riedl.de/download/macos/arm64/1789931890_9.0.2/ffmpeg.zip",
        sha256: "c8ed4c4e6978a03c485edbfe4e0a5dc2380f8a30bba5150531b31b094492d924",
        size: 28_395_699,
        binaries: &["ffmpeg"],
    },
    Asset {
        url: "https://ffmpeg.martin-riedl.de/download/macos/arm64/1789931890_9.0.2/ffprobe.zip",
        sha256: "fcbe839537485eaee7a7a8bc5cbc0f90d53617e80943e8a5b2e31cb851197ea6",
        size: 28_317_701,
        binaries: &["ffprobe"],
    },
];

#[cfg(windows)]
const ASSETS: &[Asset] = &[Asset {
    url: "https://github.com/GyanD/codexffmpeg/releases/download/9.0.2/ffmpeg-9.0.2-essentials_build.zip",
    sha256: "60f467265b1e312373dbcd92200c2618a74850f98d3d078e94296bb3fa2047ba",
    size: 114_768_076,
    binaries: &["ffmpeg.exe", "ffprobe.exe"],
}];

#[cfg(not(any(target_os = "macos", windows)))]
const ASSETS: &[Asset] = &[];

/// The pinned FFmpeg archives for this platform.
pub fn assets() -> &'static [Asset] {
    ASSETS
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Stage {
    Download,
    Verify,
    Unpack,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum InstallError {
    Network,
    Checksum,
    Unpack,
    Disk,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "state", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum FfmpegStatus {
    /// `error`: why the last download failed; cleared by the next one.
    Missing { error: Option<InstallError> },
    Downloading { received: u64, total: u64, speed: u64, stage: Stage },
    Installed { version: String, size: u64, installed_at: i64, path: String, update: bool, error: Option<InstallError> },
}

/// `meta.json` in the install dir.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Meta {
    pub version: String,
    pub installed_at: i64,
    pub size: u64,
}

/// Lowercase hex SHA-256 of a file.
pub fn sha256_file(path: &Path) -> io::Result<String> {
    let mut file = std::fs::File::open(path)?;
    let mut hasher = Sha256::new();
    let mut buf = vec![0; 1 << 16];
    loop {
        let n = file.read(&mut buf)?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    Ok(hasher.finalize().iter().map(|b| format!("{b:02x}")).collect())
}

/// The install in `dest`, if both binaries and `meta.json` are there.
pub fn read_installed(dest: &Path) -> Option<Meta> {
    if !BINARIES.iter().all(|b| dest.join(b).is_file()) {
        return None;
    }
    serde_json::from_slice(&std::fs::read(dest.join("meta.json")).ok()?).ok()
}

/// The first file named `name` anywhere below `dir`.
fn find(dir: &Path, name: &str) -> Option<PathBuf> {
    for entry in std::fs::read_dir(dir).ok()?.flatten() {
        let path = entry.path();
        let kind = entry.file_type().ok()?;
        if kind.is_dir() {
            if let Some(found) = find(&path, name) {
                return Some(found);
            }
        } else if kind.is_file() && entry.file_name() == name {
            return Some(path);
        }
    }
    None
}

fn disk(e: io::Error) -> InstallError {
    log::warn!("engines: disk error: {e}");
    InstallError::Disk
}

fn remove_dir(path: &Path) {
    if path.exists() {
        let _ = std::fs::remove_dir_all(path);
    }
}

/// The work dirs next to `dest`: unpack, staged new install, and the old install during a swap.
fn siblings(dest: &Path) -> Result<[PathBuf; 3], InstallError> {
    let parent = dest.parent().ok_or(InstallError::Disk)?;
    let name = dest.file_name().ok_or(InstallError::Disk)?.to_string_lossy();
    Ok(["unpack", "new", "old"].map(|s| parent.join(format!(".{name}-{s}"))))
}

/// `old` holds the only usable install: a swap failed and couldn't roll back.
fn stranded(dest: &Path, old: &Path) -> bool {
    read_installed(dest).is_none() && read_installed(old).is_some()
}

/// Puts back an old install that a failed swap left next to `dest`. Call at startup.
pub fn recover(dest: &Path) {
    let Ok([.., old]) = siblings(dest) else { return };
    if stranded(dest, &old) {
        remove_dir(dest);
        match std::fs::rename(&old, dest) {
            Ok(()) => log::info!("engines: restored the previous FFmpeg install"),
            Err(e) => log::warn!("engines: couldn't restore the previous FFmpeg install from {}: {e}", old.display()),
        }
    }
}

/// Verifies every archive, unpacks them, and swaps the binaries into `dest` with a fresh `meta.json`.
/// On any error `dest` is left as it was and the work dirs are removed, except an old install a
/// failed rollback left in `.<name>-old` (restored by `recover`).
pub fn install_from(
    archives: &[(PathBuf, &Asset)],
    unpack: impl Fn(&Path, &Path) -> io::Result<()>,
    dest: &Path,
    version: &str,
    now: i64,
) -> Result<(), InstallError> {
    recover(dest);
    let [work, staged, old] = siblings(dest)?;
    if stranded(dest, &old) {
        // `recover` couldn't put it back; don't risk the only install.
        return Err(InstallError::Disk);
    }
    for (path, asset) in archives {
        if sha256_file(path).map_err(disk)? != asset.sha256 {
            log::warn!("engines: checksum mismatch for {}", asset.url);
            return Err(InstallError::Checksum);
        }
    }
    for d in [&work, &staged, &old] {
        remove_dir(d);
    }
    let result = stage(archives, &unpack, &work, &staged, version, now).and_then(|()| swap(&staged, dest, &old));
    remove_dir(&work);
    remove_dir(&staged);
    if stranded(dest, &old) {
        log::warn!("engines: keeping the previous FFmpeg install in {}", old.display());
    } else {
        remove_dir(&old);
    }
    result
}

fn stage(
    archives: &[(PathBuf, &Asset)],
    unpack: &impl Fn(&Path, &Path) -> io::Result<()>,
    work: &Path,
    staged: &Path,
    version: &str,
    now: i64,
) -> Result<(), InstallError> {
    std::fs::create_dir_all(staged).map_err(disk)?;
    let mut size = 0;
    for (i, (path, asset)) in archives.iter().enumerate() {
        let into = work.join(i.to_string());
        std::fs::create_dir_all(&into).map_err(disk)?;
        unpack(path, &into).map_err(|e| {
            log::warn!("engines: unpacking {} failed: {e}", path.display());
            InstallError::Unpack
        })?;
        for bin in asset.binaries {
            let found = find(&into, bin).ok_or_else(|| {
                log::warn!("engines: {bin} not found in {}", asset.url);
                InstallError::Unpack
            })?;
            let to = staged.join(bin);
            std::fs::rename(&found, &to).map_err(disk)?;
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                std::fs::set_permissions(&to, std::fs::Permissions::from_mode(0o755)).map_err(disk)?;
            }
            size += std::fs::metadata(&to).map_err(disk)?.len();
        }
    }
    if !BINARIES.iter().all(|b| staged.join(b).is_file()) {
        return Err(InstallError::Unpack);
    }
    let meta = Meta { version: version.into(), installed_at: now, size };
    std::fs::write(staged.join("meta.json"), serde_json::to_vec(&meta).expect("plain struct")).map_err(disk)
}

/// Moves `staged` to `dest`, keeping the old `dest` until the new one is in place.
fn swap(staged: &Path, dest: &Path, old: &Path) -> Result<(), InstallError> {
    let had = dest.exists();
    if had {
        std::fs::rename(dest, old).map_err(disk)?;
    }
    if let Err(e) = std::fs::rename(staged, dest) {
        if had && let Err(back) = std::fs::rename(old, dest) {
            log::warn!("engines: rolling back the FFmpeg swap failed: {back}");
        }
        return Err(disk(e));
    }
    Ok(())
}

// ---- Shell ----

pub struct Engines {
    dir: PathBuf,
    /// `<temp>/vee-ffmpeg-download-<identifier>`, so a dev build and the installed app don't share it.
    tmp: PathBuf,
    status: Mutex<FfmpegStatus>,
    /// The running download's cancel flag.
    cancel: Mutex<Option<Arc<AtomicBool>>>,
    /// The running curl, killed on cancel and on app exit.
    curl: Mutex<Option<Child>>,
}

/// Manages the engine state. Call once in setup, before any window is built.
/// Clears a download left by a previous run and restores an install a failed swap left aside.
pub fn manage(app: &AppHandle, data_dir: &Path) {
    let dir = data_dir.join("engines").join("ffmpeg");
    let tmp = std::env::temp_dir().join(format!("vee-ffmpeg-download-{}", app.config().identifier));
    remove_dir(&tmp);
    recover(&dir);
    let status = Mutex::new(on_disk(&dir, None));
    app.manage(Engines { dir, tmp, status, cancel: Mutex::new(None), curl: Mutex::new(None) });
}

/// Stops a running download and removes its temp dir. Call on app exit.
pub fn shutdown(app: &AppHandle) {
    let Some(e) = app.try_state::<Engines>() else { return };
    if let Some(cancel) = &*e.cancel.lock().unwrap() {
        cancel.store(true, Ordering::SeqCst);
    }
    if let Some(mut child) = e.curl.lock().unwrap().take() {
        stop(&mut child);
    }
    remove_dir(&e.tmp);
}

/// `(ffmpeg, ffprobe)` when both are installed.
pub fn ffmpeg(app: &AppHandle) -> Option<(PathBuf, PathBuf)> {
    let dir = &app.state::<Engines>().dir;
    read_installed(dir)?;
    let [ffmpeg, ffprobe] = BINARIES;
    Some((dir.join(ffmpeg), dir.join(ffprobe)))
}

/// What is installed in `dir`, with the last download's `error` attached.
pub fn on_disk(dir: &Path, error: Option<InstallError>) -> FfmpegStatus {
    match read_installed(dir) {
        Some(m) => FfmpegStatus::Installed {
            update: m.version != FFMPEG_VERSION,
            version: m.version,
            size: m.size,
            installed_at: m.installed_at,
            path: dir.to_string_lossy().into_owned(),
            error,
        },
        None => FfmpegStatus::Missing { error },
    }
}

fn set(app: &AppHandle, status: FfmpegStatus) {
    *app.state::<Engines>().status.lock().unwrap() = status.clone();
    let _ = app.emit(EVENT, status);
}

enum Stop {
    Cancelled,
    Failed(InstallError),
}

impl From<InstallError> for Stop {
    fn from(e: InstallError) -> Self {
        Stop::Failed(e)
    }
}

/// Starts downloading the pinned FFmpeg in the background. A second call while one runs does nothing.
fn download(app: &AppHandle) {
    let e = app.state::<Engines>();
    let cancel = {
        let mut slot = e.cancel.lock().unwrap();
        if slot.is_some() {
            return;
        }
        let cancel = Arc::new(AtomicBool::new(false));
        *slot = Some(cancel.clone());
        cancel
    };
    let total = assets().iter().map(|a| a.size).sum();
    set(app, FfmpegStatus::Downloading { received: 0, total, speed: 0, stage: Stage::Download });
    let app = app.clone();
    std::thread::spawn(move || {
        let e = app.state::<Engines>();
        let result = fetch_and_install(&app, &e.dir, &e.tmp, total, &cancel);
        remove_dir(&e.tmp);
        *e.cancel.lock().unwrap() = None;
        let error = match result {
            Err(Stop::Failed(reason)) => Some(reason),
            Ok(()) | Err(Stop::Cancelled) => None,
        };
        set(&app, on_disk(&e.dir, error));
    });
}

fn fetch_and_install(app: &AppHandle, dest: &Path, tmp: &Path, total: u64, cancel: &AtomicBool) -> Result<(), Stop> {
    remove_dir(tmp);
    std::fs::create_dir_all(tmp).map_err(disk)?;
    let started = Instant::now();
    let mut done = 0;
    let mut archives = Vec::new();
    for (i, asset) in assets().iter().enumerate() {
        let file = tmp.join(format!("{i}.zip"));
        curl(app, asset.url, &file, cancel, |got| {
            let received = done + got;
            let secs = started.elapsed().as_secs_f64();
            let speed = if secs > 0.0 { (received as f64 / secs) as u64 } else { 0 };
            FfmpegStatus::Downloading { received, total, speed, stage: Stage::Download }
        })?;
        done += asset.size;
        archives.push((file, asset));
    }
    set(app, FfmpegStatus::Downloading { received: total, total, speed: 0, stage: Stage::Verify });
    let unpacking = AtomicBool::new(false);
    let unpack = |zip: &Path, into: &Path| {
        if !unpacking.swap(true, Ordering::SeqCst) {
            set(app, FfmpegStatus::Downloading { received: total, total, speed: 0, stage: Stage::Unpack });
        }
        unzip(zip, into)
    };
    install_from(&archives, unpack, dest, FFMPEG_VERSION, now_ms())?;
    // Downloaded by curl, so no quarantine flag is expected; cleared in case one was added.
    #[cfg(target_os = "macos")]
    let _ = std::process::Command::new("xattr").arg("-dr").arg("com.apple.quarantine").arg(dest).status();
    Ok(())
}

/// The curl arguments before `<file> <url>`; a stalled transfer fails instead of hanging.
const CURL_ARGS: [&str; 10] =
    ["-fL", "--retry", "2", "--connect-timeout", "20", "--speed-time", "60", "--speed-limit", "1024", "-o"];

fn stop(child: &mut Child) {
    let _ = child.kill();
    let _ = child.wait();
}

/// Runs curl into `file`, publishing `progress(bytes so far)` every poll.
fn curl(app: &AppHandle, url: &str, file: &Path, cancel: &AtomicBool, progress: impl Fn(u64) -> FfmpegStatus) -> Result<(), Stop> {
    let slot = &app.state::<Engines>().curl;
    {
        // Spawned under the lock after a cancel check, so `shutdown` either sees the child or stops us here.
        let mut running = slot.lock().unwrap();
        if cancel.load(Ordering::SeqCst) {
            return Err(Stop::Cancelled);
        }
        let program = if cfg!(windows) { "curl.exe" } else { "curl" };
        let child = command(Path::new(program))
            .args(CURL_ARGS)
            .arg(file)
            .arg(url)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|e| {
                log::warn!("engines: curl spawn failed: {e}");
                Stop::Failed(InstallError::Network)
            })?;
        *running = Some(child);
    }
    loop {
        std::thread::sleep(POLL);
        let mut running = slot.lock().unwrap();
        // Taken by `shutdown`.
        let Some(child) = running.as_mut() else { return Err(Stop::Cancelled) };
        if cancel.load(Ordering::SeqCst) {
            stop(child);
            *running = None;
            return Err(Stop::Cancelled);
        }
        match child.try_wait() {
            Ok(None) => {
                drop(running);
                set(app, progress(std::fs::metadata(file).map(|m| m.len()).unwrap_or(0)));
            }
            Ok(Some(status)) => {
                *running = None;
                if status.success() {
                    return Ok(());
                }
                log::warn!("engines: curl {url} exited with {status}");
                return Err(Stop::Failed(InstallError::Network));
            }
            Err(e) => {
                stop(child);
                *running = None;
                log::warn!("engines: curl wait failed: {e}");
                return Err(Stop::Failed(InstallError::Network));
            }
        }
    }
}

/// Unpacks a zip with the OS tool.
fn unzip(zip: &Path, into: &Path) -> io::Result<()> {
    let mut c = if cfg!(windows) {
        let mut c = command(Path::new("tar"));
        c.arg("-xf").arg(zip).arg("-C").arg(into);
        c
    } else {
        let mut c = command(Path::new("ditto"));
        c.args(["-x", "-k"]).arg(zip).arg(into);
        c
    };
    let status = c.stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null()).status()?;
    if status.success() { Ok(()) } else { Err(io::Error::other(format!("unzip exited with {status}"))) }
}

/// FFmpeg engine commands.
pub mod commands {
    use super::{Engines, FfmpegStatus, on_disk};
    use crate::shared::error::{AppError, AppResult};
    use std::sync::atomic::Ordering;
    use tauri::{AppHandle, Manager, State};

    #[tauri::command]
    pub fn get_ffmpeg_status(state: State<Engines>) -> FfmpegStatus {
        state.status.lock().unwrap().clone()
    }

    /// Downloads (or updates to) the pinned FFmpeg. Progress arrives as `engines://ffmpeg`.
    #[tauri::command]
    pub fn download_ffmpeg(app: AppHandle) {
        super::download(&app);
    }

    /// Stops the running download; the status goes back to what is on disk, without an error.
    #[tauri::command]
    pub fn cancel_ffmpeg_download(state: State<Engines>) {
        if let Some(cancel) = &*state.cancel.lock().unwrap() {
            cancel.store(true, Ordering::SeqCst);
        }
    }

    // ponytail: a video job already running keeps its open binaries (unix) or fails the removal (Windows).
    #[tauri::command]
    pub fn remove_ffmpeg(app: AppHandle) -> AppResult<()> {
        let e = app.state::<Engines>();
        if e.cancel.lock().unwrap().is_some() {
            return Err(AppError::InvalidInput("FFmpeg is downloading".into()));
        }
        let removed = if e.dir.exists() { std::fs::remove_dir_all(&e.dir) } else { Ok(()) };
        // A partial removal may leave a broken dir: report what is really there.
        super::set(&app, on_disk(&e.dir, None));
        Ok(removed?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const ABC: &str = "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad";

    /// An "archive" holding `abc`, matching `asset`'s hash, plus an unpack that copies `fixture`.
    fn setup(binaries: &[&str]) -> (tempfile::TempDir, PathBuf, PathBuf) {
        let root = tempfile::tempdir().unwrap();
        let archive = root.path().join("a.zip");
        std::fs::write(&archive, b"abc").unwrap();
        let fixture = root.path().join("fixture/nested/bin");
        std::fs::create_dir_all(&fixture).unwrap();
        for b in binaries {
            std::fs::write(fixture.join(b), b"bin").unwrap();
        }
        (root, archive, fixture)
    }

    fn copy_from(fixture: PathBuf) -> impl Fn(&Path, &Path) -> io::Result<()> {
        move |_, into| {
            let to = into.join("ffmpeg-9/bin");
            std::fs::create_dir_all(&to)?;
            for e in std::fs::read_dir(&fixture)? {
                let e = e?;
                std::fs::copy(e.path(), to.join(e.file_name()))?;
            }
            Ok(())
        }
    }

    fn asset(sha256: &'static str) -> Asset {
        Asset { url: "test://a.zip", sha256, size: 3, binaries: &BINARIES }
    }

    fn existing(root: &Path) -> PathBuf {
        let dest = root.join("engines/ffmpeg");
        std::fs::create_dir_all(&dest).unwrap();
        std::fs::write(dest.join("keep"), b"old").unwrap();
        dest
    }

    #[test]
    fn hashes_files_as_lowercase_hex() {
        let (_root, archive, _) = setup(&[]);
        assert_eq!(sha256_file(&archive).unwrap(), ABC);
    }

    #[test]
    fn installs_binaries_and_meta_over_an_old_install() {
        let names = BINARIES;
        let (root, archive, fixture) = setup(&names);
        let dest = existing(root.path());
        let a = asset(ABC);
        install_from(&[(archive, &a)], copy_from(fixture), &dest, "9.0.2", 42).unwrap();
        assert!(names.iter().all(|b| dest.join(b).is_file()));
        assert!(!dest.join("keep").exists());
        assert_eq!(read_installed(&dest), Some(Meta { version: "9.0.2".into(), installed_at: 42, size: 6 }));
        // Only the install dir is left next to it.
        assert_eq!(std::fs::read_dir(dest.parent().unwrap()).unwrap().count(), 1);
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(std::fs::metadata(dest.join(&names[0])).unwrap().permissions().mode() & 0o777, 0o755);
        }
    }

    #[test]
    fn a_wrong_hash_leaves_the_old_install() {
        let names = BINARIES;
        let (root, archive, fixture) = setup(&names);
        let dest = existing(root.path());
        let a = asset("00");
        assert_eq!(install_from(&[(archive, &a)], copy_from(fixture), &dest, "9.0.2", 1), Err(InstallError::Checksum));
        assert!(dest.join("keep").exists());
    }

    #[test]
    fn an_archive_without_the_binaries_leaves_the_old_install() {
        let names = BINARIES;
        let (root, archive, fixture) = setup(&names[..1]);
        let dest = existing(root.path());
        let a = asset(ABC);
        assert_eq!(install_from(&[(archive.clone(), &a)], copy_from(fixture), &dest, "9.0.2", 1), Err(InstallError::Unpack));
        assert!(dest.join("keep").exists());
        assert_eq!(std::fs::read_dir(dest.parent().unwrap()).unwrap().count(), 1);
        let failing = |_: &Path, _: &Path| Err(io::Error::other("bad zip"));
        assert_eq!(install_from(&[(archive, &a)], failing, &dest, "9.0.2", 1), Err(InstallError::Unpack));
        assert!(dest.join("keep").exists());
    }

    #[test]
    fn a_dir_without_ffprobe_is_not_installed() {
        let root = tempfile::tempdir().unwrap();
        let [ffmpeg, _] = BINARIES;
        std::fs::write(root.path().join(ffmpeg), b"bin").unwrap();
        std::fs::write(root.path().join("meta.json"), br#"{"version":"9.0.2","installedAt":1,"size":3}"#).unwrap();
        assert_eq!(read_installed(root.path()), None);
    }

    #[test]
    fn status_serializes_for_the_settings_tab() {
        let json = |s: FfmpegStatus| serde_json::to_value(s).unwrap();
        assert_eq!(json(FfmpegStatus::Missing { error: None }), serde_json::json!({ "state": "missing", "error": null }));
        assert_eq!(
            json(FfmpegStatus::Missing { error: Some(InstallError::Network) }),
            serde_json::json!({ "state": "missing", "error": "network" })
        );
        assert_eq!(
            json(FfmpegStatus::Downloading { received: 1, total: 2, speed: 3, stage: Stage::Verify }),
            serde_json::json!({ "state": "downloading", "received": 1, "total": 2, "speed": 3, "stage": "verify" })
        );
        assert_eq!(
            json(FfmpegStatus::Installed {
                version: "9.0.2".into(),
                size: 5,
                installed_at: 6,
                path: "/p".into(),
                update: false,
                error: Some(InstallError::Checksum)
            }),
            serde_json::json!({ "state": "installed", "version": "9.0.2", "size": 5, "installedAt": 6, "path": "/p", "update": false, "error": "checksum" })
        );
    }

    /// A complete install of `version` in `dir`.
    fn installed(dir: &Path, version: &str) {
        std::fs::create_dir_all(dir).unwrap();
        for b in BINARIES {
            std::fs::write(dir.join(b), b"bin").unwrap();
        }
        let meta = Meta { version: version.into(), installed_at: 7, size: 6 };
        std::fs::write(dir.join("meta.json"), serde_json::to_vec(&meta).unwrap()).unwrap();
    }

    #[test]
    fn a_failed_update_keeps_the_old_install_with_the_error() {
        let root = tempfile::tempdir().unwrap();
        let dest = root.path().join("ffmpeg");
        installed(&dest, "9.0.1");
        let (_r, archive, fixture) = setup(&BINARIES);
        let a = asset("00");
        let err = install_from(&[(archive, &a)], copy_from(fixture), &dest, "9.0.2", 1).unwrap_err();
        let status = on_disk(&dest, Some(err));
        assert!(matches!(
            status,
            FfmpegStatus::Installed { ref version, update: true, error: Some(InstallError::Checksum), .. } if version == "9.0.1"
        ));
        assert_eq!(on_disk(&root.path().join("none"), Some(InstallError::Network)), FfmpegStatus::Missing { error: Some(InstallError::Network) });
    }

    #[test]
    fn an_old_install_left_by_a_failed_rollback_is_restored() {
        let root = tempfile::tempdir().unwrap();
        let dest = root.path().join("ffmpeg");
        let old = root.path().join(".ffmpeg-old");
        installed(&old, "9.0.1");
        // A broken leftover where the install should be.
        std::fs::create_dir_all(&dest).unwrap();
        recover(&dest);
        assert_eq!(read_installed(&dest).map(|m| m.version), Some("9.0.1".into()));
        assert!(!old.exists());
        // A newer install next to a stale old one: nothing moves.
        installed(&old, "9.0.0");
        recover(&dest);
        assert_eq!(read_installed(&dest).map(|m| m.version), Some("9.0.1".into()));
    }

    #[test]
    fn install_from_restores_a_stranded_old_install_first() {
        let root = tempfile::tempdir().unwrap();
        let dest = root.path().join("ffmpeg");
        installed(&root.path().join(".ffmpeg-old"), "9.0.1");
        let (_r, archive, fixture) = setup(&BINARIES);
        let a = asset("00");
        assert_eq!(install_from(&[(archive, &a)], copy_from(fixture), &dest, "9.0.2", 1), Err(InstallError::Checksum));
        assert_eq!(read_installed(&dest).map(|m| m.version), Some("9.0.1".into()));
    }

    #[test]
    fn curl_gives_up_on_a_stalled_download() {
        for w in [["--connect-timeout", "20"], ["--speed-time", "60"], ["--speed-limit", "1024"]] {
            assert!(CURL_ARGS.windows(2).any(|p| p == w), "{w:?}");
        }
        assert_eq!(CURL_ARGS.last(), Some(&"-o"));
    }

    #[test]
    fn pinned_assets_cover_both_binaries() {
        if assets().is_empty() {
            return;
        }
        let names: Vec<&str> = assets().iter().flat_map(|a| a.binaries.iter().copied()).collect();
        assert_eq!(names, BINARIES);
        assert!(assets().iter().all(|a| a.sha256.len() == 64 && a.url.starts_with("https://")));
    }

    /// Downloads the pinned assets for real into a temp dir:
    /// `cargo test --lib installs_pinned_ffmpeg -- --ignored --nocapture`
    #[test]
    #[ignore]
    fn installs_pinned_ffmpeg() {
        let root = tempfile::tempdir().unwrap();
        let mut archives = Vec::new();
        for (i, a) in assets().iter().enumerate() {
            let file = root.path().join(format!("{i}.zip"));
            let ok = std::process::Command::new("curl").args(CURL_ARGS).arg(&file).arg(a.url).status().unwrap();
            assert!(ok.success(), "curl {}", a.url);
            assert_eq!(std::fs::metadata(&file).unwrap().len(), a.size);
            archives.push((file, a));
        }
        let dest = root.path().join("engines/ffmpeg");
        install_from(&archives, unzip, &dest, FFMPEG_VERSION, now_ms()).unwrap();
        let meta = read_installed(&dest).unwrap();
        println!("{meta:?}");
        let out = std::process::Command::new(dest.join(BINARIES[0])).arg("-version").output().unwrap();
        println!("{}", String::from_utf8_lossy(&out.stdout).lines().next().unwrap_or_default());
        assert!(out.status.success());
        let out = std::process::Command::new(dest.join(BINARIES[1])).arg("-version").output().unwrap();
        assert!(out.status.success());
    }
}
