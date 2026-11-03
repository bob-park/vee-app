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

/// Windows, in physical virtual-desktop pixels. A region: GDI `BitBlt` from the screen DC. A window: `PrintWindow`
/// draws the window alone (windows in front of it stay out), cropped to its visible frame; the screen grab of
/// its frame is the fallback.
/// Not compiled on the macOS dev box; checked only by the Windows build.
#[cfg(windows)]
pub fn grab(m: &MonitorInfo, sel: &Selection) -> AppResult<DynamicImage> {
    use ::windows::Win32::Foundation::{HWND, RECT};
    use ::windows::Win32::Graphics::Dwm::{DWMWA_EXTENDED_FRAME_BOUNDS, DwmGetWindowAttribute};
    use ::windows::Win32::UI::WindowsAndMessaging::GetWindowRect;

    let (x, y, w, h) = match *sel {
        // The monitor's physical origin plus the local rect scaled, not `to_physical` (logical origin × scale).
        Selection::Rect { x, y, width, height } => {
            let px = |v: f64| (v * m.scale).round() as i32;
            (px(m.x) + px(x), px(m.y) + px(y), px(width).max(1), px(height).max(1))
        }
        Selection::Window { id } => {
            let hwnd = HWND(id as usize as *mut _);
            let mut r = RECT::default();
            unsafe {
                DwmGetWindowAttribute(hwnd, DWMWA_EXTENDED_FRAME_BOUNDS, &mut r as *mut _ as *mut _, std::mem::size_of::<RECT>() as u32)
            }
            .map_err(|e| AppError::Other(e.to_string()))?;
            let mut outer = RECT::default();
            // SAFETY: `hwnd` is a window id from the overlay's list; a stale one only fails the call.
            if unsafe { GetWindowRect(hwnd, &mut outer) }.is_ok() {
                let full = (outer.right - outer.left, outer.bottom - outer.top);
                // The frame inside the window rect (which also holds the invisible resize borders).
                let crop = (r.left - outer.left, r.top - outer.top, r.right - r.left, r.bottom - r.top);
                if let Some(img) = print_window(hwnd, full, crop) {
                    return Ok(img);
                }
            }
            (r.left, r.top, (r.right - r.left).max(1), (r.bottom - r.top).max(1))
        }
    };
    screen_grab(x, y, w, h).ok_or_else(|| AppError::Other("screen grab failed".into()))
}

/// `PrintWindow(PW_RENDERFULLCONTENT)` of `hwnd` into a `full`-sized bitmap, cropped to `(x, y, w, h)`.
#[cfg(windows)]
fn print_window(hwnd: ::windows::Win32::Foundation::HWND, full: (i32, i32), (x, y, w, h): (i32, i32, i32, i32)) -> Option<DynamicImage> {
    use ::windows::Win32::Graphics::Gdi::{
        CreateCompatibleBitmap, CreateCompatibleDC, DeleteDC, DeleteObject, GetDC, ReleaseDC, SelectObject,
    };
    use ::windows::Win32::Storage::Xps::{PRINT_WINDOW_FLAGS, PrintWindow};
    /// Draws DirectComposition / GPU content too (Windows 8.1+), not a black box.
    const PW_RENDERFULLCONTENT: PRINT_WINDOW_FLAGS = PRINT_WINDOW_FLAGS(2);

    let (fw, fh) = full;
    if fw <= 0 || fh <= 0 || w <= 0 || h <= 0 || x < 0 || y < 0 || x + w > fw || y + h > fh {
        return None;
    }
    // SAFETY: every GDI object made here is released before returning.
    let pixels = unsafe {
        let screen = GetDC(None);
        let mem = CreateCompatibleDC(Some(screen));
        let bmp = CreateCompatibleBitmap(screen, fw, fh);
        let old = SelectObject(mem, bmp.into());
        let printed = PrintWindow(hwnd, mem, PW_RENDERFULLCONTENT).as_bool();
        let pixels = if printed { read_bitmap(mem, bmp, fw, fh) } else { None };
        SelectObject(mem, old);
        let _ = DeleteObject(bmp.into());
        let _ = DeleteDC(mem);
        ReleaseDC(None, screen);
        pixels
    }?;
    let full = image::RgbaImage::from_raw(fw as u32, fh as u32, pixels)?;
    Some(DynamicImage::ImageRgba8(image::imageops::crop_imm(&full, x as u32, y as u32, w as u32, h as u32).to_image()))
}

/// GDI `BitBlt` of the screen rect `(x, y, w, h)`.
#[cfg(windows)]
fn screen_grab(x: i32, y: i32, w: i32, h: i32) -> Option<DynamicImage> {
    use ::windows::Win32::Graphics::Gdi::{
        BitBlt, CreateCompatibleBitmap, CreateCompatibleDC, DeleteDC, DeleteObject, GetDC, ReleaseDC, SRCCOPY, SelectObject,
    };
    // SAFETY: every GDI object made here is released before returning.
    let pixels = unsafe {
        let screen = GetDC(None);
        let mem = CreateCompatibleDC(Some(screen));
        let bmp = CreateCompatibleBitmap(screen, w, h);
        let old = SelectObject(mem, bmp.into());
        let blit = BitBlt(mem, 0, 0, w, h, Some(screen), x, y, SRCCOPY);
        let pixels = if blit.is_ok() { read_bitmap(mem, bmp, w, h) } else { None };
        SelectObject(mem, old);
        let _ = DeleteObject(bmp.into());
        let _ = DeleteDC(mem);
        ReleaseDC(None, screen);
        pixels
    }?;
    image::RgbaImage::from_raw(w as u32, h as u32, pixels).map(DynamicImage::ImageRgba8)
}

/// The `w`×`h` bitmap `bmp` (selected into `mem`) as opaque RGBA rows, top-down.
/// SAFETY: `mem` is a live memory DC and `bmp` a live `w`×`h` bitmap.
#[cfg(windows)]
unsafe fn read_bitmap(
    mem: ::windows::Win32::Graphics::Gdi::HDC,
    bmp: ::windows::Win32::Graphics::Gdi::HBITMAP,
    w: i32,
    h: i32,
) -> Option<Vec<u8>> {
    use ::windows::Win32::Graphics::Gdi::{BI_RGB, BITMAPINFO, BITMAPINFOHEADER, DIB_RGB_COLORS, GetDIBits};
    let mut buf = vec![0u8; (w * h * 4) as usize];
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
    let lines = unsafe { GetDIBits(mem, bmp, 0, h as u32, Some(buf.as_mut_ptr() as *mut _), &mut info, DIB_RGB_COLORS) };
    if lines != h {
        return None;
    }
    for px in buf.as_chunks_mut::<4>().0 {
        px.swap(0, 2); // BGRA → RGBA
        px[3] = 255;
    }
    Some(buf)
}

#[cfg(not(any(target_os = "macos", windows)))]
pub fn grab(_m: &MonitorInfo, _sel: &Selection) -> AppResult<DynamicImage> {
    Err(AppError::Other("screen capture is not supported on this OS".into()))
}
