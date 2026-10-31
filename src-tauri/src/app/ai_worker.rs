//! Runs model inference in a child process (`vee-app --ai-worker`), so the GBs ONNX Runtime
//! allocates go back to the OS when the child exits (macOS keeps freed large blocks resident
//! in-process). The child is spawned on the first request and killed on editor close, app exit
//! and engine removal.
//!
//! Wire format, both ways over stdin/stdout: a `u32` LE header length, the JSON header, then
//! `header.bytes` raw bytes. Images are RGBA8 with `w`/`h` in the header. Model files are sent as
//! absolute paths in each request (only the main process can resolve them through the `AppHandle`).

use crate::domain::edit::{MAX_SIDE, SubjectPoint};
use crate::shared::error::{AppError, AppResult};
use ::image::{DynamicImage, RgbaImage};
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use std::borrow::Cow;
use std::io::{self, BufReader, BufWriter, Read, Write};
use std::path::PathBuf;
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};
use std::sync::{Mutex, PoisonError, TryLockError};

/// The CLI flag that starts the worker instead of the app.
pub const FLAG: &str = "--ai-worker";

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum Request {
    RemoveBg { model: PathBuf, w: u32, h: u32 },
    /// Re-embeds only when `key` differs from the held embedding's.
    Subject {
        encoder: PathBuf,
        decoder: PathBuf,
        key: u64,
        points: Vec<SubjectPoint>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        lasso: Option<Vec<[u32; 2]>>,
        w: u32,
        h: u32,
    },
}

impl Request {
    fn size(&self) -> (u32, u32) {
        match self {
            Request::RemoveBg { w, h, .. } | Request::Subject { w, h, .. } => (*w, *h),
        }
    }
}

/// Largest JSON header a frame may carry.
const MAX_HEADER: usize = 64 * 1024;

/// A message body that says how many payload bytes follow it, so a frame is checked before
/// its payload is allocated.
trait Body {
    fn payload_len(&self) -> io::Result<usize>;
}

/// `w × h × 4`, or an error when a side is over `MAX_SIDE`.
fn rgba_len(w: u32, h: u32) -> io::Result<usize> {
    if w > MAX_SIDE || h > MAX_SIDE {
        return Err(io::Error::other(format!("image {w}×{h} is over {MAX_SIDE} px")));
    }
    Ok(w as usize * h as usize * 4)
}

impl Body for Request {
    fn payload_len(&self) -> io::Result<usize> {
        let (w, h) = self.size();
        rgba_len(w, h)
    }
}

impl Body for Response {
    fn payload_len(&self) -> io::Result<usize> {
        match self {
            Response::Ok { w, h } => rgba_len(*w, *h),
            Response::Err { .. } => Ok(0),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum Response {
    Ok { w: u32, h: u32 },
    /// `invalid` maps to `AppError::InvalidInput`, otherwise `Other`.
    Err { invalid: bool, message: String },
}

/// A message's header: the body's fields plus the payload length.
#[derive(Serialize, Deserialize)]
struct Header<T> {
    #[serde(flatten)]
    body: T,
    bytes: usize,
}

fn write_frame<T: Serialize>(w: &mut impl Write, body: &T, payload: &[u8]) -> io::Result<()> {
    let header = serde_json::to_vec(&Header { body, bytes: payload.len() })?;
    w.write_all(&(header.len() as u32).to_le_bytes())?;
    w.write_all(&header)?;
    w.write_all(payload)?;
    w.flush()
}

/// The next message, or `None` on a clean EOF before it. A header over `MAX_HEADER`, or a payload
/// length that isn't what the body says, is an error before anything that size is allocated.
fn read_frame<T: DeserializeOwned + Body>(r: &mut impl Read) -> io::Result<Option<(T, Vec<u8>)>> {
    let mut len = [0u8; 4];
    match r.read_exact(&mut len) {
        Err(e) if e.kind() == io::ErrorKind::UnexpectedEof => return Ok(None),
        res => res?,
    }
    let len = u32::from_le_bytes(len) as usize;
    if len > MAX_HEADER {
        return Err(io::Error::other(format!("frame header is {len} bytes")));
    }
    let mut header = vec![0u8; len];
    r.read_exact(&mut header)?;
    let header: Header<T> = serde_json::from_slice(&header)?;
    if header.bytes != header.body.payload_len()? {
        return Err(io::Error::other("frame payload doesn't match its header"));
    }
    let mut payload = vec![0u8; header.bytes];
    r.read_exact(&mut payload)?;
    Ok(Some((header.body, payload)))
}

// ── worker side ──

/// The worker loop: answers each request with `handle`'s image or error, until stdin closes.
pub fn serve(mut r: impl Read, mut w: impl Write, mut handle: impl FnMut(Request, RgbaImage) -> AppResult<RgbaImage>) -> io::Result<()> {
    while let Some((req, payload)) = read_frame::<Request>(&mut r)? {
        let (iw, ih) = req.size();
        let out = RgbaImage::from_raw(iw, ih, payload)
            .ok_or_else(|| AppError::InvalidInput("image size doesn't match its bytes".into()))
            .and_then(|img| handle(req, img));
        match out {
            Ok(img) => write_frame(&mut w, &Response::Ok { w: img.width(), h: img.height() }, img.as_raw())?,
            Err(e) => {
                let invalid = matches!(e, AppError::InvalidInput(_));
                write_frame(&mut w, &Response::Err { invalid, message: e.to_string() }, &[])?
            }
        }
    }
    Ok(())
}

/// Cuts with the embedding held for `key`, embedding first when `held` is for another key. The old
/// embedding is dropped before embedding, so two are never in memory.
pub fn with_embedding<E, T>(
    held: &mut Option<(u64, E)>,
    key: u64,
    embed: impl FnOnce() -> AppResult<E>,
    cut: impl FnOnce(&E) -> AppResult<T>,
) -> AppResult<T> {
    if held.as_ref().is_none_or(|(k, _)| *k != key) {
        *held = None;
        *held = Some((key, embed()?));
    }
    cut(&held.as_ref().expect("just embedded").1)
}

/// Writes `log` records to stderr, which the parent inherits, so worker logs show in dev.
struct StderrLog;

impl log::Log for StderrLog {
    fn enabled(&self, m: &log::Metadata) -> bool {
        m.level() <= log::Level::Info
    }
    fn log(&self, r: &log::Record) {
        if self.enabled(r.metadata()) {
            eprintln!("[ai-worker] {} {}", r.level(), r.args());
        }
    }
    fn flush(&self) {}
}

/// The worker process's `main`. Exits when stdin closes, so it dies with the app.
pub fn main() {
    let _ = log::set_logger(&StderrLog).map(|()| log::set_max_level(log::LevelFilter::Info));
    let mut sam = None;
    let res = serve(BufReader::new(io::stdin().lock()), BufWriter::new(io::stdout().lock()), |req, img| {
        crate::app::ai::handle(&mut sam, req, img)
    });
    if let Err(e) = res {
        log::error!("pipe: {e}");
    }
}

// ── client side ──

struct Pipes {
    stdin: BufWriter<ChildStdin>,
    stdout: BufReader<ChildStdout>,
}

/// Held for a whole request, so requests are serial and `in_flight` can see one running.
// ponytail: one worker, serial requests; fine for one editor window.
static PIPES: Mutex<Option<Pipes>> = Mutex::new(None);
/// Locked only briefly, so `stop` can kill the child while a request holds `PIPES`.
static CHILD: Mutex<Slot> = Mutex::new(Slot { child: None, enabled: false });

/// The child, and whether one may be spawned: only while an image is open in the editor, so a
/// request that was queued or still running when the editor closed can't start a new worker.
struct Slot {
    child: Option<Child>,
    enabled: bool,
}

/// Spawns into `child`/`pipes` when they are empty and spawning is `enabled`. Dead pipes (the
/// child was killed between requests) are dropped first.
fn ensure<C, P>(child: &mut Option<C>, enabled: bool, pipes: &mut Option<P>, spawn: impl FnOnce() -> io::Result<(C, P)>) -> AppResult<()> {
    if child.is_none() {
        *pipes = None;
    }
    if pipes.is_some() {
        return Ok(());
    }
    if !enabled {
        return Err(AppError::InvalidInput("no image is open".into()));
    }
    let (c, p) = spawn().map_err(|e| AppError::Other(format!("ai worker: {e}")))?;
    *child = Some(c);
    *pipes = Some(p);
    Ok(())
}

fn spawn_at(exe: &std::path::Path) -> io::Result<(Child, Pipes)> {
    let mut cmd = Command::new(exe);
    cmd.arg(FLAG).stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::inherit());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
    }
    let mut child = cmd.spawn()?;
    let pipes = Pipes {
        stdin: BufWriter::new(child.stdin.take().expect("piped")),
        stdout: BufReader::new(child.stdout.take().expect("piped")),
    };
    Ok((child, pipes))
}

fn exchange(p: &mut Pipes, req: &Request, img: &RgbaImage) -> io::Result<AppResult<RgbaImage>> {
    write_frame(&mut p.stdin, req, img.as_raw())?;
    match read_frame::<Response>(&mut p.stdout)? {
        Some((Response::Ok { w, h }, bytes)) => {
            RgbaImage::from_raw(w, h, bytes).map(Ok).ok_or_else(|| io::Error::other("reply size doesn't match its bytes"))
        }
        Some((Response::Err { invalid: true, message }, _)) => Ok(Err(AppError::InvalidInput(message))),
        Some((Response::Err { invalid: false, message }, _)) => Ok(Err(AppError::Other(message))),
        None => Err(io::ErrorKind::UnexpectedEof.into()),
    }
}

/// Sends `req` with `img` to the worker, spawning it if needed and `enable`d (else
/// `InvalidInput("no image is open")`). On any pipe failure the worker is dropped and
/// `Other("ai worker failed")` returned; the next call respawns it. `img` is converted to RGBA
/// only once `PIPES` is held, so queued requests don't each keep a full-res copy.
pub fn call(req: &Request, img: &DynamicImage) -> AppResult<RgbaImage> {
    let mut pipes = PIPES.lock().unwrap_or_else(PoisonError::into_inner);
    {
        let mut slot = CHILD.lock().unwrap_or_else(PoisonError::into_inner);
        let enabled = slot.enabled;
        ensure(&mut slot.child, enabled, &mut pipes, || spawn_at(&std::env::current_exe()?))?;
    }
    let rgba = img.as_rgba8().map_or_else(|| Cow::Owned(img.to_rgba8()), Cow::Borrowed);
    match exchange(pipes.as_mut().expect("spawned"), req, &rgba) {
        Ok(res) => res,
        Err(e) => {
            log::error!("ai worker: {e}");
            *pipes = None;
            kill();
            Err(AppError::Other("ai worker failed".into()))
        }
    }
}

fn kill() {
    if let Some(mut child) = CHILD.lock().unwrap_or_else(PoisonError::into_inner).child.take() {
        let _ = child.kill();
        let _ = child.wait();
    }
}

/// Kills and reaps the worker, cancelling a running request (it fails with "ai worker failed").
pub fn stop() {
    kill();
    if let Ok(mut pipes) = PIPES.try_lock() {
        *pipes = None;
    }
}

/// Lets requests spawn the worker; call when the editor opens an image.
pub fn enable() {
    CHILD.lock().unwrap_or_else(PoisonError::into_inner).enabled = true;
}

/// Stops the worker and keeps later requests from respawning it until `enable`; call when the
/// editor closes and on app exit.
pub fn disable() {
    CHILD.lock().unwrap_or_else(PoisonError::into_inner).enabled = false;
    stop();
}

/// Whether a request is running right now.
pub fn in_flight() -> bool {
    matches!(PIPES.try_lock(), Err(TryLockError::WouldBlock))
}

#[cfg(test)]
pub(crate) fn hold_for_test() -> impl Sized {
    PIPES.lock().unwrap_or_else(PoisonError::into_inner)
}

#[cfg(test)]
mod tests {
    use super::*;
    use ::image::Rgba;

    fn frames(msgs: &[(Request, Vec<u8>)]) -> Vec<u8> {
        let mut buf = Vec::new();
        for (req, payload) in msgs {
            write_frame(&mut buf, req, payload).unwrap();
        }
        buf
    }

    fn bg(w: u32, h: u32) -> Request {
        Request::RemoveBg { model: "/m/model.onnx".into(), w, h }
    }

    #[test]
    fn frame_round_trip() {
        let req = Request::Subject {
            encoder: "/e.onnx".into(),
            decoder: "/d.onnx".into(),
            key: 7,
            points: vec![SubjectPoint { x: 1, y: 0, keep: true }],
            lasso: Some(vec![[0, 0], [1, 0], [1, 1]]),
            w: 2,
            h: 1,
        };
        let payload = vec![1, 2, 3, 4, 5, 6, 7, 8];
        let buf = frames(&[(req.clone(), payload.clone())]);
        let mut r = &buf[..];
        assert_eq!(read_frame::<Request>(&mut r).unwrap(), Some((req, payload)));
        assert_eq!(read_frame::<Request>(&mut r).unwrap(), None);
        let json: serde_json::Value = serde_json::from_slice(&buf[4..buf.len() - 8]).unwrap();
        assert_eq!((json["type"].as_str(), json["bytes"].as_u64()), (Some("subject"), Some(8)));
    }

    #[test]
    fn serve_dispatches() {
        let input = frames(&[(bg(2, 1), vec![9; 8]), (bg(1, 1), vec![0; 4]), (bg(1, 1), vec![0; 3])]);
        let mut out = Vec::new();
        // A payload that doesn't match `w × h` is a pipe error: it never reaches the handler.
        let res = serve(&input[..], &mut out, |req, img| match req {
            Request::RemoveBg { w: 2, .. } => {
                assert_eq!(img.as_raw(), &[9; 8]);
                Ok(RgbaImage::from_pixel(1, 1, Rgba([1, 2, 3, 4])))
            }
            _ => Err(AppError::InvalidInput("nope".into())),
        });
        assert!(res.is_err());
        let mut r = &out[..];
        assert_eq!(read_frame::<Response>(&mut r).unwrap(), Some((Response::Ok { w: 1, h: 1 }, vec![1, 2, 3, 4])));
        let invalid = Response::Err { invalid: true, message: "nope".into() };
        assert_eq!(read_frame::<Response>(&mut r).unwrap(), Some((invalid, vec![])));
        assert_eq!(read_frame::<Response>(&mut r).unwrap(), None);
    }

    #[test]
    fn oversized_frames_are_errors() {
        // A 4 GiB header length is refused before it's allocated.
        let mut r = &u32::MAX.to_le_bytes()[..];
        assert!(read_frame::<Response>(&mut r).is_err());
        // So are sides over MAX_SIDE and a byte count that isn't w × h × 4, with no payload sent.
        for (w, h, bytes) in [(MAX_SIDE + 1, 1, 4 * (MAX_SIDE as usize + 1)), (1, 1, usize::MAX)] {
            let header = serde_json::to_vec(&Header { body: Response::Ok { w, h }, bytes }).unwrap();
            let mut buf = (header.len() as u32).to_le_bytes().to_vec();
            buf.extend(header);
            assert!(read_frame::<Response>(&mut &buf[..]).is_err());
        }
        let mut r = &frames(&[(bg(MAX_SIDE + 1, 1), vec![])])[..];
        assert!(read_frame::<Request>(&mut r).is_err());
    }

    #[test]
    fn spawns_only_while_enabled() {
        let spawns = std::cell::Cell::new(0);
        let spawn = || {
            spawns.set(spawns.get() + 1);
            Ok(((), ()))
        };
        let (mut child, mut pipes) = (None, None);
        ensure(&mut child, true, &mut pipes, spawn).unwrap();
        ensure(&mut child, true, &mut pipes, spawn).unwrap();
        assert_eq!(spawns.get(), 1);
        // Editor closed: disabled, then the child killed. A late request must not respawn.
        child = None;
        assert!(matches!(ensure(&mut child, false, &mut pipes, spawn), Err(AppError::InvalidInput(_))));
        assert!(pipes.is_none() && spawns.get() == 1);
        // The editor opened again: it respawns.
        ensure(&mut child, true, &mut pipes, spawn).unwrap();
        assert_eq!(spawns.get(), 2);
    }

    #[test]
    fn subject_reembeds_only_on_new_key() {
        let (mut embeds, mut cuts, mut held) = (0, 0, None);
        for key in [1, 1, 2] {
            let got = with_embedding(&mut held, key, || { embeds += 1; Ok(key * 10) }, |e| { cuts += 1; Ok(*e) }).unwrap();
            assert_eq!(got, key * 10);
        }
        assert_eq!((embeds, cuts), (2, 3));
        // A failed embed leaves nothing held, so the next call embeds again.
        assert!(with_embedding(&mut held, 3, || Err(AppError::Other("x".into())), |e: &u64| Ok(*e)).is_err());
        assert!(held.is_none());
    }

    fn rss_kb(pid: u32) -> String {
        let out = Command::new("ps").args(["-o", "rss=", "-p", &pid.to_string()]).output().unwrap();
        String::from_utf8_lossy(&out.stdout).trim().to_string()
    }

    /// Spawns the dev build (`cargo build` first) as a worker and sends it real requests: SAM
    /// subject cuts on `VEE_AI_IMAGE` from `VEE_SAM_DIR` (`encoder.onnx`, `decoder.onnx`), and a
    /// BiRefNet cut if `VEE_AI_MODEL` is set. Prints the worker's and this process's RSS, and
    /// checks the worker is gone after the kill.
    /// `cargo test --lib worker_real_spawn -- --ignored --nocapture`
    #[test]
    #[ignore]
    fn worker_real_spawn() {
        let exe = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("target/debug").join(if cfg!(windows) { "vee-app.exe" } else { "vee-app" });
        let dir = PathBuf::from(std::env::var("VEE_SAM_DIR").expect("set VEE_SAM_DIR"));
        let img = ::image::open(std::env::var("VEE_AI_IMAGE").expect("set VEE_AI_IMAGE")).unwrap().to_rgba8();
        let (w, h) = img.dimensions();
        let (mut child, mut pipes) = spawn_at(&exe).unwrap();
        let pid = child.id();
        println!("parent rss {} KB, worker rss at start {} KB", rss_kb(std::process::id()), rss_kb(pid));
        let centre = SubjectPoint { x: w / 2, y: h / 2, keep: true };
        for (key, points) in [(1, vec![centre.clone()]), (1, vec![centre.clone(), SubjectPoint { x: 0, y: 0, keep: false }])] {
            let req = Request::Subject { encoder: dir.join("encoder.onnx"), decoder: dir.join("decoder.onnx"), key, points, lasso: None, w, h };
            let start = std::time::Instant::now();
            let cut = exchange(&mut pipes, &req, &img).unwrap().unwrap();
            println!("subject in {} ms, worker rss {} KB", start.elapsed().as_millis(), rss_kb(pid));
            assert_eq!(cut.dimensions(), (w, h));
            assert!(cut.get_pixel(centre.x, centre.y)[3] > 200);
        }
        if let Ok(model) = std::env::var("VEE_AI_MODEL") {
            let start = std::time::Instant::now();
            let cut = exchange(&mut pipes, &Request::RemoveBg { model: model.into(), w, h }, &img).unwrap().unwrap();
            println!("removeBg in {} ms, worker rss {} KB", start.elapsed().as_millis(), rss_kb(pid));
            assert_eq!(cut.dimensions(), (w, h));
        }
        let missing = exchange(&mut pipes, &Request::RemoveBg { model: "/nope.onnx".into(), w, h }, &img).unwrap();
        assert!(missing.is_err());
        println!("parent rss {} KB", rss_kb(std::process::id()));
        child.kill().unwrap();
        child.wait().unwrap();
        assert_eq!(rss_kb(pid), "", "worker still running");
        println!("worker gone after kill");
    }
}
