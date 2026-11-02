//! On-screen windows the overlay can snap to, front to back.

use serde::Serialize;

use super::geometry::MonitorInfo;

/// A window in monitor-local logical coordinates.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WinInfo {
    pub id: u64,
    pub app: String,
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

/// A window as the OS reports it, in global logical coordinates.
#[derive(Debug, Clone)]
pub struct RawWin {
    pub pid: u32,
    pub id: u64,
    pub app: String,
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

/// Drops Vee's own and off-monitor windows, clips the rest to the monitor, makes them monitor-local.
pub fn keep(raw: Vec<RawWin>, own_pid: u32, m: &MonitorInfo) -> Vec<WinInfo> {
    raw.into_iter()
        .filter(|w| w.pid != own_pid)
        .filter_map(|w| {
            let x0 = w.x.max(m.x);
            let y0 = w.y.max(m.y);
            let x1 = (w.x + w.width).min(m.x + m.width);
            let y1 = (w.y + w.height).min(m.y + m.height);
            (x1 > x0 && y1 > y0).then_some(WinInfo { id: w.id, app: w.app, x: x0 - m.x, y: y0 - m.y, width: x1 - x0, height: y1 - y0 })
        })
        .collect()
}

/// Windows on `monitor` that the user can pick, front to back.
#[allow(dead_code)] // used by `app::capture` (Task 5)
pub fn list(monitor: &MonitorInfo) -> Vec<WinInfo> {
    keep(raw_windows(monitor), std::process::id(), monitor)
}

#[cfg(target_os = "macos")]
fn raw_windows(_monitor: &MonitorInfo) -> Vec<RawWin> {
    use objc2_core_foundation::{CFArray, CFDictionary, CFNumber, CFRetained, CFString, CFType, CGRect};
    use objc2_core_graphics::{
        CGRectMakeWithDictionaryRepresentation, CGWindowListCopyWindowInfo, CGWindowListOption, kCGNullWindowID, kCGWindowBounds,
        kCGWindowLayer, kCGWindowNumber, kCGWindowOwnerName, kCGWindowOwnerPID,
    };
    let Some(list) =
        CGWindowListCopyWindowInfo(CGWindowListOption::OptionOnScreenOnly | CGWindowListOption::ExcludeDesktopElements, kCGNullWindowID)
    else {
        return Vec::new();
    };
    // SAFETY: CGWindowListCopyWindowInfo returns an array of CFDictionary<CFString, CFType>.
    let list: CFRetained<CFArray<CFDictionary<CFString, CFType>>> = unsafe { CFRetained::cast_unchecked(list) };
    // SAFETY: the key constants are immutable statics provided by CoreGraphics.
    let (number_key, pid_key, name_key, layer_key, bounds_key) =
        unsafe { (kCGWindowNumber, kCGWindowOwnerPID, kCGWindowOwnerName, kCGWindowLayer, kCGWindowBounds) };
    let number = |d: &CFDictionary<CFString, CFType>, k: &CFString| d.get(k)?.downcast::<CFNumber>().ok()?.as_i64();
    list.iter()
        .filter_map(|d| {
            if number(&d, layer_key)? != 0 {
                return None;
            }
            let bounds = d.get(bounds_key)?.downcast::<CFDictionary>().ok()?;
            let mut r = CGRect::default();
            // SAFETY: `bounds` is the window bounds dictionary and `r` is a valid CGRect to write into.
            if !unsafe { CGRectMakeWithDictionaryRepresentation(Some(&bounds), &mut r) } {
                return None;
            }
            let app = d.get(name_key).and_then(|v| v.downcast::<CFString>().ok()).map(|s| s.to_string()).unwrap_or_default();
            Some(RawWin {
                pid: number(&d, pid_key)? as u32,
                id: number(&d, number_key)? as u64,
                app,
                x: r.origin.x,
                y: r.origin.y,
                width: r.size.width,
                height: r.size.height,
            })
        })
        .collect()
}

// Not compiled on this machine (macOS dev box); checked only by the Windows build.
#[cfg(windows)]
fn raw_windows(monitor: &MonitorInfo) -> Vec<RawWin> {
    use ::windows::Win32::Foundation::{CloseHandle, HWND, LPARAM, RECT};
    use ::windows::Win32::Graphics::Dwm::{DWMWA_CLOAKED, DWMWA_EXTENDED_FRAME_BOUNDS, DwmGetWindowAttribute};
    use ::windows::Win32::System::Threading::{
        OpenProcess, PROCESS_NAME_WIN32, PROCESS_QUERY_LIMITED_INFORMATION, QueryFullProcessImageNameW,
    };
    use ::windows::Win32::UI::WindowsAndMessaging::{
        EnumWindows, GWL_EXSTYLE, GetWindowLongW, GetWindowThreadProcessId, IsWindowVisible, WS_EX_TOOLWINDOW,
    };
    use ::windows::core::{BOOL, PWSTR};

    unsafe fn exe_stem(pid: u32) -> String {
        let Ok(handle) = (unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid) }) else {
            return String::new();
        };
        let mut buf = [0u16; 1024];
        let mut len = buf.len() as u32;
        let ok = unsafe { QueryFullProcessImageNameW(handle, PROCESS_NAME_WIN32, PWSTR(buf.as_mut_ptr()), &mut len) }.is_ok();
        let _ = unsafe { CloseHandle(handle) };
        if !ok {
            return String::new();
        }
        let path = String::from_utf16_lossy(&buf[..len as usize]);
        std::path::Path::new(&path).file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default()
    }

    /// `lparam` points at the `Vec<HWND>` being filled, top of the z-order first.
    unsafe extern "system" fn visit(hwnd: HWND, lparam: LPARAM) -> BOOL {
        let out = unsafe { &mut *(lparam.0 as *mut Vec<HWND>) };
        out.push(hwnd);
        true.into()
    }

    let mut hwnds: Vec<HWND> = Vec::new();
    unsafe {
        let _ = EnumWindows(Some(visit), LPARAM(&mut hwnds as *mut _ as isize));
    }
    let scale = monitor.scale;
    hwnds
        .into_iter()
        .filter_map(|hwnd| unsafe {
            if !IsWindowVisible(hwnd).as_bool() || GetWindowLongW(hwnd, GWL_EXSTYLE) as u32 & WS_EX_TOOLWINDOW.0 != 0 {
                return None;
            }
            let mut cloaked = 0u32;
            let size = std::mem::size_of::<u32>() as u32;
            if DwmGetWindowAttribute(hwnd, DWMWA_CLOAKED, &mut cloaked as *mut _ as *mut _, size).is_ok() && cloaked != 0 {
                return None;
            }
            let mut r = RECT::default();
            DwmGetWindowAttribute(hwnd, DWMWA_EXTENDED_FRAME_BOUNDS, &mut r as *mut _ as *mut _, std::mem::size_of::<RECT>() as u32)
                .ok()?;
            let mut pid = 0u32;
            GetWindowThreadProcessId(hwnd, Some(&mut pid));
            // ponytail: divides by this monitor's scale, so windows on another mixed-DPI monitor land slightly off; they are clipped away anyway.
            Some(RawWin {
                pid,
                id: hwnd.0 as usize as u64,
                app: exe_stem(pid),
                x: r.left as f64 / scale,
                y: r.top as f64 / scale,
                width: (r.right - r.left) as f64 / scale,
                height: (r.bottom - r.top) as f64 / scale,
            })
        })
        .collect()
}

#[cfg(not(any(target_os = "macos", windows)))]
fn raw_windows(_monitor: &MonitorInfo) -> Vec<RawWin> {
    Vec::new()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn raw(pid: u32, id: u64, x: f64, y: f64, width: f64, height: f64) -> RawWin {
        RawWin { pid, id, app: format!("app{id}"), x, y, width, height }
    }

    #[test]
    fn keep_drops_own_and_offscreen_windows() {
        let m = MonitorInfo { x: 1440.0, y: 0.0, width: 1920.0, height: 1080.0, scale: 2.0 };
        let wins = vec![
            raw(1, 10, 1500.0, 100.0, 400.0, 300.0),
            raw(99, 11, 1500.0, 100.0, 400.0, 300.0), // own
            raw(2, 12, 0.0, 0.0, 800.0, 600.0),       // on the other monitor
            raw(3, 13, 3160.0, 900.0, 400.0, 400.0),  // half off the right and bottom edges
            raw(4, 14, 1600.0, 200.0, 0.0, 300.0),    // zero size
            raw(5, 15, 1440.0, 0.0, 100.0, 100.0),
        ];
        let got = keep(wins, 99, &m);
        let ids: Vec<u64> = got.iter().map(|w| w.id).collect();
        assert_eq!(ids, vec![10, 13, 15]);
        assert_eq!(got[0], WinInfo { id: 10, app: "app10".into(), x: 60.0, y: 100.0, width: 400.0, height: 300.0 });
        assert_eq!((got[1].x, got[1].y, got[1].width, got[1].height), (1720.0, 900.0, 200.0, 180.0));
        assert_eq!((got[2].x, got[2].y), (0.0, 0.0));
    }
}
