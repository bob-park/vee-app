//! Optional engines the user downloads from Settings: FFmpeg, rhwp and the background removal model,
//! each kept under `<app data>/engines/<name>/`. The network is used to look up FFmpeg's and rhwp's
//! latest release when the Convert tab opens, and to download an engine when the user asks. Model
//! engines are pinned here instead of looked up.

use crate::{
    app::state::now_ms,
    domain::convert::command,
    shared::error::{AppError, AppResult},
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::io::{self, Read};
use std::path::{Path, PathBuf};
use std::process::{Child, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter, Manager};

const EVENT: &str = "engines://status";
const POLL: Duration = Duration::from_millis(250);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum EngineId {
    Ffmpeg,
    Rhwp,
    Bgremove,
    Sam,
}

impl EngineId {
    pub const ALL: [EngineId; 4] = [EngineId::Ffmpeg, EngineId::Rhwp, EngineId::Bgremove, EngineId::Sam];

    pub fn parse(s: &str) -> AppResult<Self> {
        Self::ALL.into_iter().find(|id| id.spec().dir_name == s).ok_or_else(|| AppError::InvalidInput(format!("unknown engine {s}")))
    }

    pub fn spec(self) -> &'static EngineSpec {
        &SPECS[self as usize]
    }
}

/// How a downloaded file becomes the engine's binaries.
#[derive(Debug, Clone, PartialEq)]
pub enum AssetKind {
    /// The file itself, installed as `name` (a model).
    Raw { name: &'static str },
    /// A `tar -xf` readable archive; the engine's binaries are found by file name inside it.
    Archive,
}

/// One file to download.
#[derive(Debug, Clone, PartialEq)]
pub struct Asset {
    pub url: String,
    /// Lowercase hex, as published next to the file by its source.
    pub sha256: String,
    pub size: u64,
    pub kind: AssetKind,
}

/// An engine's latest release, as its source publishes it.
#[derive(Debug, Clone, PartialEq)]
pub struct Release {
    pub version: String,
    pub assets: Vec<Asset>,
}

impl Release {
    /// Bytes a download of this release fetches.
    pub fn size(&self) -> u64 {
        self.assets.iter().map(|a| a.size).sum()
    }

    /// A plain version, and every file from a known host with a full SHA-256.
    fn valid(&self) -> bool {
        let version = !self.version.is_empty() && self.version.split('.').all(|p| !p.is_empty() && p.bytes().all(|c| c.is_ascii_digit()));
        let file = |a: &Asset| {
            SOURCES.iter().any(|h| a.url.starts_with(h))
                && a.sha256.len() == 64
                && a.sha256.bytes().all(|c| matches!(c, b'0'..=b'9' | b'a'..=b'f'))
        };
        version && !self.assets.is_empty() && self.assets.iter().all(file)
    }
}

/// The only places a release may point at.
const SOURCES: [&str; 4] = [
    "https://github.com/edwardkim/rhwp/releases/download/",
    "https://github.com/GyanD/codexffmpeg/releases/download/",
    RIEDL_DOWNLOAD,
    "https://github.com/danielgatis/rembg/releases/download/",
];
const RIEDL_DOWNLOAD: &str = "https://ffmpeg.martin-riedl.de/download/";

/// One downloadable engine.
pub struct EngineSpec {
    /// The install dir's name, and the engine's serde name.
    pub dir_name: &'static str,
    /// Every file an install must hold, as named on this platform.
    pub binaries: &'static [&'static str],
}

/// Indexed by `EngineId as usize`.
pub const SPECS: [EngineSpec; 4] = [
    EngineSpec { dir_name: "ffmpeg", binaries: if cfg!(windows) { &["ffmpeg.exe", "ffprobe.exe"] } else { &["ffmpeg", "ffprobe"] } },
    EngineSpec { dir_name: "rhwp", binaries: if cfg!(windows) { &["rhwp.exe"] } else { &["rhwp"] } },
    EngineSpec { dir_name: "bgremove", binaries: &["model.onnx"] },
    EngineSpec { dir_name: "sam", binaries: &["encoder.onnx", "decoder.onnx"] },
];

// ---- Pinned models ----
// The model files only (BiRefNet general, swin-v1-tiny, MIT; SAM ViT-B quantized encoder and
// decoder, Apache-2.0; both from rembg's release); checksums and sizes measured when pinning. ONNX Runtime ships inside the app (`ai::runtime_path`), signed with Vee's
// identity: the hardened runtime refuses a downloaded library signed by another team.

// Bump an engine's version when its pinned files change; drives the update badge.
// Bgremove 2: the runtime left the engine.

/// The pinned release of a model engine; `None` for looked-up engines and platforms without a bundled runtime.
fn pinned(id: EngineId) -> Option<Release> {
    if !cfg!(any(target_os = "macos", windows)) {
        return None;
    }
    let raw = |url: &str, sha256: &str, size, name| Asset { url: url.into(), sha256: sha256.into(), size, kind: AssetKind::Raw { name } };
    let (version, assets) = match id {
        EngineId::Bgremove => (
            "2",
            vec![raw(
                "https://github.com/danielgatis/rembg/releases/download/v0.0.0/BiRefNet-general-bb_swin_v1_tiny-epoch_232.onnx",
                "5600024376f572a557870a5eb0afb1e5961636bef4e1e22132025467d0f03333",
                224_005_088,
                "model.onnx",
            )],
        ),
        EngineId::Sam => (
            "1",
            vec![
                raw(
                    "https://github.com/danielgatis/rembg/releases/download/v0.0.0/sam_vit_b_01ec64.encoder.quant.onnx",
                    "181301ce3500a86c38450b56e2b71dab8008a13b1812a0d6cc812f22651e143a",
                    108_836_115,
                    "encoder.onnx",
                ),
                raw(
                    "https://github.com/danielgatis/rembg/releases/download/v0.0.0/sam_vit_b_01ec64.decoder.quant.onnx",
                    "b11465135b7c93a1fd65141a4e2a96f1ddf2e3dab09b5eeab640bdf5f10aadbe",
                    8_755_366,
                    "decoder.onnx",
                ),
            ],
        ),
        EngineId::Ffmpeg | EngineId::Rhwp => return None,
    };
    Some(Release { version: version.into(), assets })
}

// ---- Latest releases ----
// FFmpeg: martin-riedl.de's static builds on macOS (one zip per binary), gyan.dev's essentials build
// (its GitHub mirror) on Windows. rhwp: its own GitHub releases. All GPL/own-licensed, downloaded by
// the user, not bundled. Checksums come from the source: GitHub's asset `digest`, martin-riedl's `.sha256`.

const RHWP_LATEST: &str = "https://api.github.com/repos/edwardkim/rhwp/releases/latest";
#[cfg_attr(not(windows), allow(dead_code))]
const GYAN_RELEASES: &str = "https://api.github.com/repos/GyanD/codexffmpeg/releases?per_page=30";
#[cfg_attr(not(any(target_os = "macos", test)), allow(dead_code))]
const RIEDL_LATEST: &str = "https://ffmpeg.martin-riedl.de/redirect/latest/macos/arm64/release";

/// The archive named `name` in a GitHub release, with the checksum GitHub computed for it.
fn github_asset(release: &serde_json::Value, name: &str) -> Option<Asset> {
    let a = release["assets"].as_array()?.iter().find(|a| a["name"] == name)?;
    Some(Asset {
        url: a["browser_download_url"].as_str()?.to_owned(),
        sha256: a["digest"].as_str()?.strip_prefix("sha256:")?.to_owned(),
        size: a["size"].as_u64()?,
        kind: AssetKind::Archive,
    })
}

/// rhwp's latest release (`tag_name` `v<ver>`) and its `rhwp-v<ver>-<platform>` archive.
#[cfg_attr(not(any(target_os = "macos", windows, test)), allow(dead_code))]
fn parse_rhwp(json: &str, platform: &str) -> Option<Release> {
    let release: serde_json::Value = serde_json::from_str(json).ok()?;
    let version = release["tag_name"].as_str()?.strip_prefix('v')?;
    let asset = github_asset(&release, &format!("rhwp-v{version}-{platform}"))?;
    Some(Release { version: version.to_owned(), assets: vec![asset] })
}

/// The newest gyan.dev release version (a tag of digits and dots; the daily `…-git-…` builds are skipped).
#[cfg_attr(not(any(windows, test)), allow(dead_code))]
fn parse_gyan(json: &str) -> Option<Release> {
    let releases: Vec<serde_json::Value> = serde_json::from_str(json).ok()?;
    let release = releases.iter().find(|r| r["tag_name"].as_str().is_some_and(|t| t.bytes().all(|c| c.is_ascii_digit() || c == b'.')))?;
    let version = release["tag_name"].as_str()?;
    let asset = github_asset(release, &format!("ffmpeg-{version}-essentials_build.zip"))?;
    Some(Release { version: version.to_owned(), assets: vec![asset] })
}

/// `…/download/macos/arm64/<id>_<ver>/ffmpeg.zip` → `<ver>`.
#[cfg_attr(not(any(target_os = "macos", test)), allow(dead_code))]
fn parse_riedl_url(url: &str) -> Option<String> {
    let dir = url.strip_prefix(RIEDL_DOWNLOAD)?.rsplit('/').nth(1)?;
    Some(dir.split_once('_')?.1.to_owned())
}

/// `<sha256>  ffmpeg.zip` → the hash, if it is one.
#[cfg_attr(not(any(target_os = "macos", test)), allow(dead_code))]
fn parse_sha256_file(text: &str) -> Option<String> {
    let hash = text.split_whitespace().next()?.to_ascii_lowercase();
    (hash.len() == 64 && hash.bytes().all(|c| c.is_ascii_hexdigit())).then_some(hash)
}

/// The last `content-length` in HEAD response headers (the one after any redirect).
#[cfg_attr(not(any(target_os = "macos", test)), allow(dead_code))]
fn parse_content_length(headers: &str) -> Option<u64> {
    headers.lines().filter_map(|l| l.split_once(':')).rfind(|(k, _)| k.trim().eq_ignore_ascii_case("content-length"))?.1.trim().parse().ok()
}

/// The curl arguments of a lookup, before its own.
const LOOKUP_ARGS: [&str; 11] = [
    "-sf",
    "--proto",
    "=https",
    "--proto-redir",
    "=https",
    "--connect-timeout",
    "10",
    "--max-time",
    "20",
    "-H",
    "Accept: application/vnd.github+json",
];

/// curl's stdout for a short request, or `None` on any failure (offline, HTTP error, rate limit).
fn curl_text(args: &[&str]) -> Option<String> {
    let program = if cfg!(windows) { "curl.exe" } else { "curl" };
    let out = command(Path::new(program))
        .args(LOOKUP_ARGS)
        .args(args)
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .output()
        .ok()?;
    out.status.success().then(|| String::from_utf8_lossy(&out.stdout).into_owned())
}

/// Looks up the engine's latest release for this platform; `None` when the source can't be reached
/// or answers with something unexpected.
pub fn resolve(id: EngineId) -> Option<Release> {
    let release = match id {
        EngineId::Rhwp => resolve_rhwp(),
        EngineId::Ffmpeg => resolve_ffmpeg(),
        EngineId::Bgremove | EngineId::Sam => pinned(id),
    };
    let release = release.filter(Release::valid);
    if release.is_none() {
        log::info!("engines: couldn't look up the latest {}", id.spec().dir_name);
    }
    release
}

fn resolve_rhwp() -> Option<Release> {
    let platform = if cfg!(target_os = "macos") {
        "macos-aarch64.tar.gz"
    } else if cfg!(windows) {
        "windows-x86_64.zip"
    } else {
        return None;
    };
    parse_rhwp(&curl_text(&[RHWP_LATEST])?, platform)
}

#[cfg(windows)]
fn resolve_ffmpeg() -> Option<Release> {
    parse_gyan(&curl_text(&[GYAN_RELEASES])?)
}

#[cfg(target_os = "macos")]
fn resolve_ffmpeg() -> Option<Release> {
    let mut version = None;
    let mut assets = Vec::new();
    for bin in SPECS[EngineId::Ffmpeg as usize].binaries {
        // GET, not HEAD: the redirect endpoint answers HEAD with 404 most of the time.
        let url = curl_text(&["-o", "/dev/null", "-w", "%{redirect_url}", &format!("{RIEDL_LATEST}/{bin}.zip")])?;
        let v = parse_riedl_url(&url)?;
        // Both zips must come from the same release.
        if version.get_or_insert_with(|| v.clone()) != &v {
            return None;
        }
        let sha256 = parse_sha256_file(&curl_text(&[&format!("{url}.sha256")])?)?;
        let size = parse_content_length(&curl_text(&["-I", &url])?)?;
        assets.push(Asset { url, sha256, size, kind: AssetKind::Archive });
    }
    Some(Release { version: version?, assets })
}

#[cfg(not(any(target_os = "macos", windows)))]
fn resolve_ffmpeg() -> Option<Release> {
    None
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
    /// An archive didn't unpack or lacked a binary.
    Unpack,
    Disk,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "state", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum EngineStatus {
    /// `error`: why the last download failed; cleared by the next one. `download_size`: bytes a download
    /// fetches, once the latest release has been looked up.
    Missing { error: Option<InstallError>, download_size: Option<u64> },
    Downloading { received: u64, total: u64, speed: u64, stage: Stage },
    Installed { version: String, size: u64, installed_at: i64, path: String, update: bool, error: Option<InstallError> },
}

/// The `engines://status` payload.
#[derive(Debug, Clone, Serialize)]
pub struct StatusEvent {
    pub engine: EngineId,
    pub status: EngineStatus,
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

/// The install in `dest`, if every binary and `meta.json` are there.
pub fn read_installed(dest: &Path, binaries: &[&str]) -> Option<Meta> {
    if !binaries.iter().all(|b| dest.join(b).is_file()) {
        return None;
    }
    serde_json::from_slice(&std::fs::read(dest.join("meta.json")).ok()?).ok()
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

/// The work dirs next to `dest`: the staged new install, and the old install during a swap.
fn siblings(dest: &Path) -> Result<[PathBuf; 2], InstallError> {
    let parent = dest.parent().ok_or(InstallError::Disk)?;
    let name = dest.file_name().ok_or(InstallError::Disk)?.to_string_lossy();
    Ok(["new", "old"].map(|s| parent.join(format!(".{name}-{s}"))))
}

/// `old` holds the only usable install: a swap failed and couldn't roll back.
fn stranded(dest: &Path, old: &Path, binaries: &[&str]) -> bool {
    read_installed(dest, binaries).is_none() && read_installed(old, binaries).is_some()
}

/// Puts back an old install that a failed swap left next to `dest`. Call at startup.
pub fn recover(dest: &Path, binaries: &[&str]) {
    let Ok([_, old]) = siblings(dest) else { return };
    if stranded(dest, &old, binaries) {
        remove_dir(dest);
        match std::fs::rename(&old, dest) {
            Ok(()) => log::info!("engines: restored the previous install in {}", dest.display()),
            Err(e) => log::warn!("engines: couldn't restore the previous install from {}: {e}", old.display()),
        }
    }
}

/// Removes the work dirs next to `dest`, keeping an old install that is the only usable one.
fn tidy(dest: &Path, binaries: &[&str]) {
    let Ok([staged, old]) = siblings(dest) else { return };
    remove_dir(&staged);
    if stranded(dest, &old, binaries) {
        log::warn!("engines: keeping the previous install in {}", old.display());
    } else {
        remove_dir(&old);
    }
}

/// Verifies every downloaded file, puts each binary in place (a raw file under its installed name,
/// an archive unpacked next to its download and searched by file name), and swaps them into `dest`
/// with a fresh `meta.json`. `on_unpack` runs before an archive is unpacked. On any error `dest` is
/// left as it was and the work dirs are removed, except an old install a failed rollback left in
/// `.<name>-old` (restored by `recover`).
pub fn install_from(
    binaries: &[&str],
    files: &[(PathBuf, &Asset)],
    dest: &Path,
    version: &str,
    now: i64,
    on_unpack: &dyn Fn(),
) -> Result<(), InstallError> {
    recover(dest, binaries);
    let [staged, old] = siblings(dest)?;
    if stranded(dest, &old, binaries) {
        // `recover` couldn't put it back; don't risk the only install.
        return Err(InstallError::Disk);
    }
    for (path, asset) in files {
        if sha256_file(path).map_err(disk)? != asset.sha256 {
            log::warn!("engines: checksum mismatch for {}", asset.url);
            return Err(InstallError::Checksum);
        }
    }
    remove_dir(&staged);
    remove_dir(&old);
    let result = stage(binaries, files, &staged, version, now, on_unpack).and_then(|()| swap(&staged, dest, &old));
    tidy(dest, binaries);
    result
}

fn stage(
    binaries: &[&str],
    files: &[(PathBuf, &Asset)],
    staged: &Path,
    version: &str,
    now: i64,
    on_unpack: &dyn Fn(),
) -> Result<(), InstallError> {
    std::fs::create_dir_all(staged).map_err(disk)?;
    let mut size = 0;
    for (path, asset) in files {
        match asset.kind {
            AssetKind::Raw { name } => size += place(path, &staged.join(name))?,
            AssetKind::Archive => {
                on_unpack();
                let mut work = path.as_os_str().to_owned();
                work.push("-unpack");
                let work = PathBuf::from(work);
                let placed = unpack_into(path, &work, binaries, staged);
                remove_dir(&work);
                size += placed?;
            }
        }
    }
    if let Some(bin) = binaries.iter().find(|b| !staged.join(b).is_file()) {
        log::warn!("engines: {bin} not found in the downloads");
        return Err(InstallError::Unpack);
    }
    let meta = Meta { version: version.into(), installed_at: now, size };
    std::fs::write(staged.join("meta.json"), serde_json::to_vec(&meta).expect("plain struct")).map_err(disk)
}

/// Unpacks `archive` into `work` and places the binaries found in it into `staged`; returns their size.
/// An archive may hold only some of them (one zip per binary); `stage` checks that all arrived.
fn unpack_into(archive: &Path, work: &Path, binaries: &[&str], staged: &Path) -> Result<u64, InstallError> {
    remove_dir(work);
    std::fs::create_dir_all(work).map_err(disk)?;
    untar(archive, work).map_err(|e| {
        log::warn!("engines: unpacking {} failed: {e}", archive.display());
        InstallError::Unpack
    })?;
    let mut size = 0;
    for found in binaries.iter().filter_map(|bin| find(work, bin)) {
        size += place(&found, &staged.join(found.file_name().expect("found by file name")))?;
    }
    Ok(size)
}

/// Copies an executable into place, marked executable; returns its size.
fn place(from: &Path, to: &Path) -> Result<u64, InstallError> {
    // Copied, not renamed: the temp dir may be on another volume.
    let size = std::fs::copy(from, to).map_err(disk)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(to, std::fs::Permissions::from_mode(0o755)).map_err(disk)?;
    }
    Ok(size)
}

/// The first regular file named `name` anywhere below `dir`. Symlinks are never followed.
fn find(dir: &Path, name: &str) -> Option<PathBuf> {
    for entry in std::fs::read_dir(dir).ok()?.flatten() {
        let path = entry.path();
        // `DirEntry::file_type` doesn't follow symlinks, so a link is neither a dir nor a file here.
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

/// Unpacks an archive with the OS `tar` (bsdtar on macOS and Windows 10 1803+, which also reads zip).
fn untar(archive: &Path, into: &Path) -> io::Result<()> {
    let program = if cfg!(windows) { "tar.exe" } else { "tar" };
    let status = command(Path::new(program))
        .arg("-xf")
        .arg(archive)
        .arg("-C")
        .arg(into)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()?;
    if status.success() { Ok(()) } else { Err(io::Error::other(format!("tar exited with {status}"))) }
}

/// Moves `staged` to `dest`, keeping the old `dest` until the new one is in place.
fn swap(staged: &Path, dest: &Path, old: &Path) -> Result<(), InstallError> {
    let had = dest.exists();
    if had {
        std::fs::rename(dest, old).map_err(disk)?;
    }
    if let Err(e) = std::fs::rename(staged, dest) {
        if had && let Err(back) = std::fs::rename(old, dest) {
            log::warn!("engines: rolling back the swap into {} failed: {back}", dest.display());
        }
        return Err(disk(e));
    }
    Ok(())
}

/// Where an engine is installed under the app data dir.
fn install_dir(data_dir: &Path, spec: &EngineSpec) -> PathBuf {
    data_dir.join("engines").join(spec.dir_name)
}

// ---- Shell ----

/// One engine's install dir and download state.
struct Slot {
    dir: PathBuf,
    /// `<temp>/vee-<engine>-download-<identifier>`, so a dev build and the installed app don't share it.
    tmp: PathBuf,
    status: Mutex<EngineStatus>,
    /// The last latest release looked up; kept when a later lookup fails.
    latest: Mutex<Option<Release>>,
    /// The running download's cancel flag.
    cancel: Mutex<Option<Arc<AtomicBool>>>,
    /// The running curl, killed on cancel and on app exit.
    curl: Mutex<Option<Child>>,
}

/// Indexed by `EngineId as usize`.
pub struct Engines([Slot; 4]);

impl Engines {
    fn get(&self, id: EngineId) -> &Slot {
        &self.0[id as usize]
    }
}

/// Manages the engine state. Call once in setup, before any window is built.
/// Clears downloads and the work dirs a previous run left (quit mid-install), and restores installs
/// a failed swap left aside.
pub fn manage(app: &AppHandle, data_dir: &Path) {
    let identifier = app.config().identifier.clone();
    let slots = SPECS.each_ref().map(|spec| {
        let dir = install_dir(data_dir, spec);
        let tmp = std::env::temp_dir().join(format!("vee-{}-download-{identifier}", spec.dir_name));
        remove_dir(&tmp);
        recover(&dir, spec.binaries);
        tidy(&dir, spec.binaries);
        let status = Mutex::new(on_disk(spec, &dir, None, None));
        Slot { dir, tmp, status, latest: Mutex::new(None), cancel: Mutex::new(None), curl: Mutex::new(None) }
    });
    app.manage(Engines(slots));
}

/// Stops running downloads and removes their temp dirs. Call on app exit.
pub fn shutdown(app: &AppHandle) {
    let Some(e) = app.try_state::<Engines>() else { return };
    for s in &e.0 {
        if let Some(cancel) = &*s.cancel.lock().unwrap() {
            cancel.store(true, Ordering::SeqCst);
        }
        if let Some(mut child) = s.curl.lock().unwrap().take() {
            stop(&mut child);
        }
        remove_dir(&s.tmp);
    }
}

/// The engine's binaries, in `EngineSpec::binaries` order, when all are installed.
pub fn engine_bin(app: &AppHandle, id: EngineId) -> Option<Vec<PathBuf>> {
    let dir = &app.state::<Engines>().inner().get(id).dir;
    let binaries = id.spec().binaries;
    read_installed(dir, binaries)?;
    Some(binaries.iter().map(|b| dir.join(b)).collect())
}

/// `(ffmpeg, ffprobe)` when both are installed.
pub fn ffmpeg(app: &AppHandle) -> Option<(PathBuf, PathBuf)> {
    let [ffmpeg, ffprobe]: [PathBuf; 2] = engine_bin(app, EngineId::Ffmpeg)?.try_into().ok()?;
    Some((ffmpeg, ffprobe))
}

/// A model engine's install dir, when the model is installed.
pub fn model_dir(app: &AppHandle, id: EngineId) -> Option<PathBuf> {
    engine_bin(app, id)?.pop()?.parent().map(Path::to_path_buf)
}

/// The rhwp CLI when installed.
pub fn rhwp(app: &AppHandle) -> Option<PathBuf> {
    engine_bin(app, EngineId::Rhwp)?.pop()
}

/// What is installed in `dir`, compared with the `latest` release if it is known, with the last
/// download's `error` attached.
pub fn on_disk(spec: &EngineSpec, dir: &Path, latest: Option<&Release>, error: Option<InstallError>) -> EngineStatus {
    match read_installed(dir, spec.binaries) {
        Some(m) => EngineStatus::Installed {
            update: latest.is_some_and(|r| newer(&r.version, &m.version)),
            version: m.version,
            size: m.size,
            installed_at: m.installed_at,
            path: dir.to_string_lossy().into_owned(),
            error,
        },
        None => EngineStatus::Missing { error, download_size: latest.map(Release::size) },
    }
}

/// `a` is a later dotted version than `b` (`10.0` > `9.0.2` > `9.0`); a part that isn't a number counts as 0.
fn newer(a: &str, b: &str) -> bool {
    let parts = |v: &str| v.split('.').map(|p| p.parse::<u64>().unwrap_or(0)).collect::<Vec<_>>();
    let (mut a, mut b) = (parts(a), parts(b));
    let len = a.len().max(b.len());
    a.resize(len, 0);
    b.resize(len, 0);
    a > b
}

/// The status on disk against the last latest release looked up.
fn current(app: &AppHandle, id: EngineId, error: Option<InstallError>) -> EngineStatus {
    let s = app.state::<Engines>().inner().get(id);
    on_disk(id.spec(), &s.dir, s.latest.lock().unwrap().as_ref(), error)
}

/// Keeps a looked-up release (a failed lookup keeps the previous one) and re-publishes the status,
/// unless a download is running: its progress stays on screen.
fn apply_latest(app: &AppHandle, id: EngineId, release: Option<Release>) {
    let s = app.state::<Engines>().inner().get(id);
    if let Some(r) = release {
        *s.latest.lock().unwrap() = Some(r);
    }
    // Held across the `set`, so a download can't start between the check and the publish and be
    // overwritten. Lock order: `cancel`, then `status` / `latest` (nothing takes them the other way).
    let cancel = s.cancel.lock().unwrap();
    if cancel.is_none() {
        // Keeps the last download's error on screen until the next download.
        let error = match &*s.status.lock().unwrap() {
            EngineStatus::Missing { error, .. } | EngineStatus::Installed { error, .. } => *error,
            EngineStatus::Downloading { .. } => None,
        };
        set(app, id, current(app, id, error));
    }
}

/// Set while a lookup runs, so remounting the Convert tab doesn't start another one beside it.
static CHECKING: AtomicBool = AtomicBool::new(false);

/// Holds `CHECKING` for one lookup; `None` when one is already running.
struct Checking;

impl Checking {
    fn start() -> Option<Self> {
        (!CHECKING.swap(true, Ordering::SeqCst)).then_some(Checking)
    }
}

impl Drop for Checking {
    fn drop(&mut self) {
        CHECKING.store(false, Ordering::SeqCst);
    }
}

fn set(app: &AppHandle, id: EngineId, status: EngineStatus) {
    *app.state::<Engines>().get(id).status.lock().unwrap() = status.clone();
    let _ = app.emit(EVENT, StatusEvent { engine: id, status });
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

/// Starts downloading the engine's latest release in the background (looked up first if no lookup
/// has succeeded yet). A second call for the same engine while one runs does nothing; different
/// engines download side by side.
fn download(app: &AppHandle, id: EngineId) {
    let e = app.state::<Engines>();
    let cancel = {
        let mut slot = e.get(id).cancel.lock().unwrap();
        if slot.is_some() {
            return;
        }
        let cancel = Arc::new(AtomicBool::new(false));
        *slot = Some(cancel.clone());
        cancel
    };
    let known = e.get(id).latest.lock().unwrap().clone();
    let total = known.as_ref().map_or(0, Release::size);
    set(app, id, EngineStatus::Downloading { received: 0, total, speed: 0, stage: Stage::Download });
    let app = app.clone();
    std::thread::spawn(move || {
        let e = app.state::<Engines>();
        let s = e.get(id);
        let release = known.or_else(|| {
            let r = resolve(id);
            // A failed lookup keeps whatever a check stored meanwhile.
            if let Some(r) = &r {
                *s.latest.lock().unwrap() = Some(r.clone());
            }
            r
        });
        let result = match &release {
            Some(r) => fetch_and_install(&app, id, r, &cancel),
            None => Err(Stop::Failed(InstallError::Network)),
        };
        remove_dir(&s.tmp);
        let error = match result {
            Err(Stop::Failed(reason)) => Some(reason),
            Ok(()) | Err(Stop::Cancelled) => None,
        };
        // Published before `cancel` clears, so a lookup finishing now sees the download still running
        // and can't replace this status (and its error) with one read mid-download.
        let mut cancel = s.cancel.lock().unwrap();
        set(&app, id, current(&app, id, error));
        *cancel = None;
    });
}

fn fetch_and_install(app: &AppHandle, id: EngineId, release: &Release, cancel: &AtomicBool) -> Result<(), Stop> {
    let spec = id.spec();
    let s = app.state::<Engines>().inner().get(id);
    remove_dir(&s.tmp);
    std::fs::create_dir_all(&s.tmp).map_err(disk)?;
    let total = release.size();
    let started = Instant::now();
    let mut done = 0;
    let mut files = Vec::new();
    for (i, asset) in release.assets.iter().enumerate() {
        let file = s.tmp.join(format!("{i}.download"));
        curl(app, id, &asset.url, &file, cancel, |got| {
            let received = done + got;
            let secs = started.elapsed().as_secs_f64();
            let speed = if secs > 0.0 { (received as f64 / secs) as u64 } else { 0 };
            EngineStatus::Downloading { received, total, speed, stage: Stage::Download }
        })?;
        done += asset.size;
        files.push((file, asset));
    }
    set(app, id, EngineStatus::Downloading { received: total, total, speed: 0, stage: Stage::Verify });
    let on_unpack = || set(app, id, EngineStatus::Downloading { received: total, total, speed: 0, stage: Stage::Unpack });
    install_from(spec.binaries, &files, &s.dir, &release.version, now_ms(), &on_unpack)?;
    // Downloaded by curl, so no quarantine flag is expected; cleared in case one was added.
    #[cfg(target_os = "macos")]
    let _ = std::process::Command::new("xattr").arg("-dr").arg("com.apple.quarantine").arg(&s.dir).status();
    Ok(())
}

/// The curl arguments before `<file> <url>`; a stalled transfer fails instead of hanging.
const CURL_ARGS: [&str; 14] = [
    "-fL",
    "--proto",
    "=https",
    "--proto-redir",
    "=https",
    "--retry",
    "2",
    "--connect-timeout",
    "20",
    "--speed-time",
    "60",
    "--speed-limit",
    "1024",
    "-o",
];

fn stop(child: &mut Child) {
    let _ = child.kill();
    let _ = child.wait();
}

/// Runs curl into `file`, publishing `progress(bytes so far)` every poll.
fn curl(app: &AppHandle, id: EngineId, url: &str, file: &Path, cancel: &AtomicBool, progress: impl Fn(u64) -> EngineStatus) -> Result<(), Stop> {
    let slot = &app.state::<Engines>().inner().get(id).curl;
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
                set(app, id, progress(std::fs::metadata(file).map(|m| m.len()).unwrap_or(0)));
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

/// Engine commands; `engine` is `"ffmpeg"`, `"rhwp"`, `"bgremove"` or `"sam"`.
pub mod commands {
    use super::{EngineId, EngineStatus, Engines};
    use crate::shared::error::{AppError, AppResult};
    use std::sync::atomic::Ordering;
    use tauri::{AppHandle, Manager, State};

    #[tauri::command]
    pub fn get_engine_status(state: State<Engines>, engine: String) -> AppResult<EngineStatus> {
        Ok(state.get(EngineId::parse(&engine)?).status.lock().unwrap().clone())
    }

    /// Looks up every engine's latest release and re-publishes their status (`update`, `downloadSize`)
    /// as `engines://status`. Run when the Convert tab opens; a failed lookup changes nothing, and a call
    /// while one runs returns at once.
    #[tauri::command(async)]
    pub fn check_engine_updates(app: AppHandle) {
        let Some(_checking) = super::Checking::start() else { return };
        std::thread::scope(|scope| {
            for id in EngineId::ALL {
                let app = &app;
                scope.spawn(move || super::apply_latest(app, id, super::resolve(id)));
            }
        });
    }

    /// Downloads (or updates to) the engine's latest release. Progress arrives as `engines://status`.
    #[tauri::command]
    pub fn download_engine(app: AppHandle, engine: String) -> AppResult<()> {
        super::download(&app, EngineId::parse(&engine)?);
        Ok(())
    }

    /// Stops the running download; the status goes back to what is on disk, without an error.
    #[tauri::command]
    pub fn cancel_engine_download(state: State<Engines>, engine: String) -> AppResult<()> {
        if let Some(cancel) = &*state.get(EngineId::parse(&engine)?).cancel.lock().unwrap() {
            cancel.store(true, Ordering::SeqCst);
        }
        Ok(())
    }

    // ponytail: a job already running keeps its open binaries (unix) or fails the removal (Windows).
    #[tauri::command]
    pub fn remove_engine(app: AppHandle, engine: String) -> AppResult<()> {
        let id = EngineId::parse(&engine)?;
        let e = app.state::<Engines>();
        let s = e.get(id);
        if s.cancel.lock().unwrap().is_some() {
            return Err(AppError::InvalidInput(format!("{engine} is downloading")));
        }
        crate::app::ai::forget(id)?;
        let removed = if s.dir.exists() { std::fs::remove_dir_all(&s.dir) } else { Ok(()) };
        // A partial removal may leave a broken dir: report what is really there.
        super::set(&app, id, super::current(&app, id, None));
        Ok(removed?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const ABC: &str = "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad";
    const FFMPEG: &[&str] = SPECS[0].binaries;
    const RHWP: &[&str] = SPECS[1].binaries;
    const BGREMOVE: &[&str] = SPECS[2].binaries;
    const NONE: &dyn Fn() = &|| ();

    /// One downloaded file per FFmpeg binary, each holding `abc`.
    fn setup() -> (tempfile::TempDir, [PathBuf; 2]) {
        let root = tempfile::tempdir().unwrap();
        let files = [0, 1].map(|i| root.path().join(format!("{}-download", FFMPEG[i])));
        for f in &files {
            std::fs::write(f, b"abc").unwrap();
        }
        (root, files)
    }

    /// A raw asset per FFmpeg binary with these hashes.
    fn assets_with(sha256: [&'static str; 2]) -> [Asset; 2] {
        [0, 1].map(|i| Asset { url: "test://dl".into(), sha256: sha256[i].into(), size: 3, kind: AssetKind::Raw { name: FFMPEG[i] } })
    }

    fn pairs<'a>(files: &[PathBuf; 2], assets: &'a [Asset; 2]) -> Vec<(PathBuf, &'a Asset)> {
        files.iter().cloned().zip(assets.iter()).collect()
    }

    fn existing(root: &Path) -> PathBuf {
        let dest = root.join("engines/ffmpeg");
        std::fs::create_dir_all(&dest).unwrap();
        std::fs::write(dest.join("keep"), b"old").unwrap();
        dest
    }

    /// A real tar.gz in `<root>/dl/0.download` of `<root>/fixture`, filled by `fill`, and its archive asset.
    fn archive(root: &Path, fill: impl Fn(&Path)) -> (PathBuf, Asset) {
        let fixture = root.join("fixture");
        std::fs::create_dir_all(fixture.join("rhwp")).unwrap();
        fill(&fixture);
        std::fs::create_dir_all(root.join("dl")).unwrap();
        let file = root.join("dl/0.download");
        let ok = std::process::Command::new("tar").arg("-czf").arg(&file).arg("-C").arg(&fixture).arg(".").status().unwrap();
        assert!(ok.success());
        let sha256 = sha256_file(&file).unwrap();
        (file, Asset { url: "test://a.tar.gz".into(), sha256, size: 3, kind: AssetKind::Archive })
    }

    fn with_rhwp(fixture: &Path) {
        std::fs::write(fixture.join("rhwp").join(RHWP[0]), b"rhwp-bin").unwrap();
        std::fs::write(fixture.join("rhwp/README.md"), b"docs").unwrap();
    }

    #[test]
    fn hashes_files_as_lowercase_hex() {
        let (_root, files) = setup();
        assert_eq!(sha256_file(&files[0]).unwrap(), ABC);
    }

    #[test]
    fn installs_binaries_and_meta_over_an_old_install() {
        let (root, files) = setup();
        let dest = existing(root.path());
        let a = assets_with([ABC, ABC]);
        install_from(FFMPEG, &pairs(&files, &a), &dest, "9.0.2", 42, NONE).unwrap();
        assert!(FFMPEG.iter().all(|b| dest.join(b).is_file()));
        assert!(!dest.join("keep").exists());
        assert_eq!(read_installed(&dest, FFMPEG), Some(Meta { version: "9.0.2".into(), installed_at: 42, size: 6 }));
        // Only the install dir is left next to it.
        assert_eq!(std::fs::read_dir(dest.parent().unwrap()).unwrap().count(), 1);
        assert_eq!(std::fs::read(dest.join(FFMPEG[1])).unwrap(), b"abc");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(std::fs::metadata(dest.join(FFMPEG[0])).unwrap().permissions().mode() & 0o777, 0o755);
        }
    }

    #[test]
    fn installs_a_binary_found_in_an_archive() {
        let root = tempfile::tempdir().unwrap();
        let (file, a) = archive(root.path(), with_rhwp);
        let dest = root.path().join("engines/rhwp");
        std::fs::create_dir_all(&dest).unwrap();
        std::fs::write(dest.join("keep"), b"old").unwrap();
        let unpacked = std::cell::Cell::new(0);
        install_from(RHWP, &[(file, &a)], &dest, "0.8.7", 42, &|| unpacked.set(unpacked.get() + 1)).unwrap();
        assert_eq!(unpacked.get(), 1);
        assert_eq!(std::fs::read(dest.join(RHWP[0])).unwrap(), b"rhwp-bin");
        // Only the binary and meta.json: the archive's other files stay behind.
        assert_eq!(std::fs::read_dir(&dest).unwrap().count(), 2);
        assert_eq!(read_installed(&dest, RHWP), Some(Meta { version: "0.8.7".into(), installed_at: 42, size: 8 }));
        assert_eq!(std::fs::read_dir(dest.parent().unwrap()).unwrap().count(), 1);
        // The unpack work dir next to the download is gone.
        assert_eq!(std::fs::read_dir(root.path().join("dl")).unwrap().count(), 1);
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(std::fs::metadata(dest.join(RHWP[0])).unwrap().permissions().mode() & 0o777, 0o755);
        }
    }

    #[test]
    fn installs_one_binary_from_each_archive_and_needs_them_all() {
        let root = tempfile::tempdir().unwrap();
        let [(f0, a0), (f1, a1)] = [0, 1].map(|i| archive(&root.path().join(i.to_string()), |f| std::fs::write(f.join(FFMPEG[i]), FFMPEG[i]).unwrap()));
        let dest = existing(root.path());
        assert_eq!(install_from(FFMPEG, &[(f0.clone(), &a0)], &dest, "9.0.2", 1, NONE), Err(InstallError::Unpack));
        assert!(dest.join("keep").is_file());
        install_from(FFMPEG, &[(f0, &a0), (f1, &a1)], &dest, "9.0.2", 1, NONE).unwrap();
        assert!(FFMPEG.iter().all(|b| std::fs::read(dest.join(b)).unwrap() == b.as_bytes()));
    }

    #[test]
    fn an_archive_without_the_binary_leaves_the_old_install() {
        let root = tempfile::tempdir().unwrap();
        let (file, a) = archive(root.path(), |f| std::fs::write(f.join("rhwp/README.md"), b"docs").unwrap());
        let dest = existing(root.path());
        assert_eq!(install_from(RHWP, &[(file, &a)], &dest, "0.8.7", 1, NONE), Err(InstallError::Unpack));
        assert!(dest.join("keep").exists());
        assert_eq!(std::fs::read_dir(dest.parent().unwrap()).unwrap().count(), 1);
        assert_eq!(std::fs::read_dir(root.path().join("dl")).unwrap().count(), 1);
    }

    #[test]
    fn a_file_tar_cannot_read_is_an_unpack_error() {
        let root = tempfile::tempdir().unwrap();
        let dest = existing(root.path());
        let file = root.path().join("0.download");
        std::fs::write(&file, b"abc").unwrap();
        let a = Asset { url: "test://a.tar.gz".into(), sha256: ABC.into(), size: 3, kind: AssetKind::Archive };
        assert_eq!(install_from(RHWP, &[(file, &a)], &dest, "0.8.7", 1, NONE), Err(InstallError::Unpack));
        assert!(dest.join("keep").exists());
    }

    #[cfg(unix)]
    #[test]
    fn a_symlink_in_an_archive_is_not_followed() {
        let root = tempfile::tempdir().unwrap();
        let (file, a) = archive(root.path(), |f| std::os::unix::fs::symlink("/bin/sh", f.join("rhwp/rhwp")).unwrap());
        let dest = root.path().join("engines/rhwp");
        assert_eq!(install_from(RHWP, &[(file, &a)], &dest, "0.8.7", 1, NONE), Err(InstallError::Unpack));
        assert!(!dest.exists());
    }

    #[test]
    fn a_wrong_hash_leaves_the_old_install() {
        let (root, files) = setup();
        let dest = existing(root.path());
        // Only the last file is bad: nothing is copied before every hash checks out.
        let a = assets_with([ABC, "00"]);
        assert_eq!(install_from(FFMPEG, &pairs(&files, &a), &dest, "9.0.2", 1, NONE), Err(InstallError::Checksum));
        assert!(dest.join("keep").exists());
        assert_eq!(std::fs::read_dir(dest.parent().unwrap()).unwrap().count(), 1);
    }

    #[test]
    fn a_missing_download_leaves_the_old_install() {
        let (root, files) = setup();
        let dest = existing(root.path());
        std::fs::remove_file(&files[1]).unwrap();
        let a = assets_with([ABC, ABC]);
        assert_eq!(install_from(FFMPEG, &pairs(&files, &a), &dest, "9.0.2", 1, NONE), Err(InstallError::Disk));
        assert!(dest.join("keep").exists());
        assert_eq!(std::fs::read_dir(dest.parent().unwrap()).unwrap().count(), 1);
    }

    #[test]
    fn a_dir_without_ffprobe_is_not_installed() {
        let root = tempfile::tempdir().unwrap();
        std::fs::write(root.path().join(FFMPEG[0]), b"bin").unwrap();
        std::fs::write(root.path().join("meta.json"), br#"{"version":"9.0.2","installedAt":1,"size":3}"#).unwrap();
        assert_eq!(read_installed(root.path(), FFMPEG), None);
    }

    #[test]
    fn status_serializes_for_the_settings_tab() {
        let json = |s: EngineStatus| serde_json::to_value(s).unwrap();
        assert_eq!(
            json(EngineStatus::Missing { error: None, download_size: Some(9) }),
            serde_json::json!({ "state": "missing", "error": null, "downloadSize": 9 })
        );
        assert_eq!(
            json(EngineStatus::Missing { error: Some(InstallError::Unpack), download_size: Some(9) }),
            serde_json::json!({ "state": "missing", "error": "unpack", "downloadSize": 9 })
        );
        assert_eq!(
            json(EngineStatus::Downloading { received: 1, total: 2, speed: 3, stage: Stage::Unpack }),
            serde_json::json!({ "state": "downloading", "received": 1, "total": 2, "speed": 3, "stage": "unpack" })
        );
        assert_eq!(
            json(EngineStatus::Installed {
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

    #[test]
    fn the_status_event_names_its_engine() {
        let root = tempfile::tempdir().unwrap();
        let spec = EngineId::Rhwp.spec();
        let event = StatusEvent { engine: EngineId::Rhwp, status: on_disk(spec, root.path(), None, None) };
        assert_eq!(
            serde_json::to_value(event).unwrap(),
            serde_json::json!({ "engine": "rhwp", "status": { "state": "missing", "error": null, "downloadSize": null } })
        );
    }

    #[test]
    fn engine_ids_parse_their_serde_names() {
        for id in EngineId::ALL {
            let name = serde_json::to_value(id).unwrap();
            assert_eq!(name, id.spec().dir_name);
            assert_eq!(EngineId::parse(name.as_str().unwrap()).unwrap(), id);
        }
        assert!(matches!(EngineId::parse("libreoffice"), Err(AppError::InvalidInput(_))));
    }

    /// A complete install of `version` in `dir`.
    fn installed(dir: &Path, binaries: &[&str], version: &str) {
        std::fs::create_dir_all(dir).unwrap();
        for b in binaries {
            std::fs::write(dir.join(b), b"bin").unwrap();
        }
        let meta = Meta { version: version.into(), installed_at: 7, size: 6 };
        std::fs::write(dir.join("meta.json"), serde_json::to_vec(&meta).unwrap()).unwrap();
    }

    #[test]
    fn engines_install_side_by_side() {
        let root = tempfile::tempdir().unwrap();
        let [ffmpeg, rhwp, ..] = SPECS.each_ref().map(|s| install_dir(root.path(), s));
        assert_ne!(ffmpeg, rhwp);
        installed(&ffmpeg, FFMPEG, "8.1.2");
        installed(&rhwp, RHWP, "0.8.7");
        // What `remove_engine` does for one engine.
        std::fs::remove_dir_all(&ffmpeg).unwrap();
        assert!(read_installed(&ffmpeg, FFMPEG).is_none());
        assert_eq!(read_installed(&rhwp, RHWP).map(|m| m.version), Some("0.8.7".into()));
        // Startup tidying of one engine leaves the other alone.
        recover(&ffmpeg, FFMPEG);
        tidy(&ffmpeg, FFMPEG);
        assert!(read_installed(&rhwp, RHWP).is_some());
    }

    #[test]
    fn a_failed_update_keeps_the_old_install_with_the_error() {
        let root = tempfile::tempdir().unwrap();
        let dest = root.path().join("ffmpeg");
        installed(&dest, FFMPEG, "9.0.1");
        let (_r, files) = setup();
        let a = assets_with(["00", "00"]);
        let err = install_from(FFMPEG, &pairs(&files, &a), &dest, "9.0.2", 1, NONE).unwrap_err();
        let spec = EngineId::Ffmpeg.spec();
        let status = on_disk(spec, &dest, Some(&release("9.0.2", 10)), Some(err));
        assert!(matches!(
            status,
            EngineStatus::Installed { ref version, update: true, error: Some(InstallError::Checksum), .. } if version == "9.0.1"
        ));
        assert_eq!(
            on_disk(spec, &root.path().join("none"), None, Some(InstallError::Network)),
            EngineStatus::Missing { error: Some(InstallError::Network), download_size: None }
        );
    }

    #[test]
    fn an_old_install_left_by_a_failed_rollback_is_restored() {
        let root = tempfile::tempdir().unwrap();
        let dest = root.path().join("ffmpeg");
        let old = root.path().join(".ffmpeg-old");
        installed(&old, FFMPEG, "9.0.1");
        // A broken leftover where the install should be.
        std::fs::create_dir_all(&dest).unwrap();
        recover(&dest, FFMPEG);
        assert_eq!(read_installed(&dest, FFMPEG).map(|m| m.version), Some("9.0.1".into()));
        assert!(!old.exists());
        // A newer install next to a stale old one: nothing moves.
        installed(&old, FFMPEG, "9.0.0");
        recover(&dest, FFMPEG);
        assert_eq!(read_installed(&dest, FFMPEG).map(|m| m.version), Some("9.0.1".into()));
    }

    #[test]
    fn startup_clears_work_dirs_a_quit_mid_install_left() {
        let root = tempfile::tempdir().unwrap();
        let dest = root.path().join("ffmpeg");
        installed(&dest, FFMPEG, "9.0.2");
        std::fs::create_dir_all(root.path().join(".ffmpeg-new")).unwrap();
        // Quit after the second rename: the old install is no longer needed.
        installed(&root.path().join(".ffmpeg-old"), FFMPEG, "9.0.1");
        recover(&dest, FFMPEG);
        tidy(&dest, FFMPEG);
        assert_eq!(std::fs::read_dir(root.path()).unwrap().count(), 1);
        assert_eq!(read_installed(&dest, FFMPEG).map(|m| m.version), Some("9.0.2".into()));
        // A stranded old install survives (restored by `recover` first, or kept if that fails).
        installed(&root.path().join(".ffmpeg-old"), FFMPEG, "9.0.1");
        std::fs::remove_dir_all(&dest).unwrap();
        tidy(&dest, FFMPEG);
        assert!(read_installed(&root.path().join(".ffmpeg-old"), FFMPEG).is_some());
    }

    /// A release of `version` whose one file is `size` bytes.
    fn release(version: &str, size: u64) -> Release {
        Release {
            version: version.into(),
            assets: vec![Asset { url: "test://dl".into(), sha256: ABC.into(), size, kind: AssetKind::Archive }],
        }
    }

    #[test]
    fn update_and_size_come_from_the_latest_release() {
        let root = tempfile::tempdir().unwrap();
        let spec = EngineId::Ffmpeg.spec();
        let dest = root.path().join("ffmpeg");
        let update = |latest: Option<&Release>| match on_disk(spec, &dest, latest, None) {
            EngineStatus::Installed { update, .. } => update,
            s => panic!("{s:?}"),
        };
        assert_eq!(on_disk(spec, &dest, None, None), EngineStatus::Missing { error: None, download_size: None });
        assert_eq!(on_disk(spec, &dest, Some(&release("9.0.2", 10)), None), EngineStatus::Missing { error: None, download_size: Some(10) });
        installed(&dest, FFMPEG, "9.0.1");
        assert!(update(Some(&release("9.0.2", 10))));
        assert!(update(Some(&release("10.0", 10))));
        assert!(!update(Some(&release("9.0.1", 10))));
        // A "latest" older than the install never offers a downgrade.
        assert!(!update(Some(&release("9.0", 10))));
        assert!(!update(Some(&release("8.1.2", 10))));
        assert!(!update(None));
    }

    #[test]
    fn install_from_restores_a_stranded_old_install_first() {
        let root = tempfile::tempdir().unwrap();
        let dest = root.path().join("ffmpeg");
        installed(&root.path().join(".ffmpeg-old"), FFMPEG, "9.0.1");
        let (_r, files) = setup();
        let a = assets_with(["00", "00"]);
        assert_eq!(install_from(FFMPEG, &pairs(&files, &a), &dest, "9.0.2", 1, NONE), Err(InstallError::Checksum));
        assert_eq!(read_installed(&dest, FFMPEG).map(|m| m.version), Some("9.0.1".into()));
    }

    const RHWP_JSON: &str = r#"{"tag_name":"v0.8.7","assets":[{"name":"rhwp-v0.8.7-macos-aarch64.tar.gz","size":12941874,
      "browser_download_url":"https://github.com/edwardkim/rhwp/releases/download/v0.8.7/rhwp-v0.8.7-macos-aarch64.tar.gz",
      "digest":"sha256:6ed46aa63f6d10c447842999f73abc59cf9586c5645fefc5e763bd6a6d43662d"}]}"#;

    #[test]
    fn rhwp_latest_release_from_the_github_api() {
        let r = parse_rhwp(RHWP_JSON, "macos-aarch64.tar.gz").unwrap();
        assert_eq!((r.version.as_str(), r.size()), ("0.8.7", 12_941_874));
        assert_eq!(r.assets[0].sha256, "6ed46aa63f6d10c447842999f73abc59cf9586c5645fefc5e763bd6a6d43662d");
        assert!(r.valid());
        let no_digest = RHWP_JSON.replace(r#""digest""#, r#""label""#);
        assert_eq!(parse_rhwp(&no_digest, "macos-aarch64.tar.gz"), None);
        assert_eq!(parse_rhwp(RHWP_JSON, "windows-x86_64.zip"), None);
        assert_eq!(parse_rhwp("", "macos-aarch64.tar.gz"), None);
    }

    #[test]
    fn gyan_skips_git_builds_for_the_newest_release_version() {
        let asset = |tag: &str| {
            format!(
                r#"{{"tag_name":"{tag}","assets":[{{"name":"ffmpeg-{tag}-essentials_build.zip","size":5,
                "browser_download_url":"https://github.com/GyanD/codexffmpeg/releases/download/{tag}/ffmpeg-{tag}-essentials_build.zip",
                "digest":"sha256:60f467265b1e312373dbcd92200c2618a74850f98d3d078e94296bb3fa2047ba"}}]}}"#
            )
        };
        let json = format!("[{},{},{}]", asset("2026-10-22-git-bf0e831606"), asset("9.0.2"), asset("9.0.1"));
        let r = parse_gyan(&json).unwrap();
        assert_eq!(r.version, "9.0.2");
        assert!(r.assets[0].url.ends_with("/9.0.2/ffmpeg-9.0.2-essentials_build.zip"));
        assert!(r.valid());
        assert_eq!(parse_gyan(&format!("[{}]", asset("2026-10-22-git-bf0e831606"))), None);
        assert_eq!(parse_gyan("{}"), None);
    }

    #[test]
    fn riedl_redirect_and_checksum_files() {
        assert_eq!(
            parse_riedl_url("https://ffmpeg.martin-riedl.de/download/macos/arm64/1789931890_9.0.2/ffmpeg.zip").as_deref(),
            Some("9.0.2")
        );
        assert_eq!(parse_riedl_url("https://ffmpeg.martin-riedl.de/redirect/latest/macos/arm64/release/ffmpeg.zip"), None);
        assert_eq!(parse_riedl_url(""), None);
        let sha = "c8ed4c4e6978a03c485edbfe4e0a5dc2380f8a30bba5150531b31b094492d924";
        assert_eq!(parse_sha256_file(&format!("{sha}  ffmpeg.zip\n")).as_deref(), Some(sha));
        assert_eq!(parse_sha256_file("<html>not found</html>"), None);
        assert_eq!(
            parse_content_length("HTTP/2 302\r\ncontent-length: 19\r\n\r\nHTTP/2 200\r\nContent-Length: 28395699\r\n"),
            Some(28_395_699)
        );
        assert_eq!(parse_content_length("HTTP/2 200\r\n"), None);
    }

    #[test]
    fn model_releases_are_valid() {
        assert!(pinned(EngineId::Bgremove).unwrap().valid());
        assert_eq!(pinned(EngineId::Bgremove).unwrap().version, "2");
        assert_eq!(pinned(EngineId::Ffmpeg), None);
    }

    #[test]
    fn model_release_is_the_model_alone() {
        let r = pinned(EngineId::Bgremove).unwrap();
        let kinds: Vec<_> = r.assets.iter().map(|a| a.kind.clone()).collect();
        assert_eq!(kinds, [AssetKind::Raw { name: "model.onnx" }]);
        assert_eq!(BGREMOVE, ["model.onnx"]);
        assert!(!SOURCES.iter().any(|s| s.contains("onnxruntime")));
    }

    #[test]
    fn engine_ids_round_trip() {
        assert_eq!(EngineId::parse("bgremove").unwrap(), EngineId::Bgremove);
        assert_eq!(EngineId::parse("sam").unwrap(), EngineId::Sam);
    }

    #[test]
    #[cfg(any(target_os = "macos", windows))]
    fn sam_release_is_encoder_and_decoder() {
        let r = pinned(EngineId::Sam).unwrap();
        assert!(r.valid());
        assert_eq!(r.assets.len(), 2);
        assert_eq!(r.size(), 108_836_115 + 8_755_366);
        let kinds: Vec<_> = r.assets.iter().map(|a| a.kind.clone()).collect();
        assert_eq!(kinds, [AssetKind::Raw { name: "encoder.onnx" }, AssetKind::Raw { name: "decoder.onnx" }]);
    }

    #[test]
    fn an_install_with_the_old_runtime_is_installed_and_updates() {
        let root = tempfile::tempdir().unwrap();
        let model = root.path().join("1.download");
        std::fs::write(&model, b"abc").unwrap();
        let m = Asset { url: "test://m".into(), sha256: ABC.into(), size: 3, kind: AssetKind::Raw { name: BGREMOVE[0] } };
        let dest = root.path().join("engines/bgremove");
        install_from(BGREMOVE, &[(model, &m)], &dest, "1", 1, NONE).unwrap();
        // Version 1 also held the runtime here.
        std::fs::write(dest.join("libonnxruntime.dylib"), b"ort").unwrap();
        let latest = pinned(EngineId::Bgremove).unwrap();
        assert!(matches!(on_disk(&SPECS[2], &dest, Some(&latest), None), EngineStatus::Installed { update: true, .. }));
        // The update replaces the whole dir, runtime included.
        let model = root.path().join("2.download");
        std::fs::write(&model, b"abc").unwrap();
        install_from(BGREMOVE, &[(model, &m)], &dest, &latest.version, 2, NONE).unwrap();
        assert!(!dest.join("libonnxruntime.dylib").exists());
        assert!(matches!(on_disk(&SPECS[2], &dest, Some(&latest), None), EngineStatus::Installed { update: false, .. }));
    }

    #[test]
    fn a_release_from_elsewhere_or_with_a_bad_version_is_invalid() {
        let ok = parse_rhwp(RHWP_JSON, "macos-aarch64.tar.gz").unwrap();
        let with = |f: &dyn Fn(&mut Release)| {
            let mut r = ok.clone();
            f(&mut r);
            r.valid()
        };
        assert!(!with(&|r| r.assets[0].url = r.assets[0].url.replace("https://github.com/", "http://github.com/")));
        assert!(!with(&|r| r.assets[0].url = "https://evil.example/rhwp.tar.gz".into()));
        assert!(!with(&|r| r.assets[0].url = "https://github.com/someone/else/releases/download/v1/rhwp.tar.gz".into()));
        assert!(!with(&|r| r.version = "9.0-dev".into()));
        assert!(!with(&|r| r.version = String::new()));
        assert!(!with(&|r| r.assets[0].sha256 = "abc".into()));
        assert!(!with(&|r| r.assets.clear()));
    }

    #[test]
    fn only_one_lookup_runs_at_a_time() {
        let first = Checking::start();
        assert!(first.is_some());
        assert!(Checking::start().is_none());
        drop(first);
        assert!(Checking::start().is_some());
    }

    #[test]
    fn curl_gives_up_on_a_stalled_download() {
        for w in [["--connect-timeout", "20"], ["--speed-time", "60"], ["--speed-limit", "1024"]] {
            assert!(CURL_ARGS.windows(2).any(|p| p == w), "{w:?}");
        }
        assert_eq!(CURL_ARGS.last(), Some(&"-o"));
        // Never handed to plain http, not even by a redirect.
        for w in [["--proto", "=https"], ["--proto-redir", "=https"]] {
            assert!(CURL_ARGS.windows(2).any(|p| p == w), "{w:?}");
            assert!(LOOKUP_ARGS.windows(2).any(|p| p == w), "{w:?}");
        }
    }

    /// Looks up the latest release and downloads it for real into a temp dir.
    fn install_latest(id: EngineId) -> (tempfile::TempDir, PathBuf) {
        let spec = id.spec();
        let release = resolve(id).expect("latest release");
        println!("latest {}: {}", spec.dir_name, release.version);
        let root = tempfile::tempdir().unwrap();
        let started = Instant::now();
        let mut files = Vec::new();
        for (i, a) in release.assets.iter().enumerate() {
            let file = root.path().join(format!("{i}.download"));
            let ok = std::process::Command::new("curl").args(CURL_ARGS).arg(&file).arg(&a.url).status().unwrap();
            assert!(ok.success(), "curl {}", a.url);
            assert_eq!(std::fs::metadata(&file).unwrap().len(), a.size);
            files.push((file, a));
        }
        println!("downloaded in {:.1}s", started.elapsed().as_secs_f64());
        let dest = install_dir(root.path(), spec);
        install_from(spec.binaries, &files, &dest, &release.version, now_ms(), NONE).unwrap();
        println!("{:?}", read_installed(&dest, spec.binaries).unwrap());
        (root, dest)
    }

    /// `cargo test --lib installs_latest_ffmpeg -- --ignored --nocapture`
    #[test]
    #[ignore]
    fn installs_latest_ffmpeg() {
        let (_root, dest) = install_latest(EngineId::Ffmpeg);
        let out = std::process::Command::new(dest.join(FFMPEG[0])).arg("-version").output().unwrap();
        println!("{}", String::from_utf8_lossy(&out.stdout).lines().next().unwrap_or_default());
        assert!(out.status.success());
        let out = std::process::Command::new(dest.join(FFMPEG[1])).arg("-version").output().unwrap();
        assert!(out.status.success());
    }

    /// Downloads the pinned model (~224 MB) and loads it with the vendored runtime (run
    /// `node scripts/fetch-onnxruntime.mjs` first):
    /// `cargo test --lib installs_pinned_bgremove -- --ignored --nocapture`
    #[test]
    #[ignore]
    fn installs_pinned_bgremove() {
        let (_root, dest) = install_latest(EngineId::Bgremove);
        let env = ort::init_from(crate::app::ai::vendor_runtime()).expect("load ORT").build().unwrap();
        let session = ort::session::Session::builder(&env).unwrap().commit_from_file(dest.join(BGREMOVE[0])).unwrap();
        assert_eq!(session.inputs().len(), 1);
    }

    /// `cargo test --lib installs_latest_rhwp -- --ignored --nocapture`
    #[test]
    #[ignore]
    fn installs_latest_rhwp() {
        let (_root, dest) = install_latest(EngineId::Rhwp);
        let out = std::process::Command::new(dest.join(RHWP[0])).arg("--help").output().unwrap();
        println!("{}", String::from_utf8_lossy(&out.stdout).lines().next().unwrap_or_default());
        assert!(out.status.success());
    }
}
