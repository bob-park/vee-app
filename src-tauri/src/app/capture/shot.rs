//! Grabs the selected part of the screen at its native resolution. The overlay must be hidden first.

use super::geometry::{MonitorInfo, Selection};
use crate::shared::error::{AppError, AppResult};
use image::DynamicImage;

/// macOS: `screencapture` into a temp PNG (points in; Retina pixels out), read back, then deleted.
#[cfg(target_os = "macos")]
pub fn grab(m: &MonitorInfo, sel: &Selection) -> AppResult<DynamicImage> {
    use super::geometry::to_global;
    let nanos = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_nanos());
    let tmp = std::env::temp_dir().join(format!("vee-capture-{}-{nanos}.png", std::process::id()));
    let mut cmd = std::process::Command::new("/usr/sbin/screencapture");
    cmd.arg("-x");
    match *sel {
        Selection::Rect { x, y, width, height } => {
            let (gx, gy, w, h) = to_global((x, y, width, height), m);
            cmd.arg("-R").arg(format!("{},{},{},{}", gx.round(), gy.round(), w.round().max(1.0), h.round().max(1.0)));
        }
        // `-o`: no window shadow.
        Selection::Window { id } => {
            cmd.args(["-o", "-l"]).arg(id.to_string());
        }
    }
    let status = cmd.arg(&tmp).status()?;
    let img = if status.success() {
        image::open(&tmp).map_err(|e| AppError::Other(e.to_string()))
    } else {
        Err(AppError::Other(format!("screencapture failed: {status}")))
    };
    let _ = std::fs::remove_file(&tmp);
    img
}

/// Windows: GDI `BitBlt` from the screen DC, in physical virtual-desktop pixels.
/// Not compiled on the macOS dev box; checked only by the Windows build.
#[cfg(windows)]
pub fn grab(m: &MonitorInfo, sel: &Selection) -> AppResult<DynamicImage> {
    use ::windows::Win32::Foundation::{HWND, RECT};
    use ::windows::Win32::Graphics::Dwm::{DWMWA_EXTENDED_FRAME_BOUNDS, DwmGetWindowAttribute};
    use ::windows::Win32::Graphics::Gdi::{
        BI_RGB, BITMAPINFO, BITMAPINFOHEADER, BitBlt, CreateCompatibleBitmap, CreateCompatibleDC, DIB_RGB_COLORS, DeleteDC, DeleteObject,
        GetDC, GetDIBits, ReleaseDC, SRCCOPY, SelectObject,
    };

    let (x, y, w, h) = match *sel {
        // The monitor's physical origin plus the local rect scaled, not `to_physical` (logical origin × scale).
        Selection::Rect { x, y, width, height } => {
            let px = |v: f64| (v * m.scale).round() as i32;
            (px(m.x) + px(x), px(m.y) + px(y), px(width).max(1), px(height).max(1))
        }
        Selection::Window { id } => {
            let mut r = RECT::default();
            unsafe {
                DwmGetWindowAttribute(
                    HWND(id as usize as *mut _),
                    DWMWA_EXTENDED_FRAME_BOUNDS,
                    &mut r as *mut _ as *mut _,
                    std::mem::size_of::<RECT>() as u32,
                )
            }
            .map_err(|e| AppError::Other(e.to_string()))?;
            (r.left, r.top, (r.right - r.left).max(1), (r.bottom - r.top).max(1))
        }
    };
    let mut buf = vec![0u8; (w * h * 4) as usize];
    // SAFETY: every GDI object made here is released before returning; `buf` holds w×h 32-bit pixels.
    let copied = unsafe {
        let screen = GetDC(None);
        let mem = CreateCompatibleDC(Some(screen));
        let bmp = CreateCompatibleBitmap(screen, w, h);
        let old = SelectObject(mem, bmp.into());
        let blit = BitBlt(mem, 0, 0, w, h, Some(screen), x, y, SRCCOPY);
        let mut info = BITMAPINFO {
            bmiHeader: BITMAPINFOHEADER {
                biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                biWidth: w,
                biHeight: -h, // top-down rows
                biPlanes: 1,
                biBitCount: 32,
                biCompression: BI_RGB.0,
                ..Default::default()
            },
            ..Default::default()
        };
        let lines = GetDIBits(mem, bmp, 0, h as u32, Some(buf.as_mut_ptr() as *mut _), &mut info, DIB_RGB_COLORS);
        SelectObject(mem, old);
        let _ = DeleteObject(bmp.into());
        let _ = DeleteDC(mem);
        ReleaseDC(None, screen);
        blit.is_ok() && lines == h
    };
    if !copied {
        return Err(AppError::Other("screen grab failed".into()));
    }
    for px in buf.as_chunks_mut::<4>().0 {
        px.swap(0, 2); // BGRA → RGBA
        px[3] = 255;
    }
    image::RgbaImage::from_raw(w as u32, h as u32, buf)
        .map(DynamicImage::ImageRgba8)
        .ok_or_else(|| AppError::Other("screen grab failed".into()))
}

#[cfg(not(any(target_os = "macos", windows)))]
pub fn grab(_m: &MonitorInfo, _sel: &Selection) -> AppResult<DynamicImage> {
    Err(AppError::Other("screen capture is not supported on this OS".into()))
}
