//! Windows 10 2004+ recording: an FFmpeg child grabs the screen into `video.mkv` while a thread records system
//! audio with WASAPI loopback into `audio.wav`; `stop` ends both and muxes them into the output.
//!
//! Video is `gdigrab`, not `ddagrab` (see `plan::grab_args`): any GPU, RDP and VMs, and a plain offset + size.
//!
//! A/V sync: the loopback client starts first, then FFmpeg is spawned at once, so both start at about the same
//! instant. Loopback delivers no packets while nothing plays, so the audio thread pads with silence to keep its
//! length on the wall clock; the mux cuts both to the shorter (`-shortest`).
//! ponytail: FFmpeg's input-open latency (~0.1–0.3 s) shows as audio slightly ahead of video; trim the WAV by
//! the time FFmpeg reports its first frame (`-progress`) if that is noticeable.
//!
//! Vee's own sounds are not excluded here (process-excluding loopback is Windows 11 only): the caller plays the
//! start sound before `start` and the stop sound after `stop`.

use super::plan::{self, Container};
use crate::{
    app::{
        capture::geometry::{MonitorInfo, Selection, to_physical},
        engines,
    },
    domain::convert::{Killed, command},
    shared::error::{AppError, AppResult},
};
use std::fs::File;
use std::io::{BufWriter, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::thread::JoinHandle;
use std::time::{Duration, Instant};
use tauri::AppHandle;

/// How long FFmpeg gets to finish the file after `q`, and the audio client to start.
const TIMEOUT: Duration = Duration::from_secs(10);
const POLL: Duration = Duration::from_millis(10);

/// A recording in progress.
pub struct Running {
    ffmpeg: PathBuf,
    grab: Option<Killed>,
    audio: Option<JoinHandle<Result<(), String>>>,
    stop_audio: Arc<AtomicBool>,
    video_file: PathBuf,
    audio_file: PathBuf,
    out: PathBuf,
}

fn other(step: &str, e: impl std::fmt::Display) -> AppError {
    AppError::Other(format!("recording: {step}: {e}"))
}

impl Running {
    pub fn start(app: &AppHandle, monitor: &MonitorInfo, sel: &Selection, _container: Container, out: &Path) -> AppResult<Running> {
        let (ffmpeg, _) = engines::ffmpeg(app).ok_or_else(|| other("ffmpeg", "not installed"))?;
        let rect = match *sel {
            Selection::Rect { x, y, width, height } => to_physical((x, y, width, height), monitor),
            Selection::Window { id } => window_rect(id)?,
        };
        let dir = std::env::temp_dir().join(format!("vee-record-{}", app.config().identifier));
        std::fs::create_dir_all(&dir)?;
        let (video_file, audio_file) = (dir.join("video.mkv"), dir.join("audio.wav"));

        let stop_audio = Arc::new(AtomicBool::new(false));
        let (ready_tx, ready) = mpsc::channel();
        let audio = {
            let (path, stop) = (audio_file.clone(), stop_audio.clone());
            std::thread::spawn(move || loopback(&path, &stop, ready_tx))
        };
        // From here on, dropping `running` stops and joins the audio thread and kills FFmpeg.
        let mut running = Running { ffmpeg, grab: None, audio: Some(audio), stop_audio, video_file, audio_file, out: out.to_path_buf() };
        match ready.recv_timeout(TIMEOUT) {
            Ok(Ok(())) => {}
            Ok(Err(e)) => return Err(running.fail(other("audio", e))),
            Err(_) => return Err(running.fail(other("audio", "timed out"))),
        }
        let child = command(&running.ffmpeg)
            .args(plan::grab_args(rect, &running.video_file))
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn();
        match child {
            Ok(c) => running.grab = Some(Killed(c)),
            Err(e) => return Err(running.fail(other("ffmpeg", e))),
        }
        Ok(running)
    }

    /// Ends the grab and the audio, muxes them into the output and waits until it is written.
    pub fn stop(mut self) -> AppResult<PathBuf> {
        let r = self.finish();
        self.remove_parts();
        if r.is_err() {
            let _ = std::fs::remove_file(&self.out);
        }
        r.map(|()| self.out.clone())
    }

    /// Stops and deletes everything.
    pub fn cancel(mut self) {
        let _ = self.halt();
        self.remove_parts();
        let _ = std::fs::remove_file(&self.out);
    }

    fn finish(&mut self) -> AppResult<()> {
        let mut grab = self.grab.take().ok_or_else(|| other("ffmpeg", "not running"))?;
        // `q` makes FFmpeg flush and close the file; dropping stdin then also ends it if `q` was missed.
        if let Some(mut stdin) = grab.0.stdin.take() {
            let _ = stdin.write_all(b"q");
        }
        let deadline = Instant::now() + TIMEOUT;
        loop {
            match grab.0.try_wait()? {
                Some(status) if status.success() => break,
                Some(status) => return Err(other("ffmpeg", status)),
                None if Instant::now() < deadline => std::thread::sleep(POLL),
                // Dropping `grab` kills it.
                None => return Err(other("ffmpeg", "did not stop")),
            }
        }
        // A loopback failure mid-recording (device removed, default device changed) cut the WAV short:
        // keep the full video without sound rather than trimming it to the audio.
        let args = match self.halt() {
            Ok(()) => plan::mux_args(&self.video_file, &self.audio_file, &self.out),
            Err(e) => {
                log::warn!("recording: audio: {e}; saving without sound");
                plan::mux_args_video_only(&self.video_file, &self.out)
            }
        };
        let mut mux = command(&self.ffmpeg);
        mux.args(args).stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null());
        let mut child = Killed(mux.spawn().map_err(|e| other("mux", e))?);
        // ponytail: no deadline on the mux; it only copies video and encodes the audio, seconds even for long recordings.
        match child.0.wait()? {
            s if s.success() => Ok(()),
            s => Err(other("mux", s)),
        }
    }

    /// Stops the audio thread (joined) and kills FFmpeg if it still runs. `Err` when the audio thread failed.
    fn halt(&mut self) -> Result<(), String> {
        self.stop_audio.store(true, Ordering::SeqCst);
        let audio = match self.audio.take().map(JoinHandle::join) {
            Some(Ok(r)) => r,
            Some(Err(_)) => Err("thread panicked".into()),
            None => Ok(()),
        };
        self.grab = None;
        audio
    }

    fn remove_parts(&self) {
        let _ = std::fs::remove_file(&self.video_file);
        let _ = std::fs::remove_file(&self.audio_file);
    }

    fn fail(mut self, e: AppError) -> AppError {
        let _ = self.halt();
        self.remove_parts();
        e
    }
}

impl Drop for Running {
    /// The app quit or panicked mid-recording: no thread or FFmpeg is left behind.
    fn drop(&mut self) {
        let _ = self.halt();
    }
}

/// The window's frame (no drop shadow) in global physical px at the start, recorded as a fixed rect.
fn window_rect(id: u64) -> AppResult<(i32, i32, u32, u32)> {
    use ::windows::Win32::Foundation::{HWND, RECT};
    use ::windows::Win32::Graphics::Dwm::{DWMWA_EXTENDED_FRAME_BOUNDS, DwmGetWindowAttribute};
    let mut r = RECT::default();
    unsafe {
        DwmGetWindowAttribute(
            HWND(id as usize as *mut _),
            DWMWA_EXTENDED_FRAME_BOUNDS,
            &mut r as *mut _ as *mut _,
            std::mem::size_of::<RECT>() as u32,
        )
    }
    .map_err(|e| other("window", e))?;
    Ok((r.left, r.top, (r.right - r.left).max(2) as u32, (r.bottom - r.top).max(2) as u32))
}

/// Records the default render device's mix with WASAPI loopback into the WAV at `path` until `stop` is set.
/// `ready` gets `Ok` once the client runs, or the error if it never does.
fn loopback(path: &Path, stop: &AtomicBool, ready: mpsc::Sender<Result<(), String>>) -> Result<(), String> {
    use ::windows::Win32::System::Com::{COINIT_MULTITHREADED, CoInitializeEx, CoUninitialize};
    let init = unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) };
    // Every COM object lives inside `capture`, so all are released before `CoUninitialize`.
    let r = unsafe { capture(path, stop, &ready) };
    if init.is_ok() {
        unsafe { CoUninitialize() };
    }
    if let Err(e) = &r {
        // Ignored when `start` already got `Ok`: a failure mid-recording shows up when the thread is joined.
        let _ = ready.send(Err(e.clone()));
    }
    r
}

unsafe fn capture(path: &Path, stop: &AtomicBool, ready: &mpsc::Sender<Result<(), String>>) -> Result<(), String> {
    use ::windows::Win32::Media::Audio::{
        AUDCLNT_BUFFERFLAGS_SILENT, AUDCLNT_SHAREMODE_SHARED, AUDCLNT_STREAMFLAGS_LOOPBACK, IAudioCaptureClient, IMMDeviceEnumerator,
        MMDeviceEnumerator, eConsole, eRender,
    };
    use ::windows::Win32::System::Com::{CLSCTX_ALL, CoCreateInstance, CoTaskMemFree};
    let e = |step: &'static str| move |err: ::windows::core::Error| format!("{step}: {err}");

    let enumerator: IMMDeviceEnumerator = unsafe { CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL) }.map_err(e("enumerator"))?;
    let device = unsafe { enumerator.GetDefaultAudioEndpoint(eRender, eConsole) }.map_err(e("device"))?;
    let client = unsafe { activate(&device) }.map_err(e("activate"))?;
    let fmt = unsafe { client.GetMixFormat() }.map_err(e("format"))?;
    // The mix format (usually WAVE_FORMAT_EXTENSIBLE float) is written into the WAV header as is.
    let (fmt_bytes, block, rate) = unsafe {
        let f = &*fmt;
        (std::slice::from_raw_parts(fmt as *const u8, 18 + f.cbSize as usize).to_vec(), f.nBlockAlign as usize, f.nSamplesPerSec as u64)
    };
    // 1 s buffer (100 ns units), far more than the 10 ms poll needs.
    let init = unsafe { client.Initialize(AUDCLNT_SHAREMODE_SHARED, AUDCLNT_STREAMFLAGS_LOOPBACK, 10_000_000, 0, fmt, None) };
    unsafe { CoTaskMemFree(Some(fmt as *const _)) };
    init.map_err(e("initialize"))?;
    let cap: IAudioCaptureClient = unsafe { client.GetService() }.map_err(e("service"))?;
    let mut wav = Wav::create(path, &fmt_bytes).map_err(|err| format!("wav: {err}"))?;
    unsafe { client.Start() }.map_err(e("start"))?;
    let t0 = Instant::now();
    let _ = ready.send(Ok(()));

    let silent = AUDCLNT_BUFFERFLAGS_SILENT.0 as u32;
    let mut frames_written = 0u64;
    let io = |err: std::io::Error| format!("wav: {err}");
    while !stop.load(Ordering::SeqCst) {
        std::thread::sleep(POLL);
        while unsafe { cap.GetNextPacketSize() }.map_err(e("packet"))? > 0 {
            let (mut data, mut frames, mut flags) = (std::ptr::null_mut(), 0u32, 0u32);
            unsafe { cap.GetBuffer(&mut data, &mut frames, &mut flags, None, None) }.map_err(e("buffer"))?;
            let len = frames as usize * block;
            let written = if flags & silent != 0 || data.is_null() {
                wav.silence(len)
            } else {
                wav.write(unsafe { std::slice::from_raw_parts(data, len) })
            };
            unsafe { cap.ReleaseBuffer(frames) }.map_err(e("release"))?;
            written.map_err(io)?;
            frames_written += u64::from(frames);
        }
        // Nothing plays → no packets: pad up to the wall clock, minus 20 ms for packets still in flight.
        let due = (t0.elapsed().as_micros() as u64 * rate / 1_000_000).saturating_sub(rate / 50);
        if due > frames_written {
            wav.silence((due - frames_written) as usize * block).map_err(io)?;
            frames_written = due;
        }
    }
    let _ = unsafe { client.Stop() };
    wav.finish().map_err(io)
}

/// `IMMDevice::Activate` for an `IAudioClient`. The `windows` crate gates that method behind the
/// `Win32_System_Com_StructuredStorage` feature (for its optional `PROPVARIANT` param), which isn't enabled,
/// so it is called through the vtable: slot 3, right after IUnknown's three, with no activation params.
unsafe fn activate(
    device: &::windows::Win32::Media::Audio::IMMDevice,
) -> ::windows::core::Result<::windows::Win32::Media::Audio::IAudioClient> {
    use ::windows::Win32::Media::Audio::IAudioClient;
    use ::windows::Win32::System::Com::{CLSCTX, CLSCTX_ALL};
    use ::windows::core::{GUID, HRESULT, Interface};
    use std::ffi::c_void;
    type ActivateFn = unsafe extern "system" fn(*mut c_void, *const GUID, CLSCTX, *const c_void, *mut *mut c_void) -> HRESULT;
    unsafe {
        let raw = device.as_raw();
        let vtable = *(raw as *const *const usize);
        let f: ActivateFn = std::mem::transmute::<usize, ActivateFn>(*vtable.add(3));
        let mut out = std::ptr::null_mut();
        f(raw, &IAudioClient::IID, CLSCTX_ALL, std::ptr::null(), &mut out).ok()?;
        Ok(IAudioClient::from_raw(out))
    }
}

/// A RIFF/WAVE file whose sizes are filled in by `finish`.
// ponytail: 32-bit RIFF sizes cap the WAV at 4 GB, about 3 h of 48 kHz stereo float; RF64 if recordings get that long.
struct Wav {
    file: BufWriter<File>,
    fmt_len: u32,
    data_len: u64,
}

impl Wav {
    fn create(path: &Path, fmt: &[u8]) -> std::io::Result<Wav> {
        let mut file = BufWriter::new(File::create(path)?);
        let fmt_len = fmt.len() as u32;
        file.write_all(b"RIFF\0\0\0\0WAVEfmt ")?;
        file.write_all(&fmt_len.to_le_bytes())?;
        file.write_all(fmt)?;
        file.write_all(b"data\0\0\0\0")?;
        Ok(Wav { file, fmt_len, data_len: 0 })
    }

    fn write(&mut self, bytes: &[u8]) -> std::io::Result<()> {
        self.data_len += bytes.len() as u64;
        self.file.write_all(bytes)
    }

    fn silence(&mut self, len: usize) -> std::io::Result<()> {
        self.write(&vec![0u8; len])
    }

    fn finish(mut self) -> std::io::Result<()> {
        let data = self.data_len.min(u64::from(u32::MAX - 64)) as u32;
        self.file.seek(SeekFrom::Start(4))?;
        self.file.write_all(&(4 + 8 + self.fmt_len + 8 + data).to_le_bytes())?;
        self.file.seek(SeekFrom::Start(u64::from(20 + self.fmt_len + 4)))?;
        self.file.write_all(&data.to_le_bytes())?;
        self.file.flush()
    }
}
