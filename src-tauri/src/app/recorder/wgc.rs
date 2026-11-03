//! Windows.Graphics.Capture (Windows 10 1903+) of one window: the window's own pixels, wherever it moves and
//! whatever covers it, unlike a `gdigrab` screen rect. The newest frame is kept as tight BGRA for the caller.
//!
//! Windows 10 draws a yellow border around a captured window and Windows 11 may too; `IsBorderRequired(false)`
//! turns it off where the OS allows it (Windows 11, or a granted borderless capture). The border is drawn on
//! screen only, never in the captured frames.
//!
//! The frame pool is free-threaded, so `FrameArrived` runs on a system thread-pool thread: no thread of ours.
//! `Drop` unregisters the handler and closes the session and pool, which releases the D3D device and buffers.

use ::windows::Foundation::TypedEventHandler;
use ::windows::Graphics::Capture::{Direct3D11CaptureFramePool, GraphicsCaptureItem, GraphicsCaptureSession};
use ::windows::Graphics::DirectX::Direct3D11::IDirect3DDevice;
use ::windows::Graphics::DirectX::DirectXPixelFormat;
use ::windows::Graphics::SizeInt32;
use ::windows::Win32::Foundation::{HMODULE, HWND};
use ::windows::Win32::Graphics::Direct3D::{D3D_DRIVER_TYPE_HARDWARE, D3D_DRIVER_TYPE_WARP};
use ::windows::Win32::Graphics::Direct3D11::{
    D3D11_CPU_ACCESS_READ, D3D11_CREATE_DEVICE_BGRA_SUPPORT, D3D11_MAP_READ, D3D11_MAPPED_SUBRESOURCE, D3D11_SDK_VERSION,
    D3D11_TEXTURE2D_DESC, D3D11_USAGE_STAGING, D3D11CreateDevice, ID3D11Device, ID3D11DeviceContext, ID3D11Texture2D,
};
use ::windows::Win32::Graphics::Dxgi::IDXGIDevice;
use ::windows::Win32::System::WinRT::Direct3D11::{CreateDirect3D11DeviceFromDXGIDevice, IDirect3DDxgiInterfaceAccess};
use ::windows::Win32::System::WinRT::Graphics::Capture::IGraphicsCaptureItemInterop;
use ::windows::Win32::UI::WindowsAndMessaging::IsWindow;
use ::windows::core::{IInspectable, Interface, Ref};
use std::sync::{Arc, Mutex};

const FORMAT: DirectXPixelFormat = DirectXPixelFormat::B8G8R8A8UIntNormalized;
const BUFFERS: i32 = 2;

/// One captured frame: `height` rows of `width * 4` bytes, no padding.
pub(super) struct Frame {
    pub width: u32,
    pub height: u32,
    pub bgra: Vec<u8>,
}

pub(super) enum StartError {
    /// WGC can't capture this window here: fall back to `gdigrab`.
    Unavailable(String),
    /// The window no longer exists.
    Gone,
}

type Latest = Arc<Mutex<Option<Arc<Frame>>>>;

/// A running capture of one window; stopped and freed when dropped.
pub(super) struct Capture {
    session: GraphicsCaptureSession,
    pool: Direct3D11CaptureFramePool,
    token: i64,
    size: (u32, u32),
    latest: Latest,
}

/// The frame handler's state; lives in the handler, so it goes when the pool releases the handler.
struct Gpu {
    device: ID3D11Device,
    context: ID3D11DeviceContext,
    winrt: IDirect3DDevice,
    /// The pool's buffer size.
    size: SizeInt32,
    /// Reused while the frame size stays the same.
    staging: Option<ID3D11Texture2D>,
}

// SAFETY: the D3D11 device is free-threaded and its WinRT wrapper is agile; the immediate context (not
// thread-safe) is only touched while the handler holds the `Gpu` mutex.
unsafe impl Send for Gpu {}

fn unavailable(step: &str, e: impl std::fmt::Display) -> StartError {
    StartError::Unavailable(format!("wgc: {step}: {e}"))
}

impl Capture {
    pub(super) fn start(hwnd: u64) -> Result<Capture, StartError> {
        let hwnd = HWND(hwnd as usize as *mut _);
        // SAFETY: `IsWindow` only reads the handle; any value is allowed.
        if !unsafe { IsWindow(Some(hwnd)) }.as_bool() {
            return Err(StartError::Gone);
        }
        if !GraphicsCaptureSession::IsSupported().unwrap_or(false) {
            return Err(unavailable("supported", "no"));
        }
        let device = d3d_device()?;
        let context = {
            // SAFETY: a live device always has an immediate context.
            unsafe { device.GetImmediateContext() }.map_err(|e| unavailable("context", e))?
        };
        let winrt = {
            let dxgi: IDXGIDevice = device.cast().map_err(|e| unavailable("dxgi", e))?;
            // SAFETY: `dxgi` is a live DXGI device of `device`.
            unsafe { CreateDirect3D11DeviceFromDXGIDevice(&dxgi) }
                .and_then(|d| d.cast::<IDirect3DDevice>())
                .map_err(|e| unavailable("winrt device", e))?
        };
        let item = ::windows::core::factory::<GraphicsCaptureItem, IGraphicsCaptureItemInterop>()
            // SAFETY: `hwnd` was a window a moment ago; a stale handle makes this fail, not crash.
            .and_then(|interop| unsafe { interop.CreateForWindow::<GraphicsCaptureItem>(hwnd) })
            .map_err(|e| unavailable("item", e))?;
        let size = item.Size().map_err(|e| unavailable("size", e))?;
        let pool =
            Direct3D11CaptureFramePool::CreateFreeThreaded(&winrt, FORMAT, BUFFERS, size).map_err(|e| unavailable("frame pool", e))?;
        let session = pool.CreateCaptureSession(&item).map_err(|e| unavailable("session", e))?;

        let latest: Latest = Arc::default();
        let gpu = Mutex::new(Gpu { device, context, winrt, size, staging: None });
        let sink = latest.clone();
        let handler = TypedEventHandler::new(move |pool: Ref<Direct3D11CaptureFramePool>, _: Ref<IInspectable>| {
            let Ok(mut gpu) = gpu.lock() else { return Ok(()) };
            match pool.ok().and_then(|p| on_frame(p, &mut gpu)) {
                Ok(Some(frame)) => {
                    if let Ok(mut l) = sink.lock() {
                        *l = Some(Arc::new(frame));
                    }
                }
                Ok(None) => {}
                Err(e) => log::warn!("wgc: frame skipped: {e}"),
            }
            Ok(())
        });
        let token = pool.FrameArrived(&handler).map_err(|e| unavailable("frame handler", e))?;
        let capture = Capture { session, pool, token, size: (size.Width.max(0) as u32, size.Height.max(0) as u32), latest };
        let _ = capture.session.SetIsCursorCaptureEnabled(true);
        if let Err(e) = capture.session.SetIsBorderRequired(false) {
            log::debug!("wgc: border stays: {e}");
        }
        // On an error `capture` drops here, which unregisters the handler and closes everything.
        capture.session.StartCapture().map_err(|e| unavailable("start", e))?;
        Ok(capture)
    }

    /// The window's size at the start, in physical px.
    pub(super) fn size(&self) -> (u32, u32) {
        self.size
    }

    /// The newest frame; `None` until the first arrives.
    pub(super) fn latest(&self) -> Option<Arc<Frame>> {
        self.latest.lock().ok()?.clone()
    }
}

impl Drop for Capture {
    fn drop(&mut self) {
        let _ = self.pool.RemoveFrameArrived(self.token);
        let _ = self.session.Close();
        let _ = self.pool.Close();
        if let Ok(mut l) = self.latest.lock() {
            l.take();
        }
    }
}

/// A D3D11 device that WGC can draw into: the GPU, else WARP (software).
fn d3d_device() -> Result<ID3D11Device, StartError> {
    let mut last = None;
    for driver in [D3D_DRIVER_TYPE_HARDWARE, D3D_DRIVER_TYPE_WARP] {
        let mut device = None;
        // SAFETY: `device` outlives the call; no adapter, module or feature-level array is passed.
        let r = unsafe {
            D3D11CreateDevice(
                None,
                driver,
                HMODULE::default(),
                D3D11_CREATE_DEVICE_BGRA_SUPPORT,
                None,
                D3D11_SDK_VERSION,
                Some(&mut device),
                None,
                None,
            )
        };
        match (r, device) {
            (Ok(()), Some(d)) => return Ok(d),
            (Err(e), _) => last = Some(e.to_string()),
            (Ok(()), None) => last = Some("no device".into()),
        }
    }
    Err(unavailable("d3d11 device", last.unwrap_or_default()))
}

/// Takes the next frame from `pool` and copies it out as tight BGRA. `None`: no frame, or the window was
/// resized and the pool is recreated at the new size (that frame is skipped).
fn on_frame(pool: &Direct3D11CaptureFramePool, gpu: &mut Gpu) -> ::windows::core::Result<Option<Frame>> {
    let frame = pool.TryGetNextFrame()?;
    let content = frame.ContentSize()?;
    if content != gpu.size {
        let _ = frame.Close();
        if content.Width > 0 && content.Height > 0 {
            pool.Recreate(&gpu.winrt, FORMAT, BUFFERS, content)?;
            gpu.size = content;
        }
        return Ok(None);
    }
    // SAFETY: the surface of a D3D11 frame pool is a DXGI surface backed by an `ID3D11Texture2D`.
    let texture: ID3D11Texture2D = unsafe { frame.Surface()?.cast::<IDirect3DDxgiInterfaceAccess>()?.GetInterface()? };
    let mut desc = D3D11_TEXTURE2D_DESC::default();
    // SAFETY: `desc` is a valid out pointer for the call.
    unsafe { texture.GetDesc(&mut desc) };
    let staging = staging(gpu, &desc)?;
    // SAFETY: both textures belong to `gpu.device` and have the same size and format.
    unsafe { gpu.context.CopyResource(&staging, &texture) };
    let _ = frame.Close();

    let (width, height) = (desc.Width, desc.Height);
    let row = width as usize * 4;
    let mut mapped = D3D11_MAPPED_SUBRESOURCE::default();
    // SAFETY: `staging` is a CPU-readable staging texture; `mapped` is a valid out pointer.
    unsafe { gpu.context.Map(&staging, 0, D3D11_MAP_READ, 0, Some(&mut mapped))? };
    let mut bgra = Vec::with_capacity(row * height as usize);
    for y in 0..height as usize {
        // SAFETY: while mapped, `pData` holds `height` rows `RowPitch` (>= `row`) bytes apart.
        let src = unsafe { std::slice::from_raw_parts((mapped.pData as *const u8).add(y * mapped.RowPitch as usize), row) };
        bgra.extend_from_slice(src);
    }
    // SAFETY: mapped above, and `src` is no longer used.
    unsafe { gpu.context.Unmap(&staging, 0) };
    Ok(Some(Frame { width, height, bgra }))
}

/// The staging texture for frames shaped like `desc`, recreated when their size changes.
fn staging(gpu: &mut Gpu, desc: &D3D11_TEXTURE2D_DESC) -> ::windows::core::Result<ID3D11Texture2D> {
    if let Some(s) = &gpu.staging {
        let mut have = D3D11_TEXTURE2D_DESC::default();
        // SAFETY: `have` is a valid out pointer for the call.
        unsafe { s.GetDesc(&mut have) };
        if (have.Width, have.Height, have.Format) == (desc.Width, desc.Height, desc.Format) {
            return Ok(s.clone());
        }
    }
    gpu.staging = None;
    let want = D3D11_TEXTURE2D_DESC {
        Width: desc.Width,
        Height: desc.Height,
        MipLevels: 1,
        ArraySize: 1,
        Format: desc.Format,
        SampleDesc: desc.SampleDesc,
        Usage: D3D11_USAGE_STAGING,
        BindFlags: 0,
        CPUAccessFlags: D3D11_CPU_ACCESS_READ.0 as u32,
        MiscFlags: 0,
    };
    let mut s = None;
    // SAFETY: `want` and `s` outlive the call; no initial data.
    unsafe { gpu.device.CreateTexture2D(&want, None, Some(&mut s))? };
    let s = s.ok_or_else(|| ::windows::core::Error::from(::windows::Win32::Foundation::E_FAIL))?;
    gpu.staging = Some(s.clone());
    Ok(s)
}

/// `Running` (which will hold a `Capture`) moves between threads.
const _: fn() = || {
    fn send<T: Send>() {}
    send::<Capture>();
};
