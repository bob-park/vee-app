//! The folder under the cursor during a drag out of the panel, and the convert modifier key.
//! Points are in the window-positioning space (`window::cursor_point`): macOS points from the
//! primary display's top-left, Windows physical virtual-desktop pixels.

use std::path::PathBuf;

/// What a drop at the cursor would land in.
#[derive(Debug, PartialEq)]
pub enum DropFolder {
    Folder(PathBuf),
    NotFolder,
    /// A Finder window is under the cursor but Vee may not ask Finder for its folder.
    FinderBlocked,
}

/// Whether Vee may send Apple Events to Finder.
#[derive(Debug, PartialEq)]
pub enum FinderAccess {
    Granted,
    Denied,
    /// Not asked yet, or Finder isn't running.
    Unknown,
}

/// An on-screen window, front to back as the OS lists them.
pub struct WinInfo {
    pub pid: i32,
    pub layer: i64,
    /// x, y, w, h
    pub bounds: (f64, f64, f64, f64),
    pub finder: bool,
}

type Bounds = (f64, f64, f64, f64);

#[derive(Debug, PartialEq)]
pub enum Hit {
    Desktop,
    /// A Finder window, with its on-screen bounds.
    Finder(Bounds),
    Other,
}

fn contains((x, y, w, h): (f64, f64, f64, f64), (px, py): (f64, f64)) -> bool {
    px >= x && px < x + w && py >= y && py < y + h
}

/// Vee's own windows below this CG layer (kCGDraggingWindowLevel) block a drop; the drag image sits at or above it.
const DRAG_LAYER: i64 = 500;

/// Whether `w` can be what a drop at `point` lands on: a normal-layer window, or one of Vee's own
/// (always-on-top panel and toast sit above layer 0).
fn counts(w: &WinInfo, point: (f64, f64), own_pid: i32) -> bool {
    (w.layer == 0 || (w.pid == own_pid && w.layer < DRAG_LAYER)) && contains(w.bounds, point)
}

/// The front window under `point` that a drop would land on; none means the desktop.
pub fn pick_hit(windows: &[WinInfo], point: (f64, f64), own_pid: i32) -> Hit {
    match windows.iter().find(|w| counts(w, point, own_pid)) {
        None => Hit::Desktop,
        Some(w) if w.pid != own_pid && w.finder => Hit::Finder(w.bounds),
        Some(_) => Hit::Other,
    }
}

/// The folder of the Finder window that is the on-screen window `hit`: the one with the same bounds
/// (within 1 pt), else the first containing `point`. Finder lists windows on other Spaces too.
pub fn pick_finder_window(windows: Vec<(Bounds, PathBuf)>, hit: Bounds, point: (f64, f64)) -> Option<PathBuf> {
    let same = |(x, y, w, h): Bounds| [x - hit.0, y - hit.1, w - hit.2, h - hit.3].iter().all(|d| d.abs() <= 1.0);
    let at = windows.iter().position(|(b, _)| same(*b)).or_else(|| windows.iter().position(|(b, _)| contains(*b, point)))?;
    windows.into_iter().nth(at).map(|(_, p)| p)
}

/// Parses `left,top,right,bottom\t/abs/path/` lines from the Finder script into (x, y, w, h) and path.
pub fn parse_finder_windows(out: &str) -> Vec<((f64, f64, f64, f64), PathBuf)> {
    out.lines()
        .filter_map(|line| {
            let (bounds, path) = line.split_once('\t')?;
            if !path.starts_with('/') {
                return None;
            }
            let n: Vec<f64> = bounds.split(',').map(|v| v.trim().parse().ok()).collect::<Option<_>>()?;
            let [l, t, r, b] = n[..] else { return None };
            Some(((l, t, r - l, b - t), PathBuf::from(path)))
        })
        .collect()
}

#[cfg(target_os = "macos")]
pub fn key_down(key: &str) -> bool {
    use objc2_core_graphics::{CGEventFlags, CGEventSource, CGEventSourceStateID};
    let mask = match key {
        "alt" => CGEventFlags::MaskAlternate,
        "ctrl" => CGEventFlags::MaskControl,
        "shift" => CGEventFlags::MaskShift,
        _ => return false,
    };
    CGEventSource::flags_state(CGEventSourceStateID::CombinedSessionState).contains(mask)
}

#[cfg(target_os = "macos")]
fn on_screen_windows() -> Vec<WinInfo> {
    use objc2_core_foundation::{CFDictionary, CFNumber, CFRetained, CFString, CFType, CGRect};
    use objc2_core_graphics::{
        CGRectMakeWithDictionaryRepresentation, CGWindowListCopyWindowInfo, CGWindowListOption, kCGNullWindowID,
        kCGWindowBounds, kCGWindowLayer, kCGWindowOwnerPID,
    };
    let Some(list) = CGWindowListCopyWindowInfo(
        CGWindowListOption::OptionOnScreenOnly | CGWindowListOption::ExcludeDesktopElements,
        kCGNullWindowID,
    ) else {
        return Vec::new();
    };
    // SAFETY: CGWindowListCopyWindowInfo returns an array of CFDictionary<CFString, CFType>.
    let list: CFRetained<objc2_core_foundation::CFArray<CFDictionary<CFString, CFType>>> =
        unsafe { CFRetained::cast_unchecked(list) };
    // SAFETY: the key constants are immutable statics provided by CoreGraphics.
    let (pid_key, layer_key, bounds_key) = unsafe { (kCGWindowOwnerPID, kCGWindowLayer, kCGWindowBounds) };
    let number = |d: &CFDictionary<CFString, CFType>, k: &CFString| d.get(k)?.downcast::<CFNumber>().ok()?.as_i64();
    list.iter()
        .filter_map(|d| {
            let pid = number(&d, pid_key)? as i32;
            let layer = number(&d, layer_key)?;
            let dict = d.get(bounds_key)?.downcast::<CFDictionary>().ok()?;
            let mut r = CGRect::default();
            // SAFETY: `dict` is the window bounds dictionary and `r` is a valid CGRect to write into.
            if !unsafe { CGRectMakeWithDictionaryRepresentation(Some(&dict), &mut r) } {
                return None;
            }
            Some(WinInfo { pid, layer, bounds: (r.origin.x, r.origin.y, r.size.width, r.size.height), finder: false })
        })
        .collect()
}

#[cfg(target_os = "macos")]
const FINDER_WINDOWS_SCRIPT: &str = r#"tell application "Finder"
  set out to ""
  -- By index: looping over the window list makes w "item 1 of …", which Finder reads as a file in the window.
  repeat with i from 1 to (count of Finder windows)
    set w to Finder window i
    try
      if collapsed of w then error number -128
      set b to bounds of w
      set out to out & (item 1 of b) & "," & (item 2 of b) & "," & (item 3 of b) & "," & (item 4 of b) & tab & (POSIX path of (target of w as alias)) & linefeed
    end try
  end repeat
  return out
end tell"#;

#[cfg(target_os = "macos")]
fn is_finder(pid: i32) -> bool {
    use objc2_app_kit::NSRunningApplication;
    // Called off the main thread, where no pool drains the autoreleased objects.
    objc2::rc::autoreleasepool(|_| {
        NSRunningApplication::runningApplicationWithProcessIdentifier(pid)
            .and_then(|a| a.bundleIdentifier())
            .is_some_and(|b| b.to_string() == "com.apple.finder")
    })
}

#[cfg(target_os = "macos")]
pub fn folder_at(point: (f64, f64)) -> DropFolder {
    let own_pid = std::process::id() as i32;
    let mut windows = on_screen_windows();
    // Only the window the drop lands on needs its bundle id.
    if let Some(w) = windows.iter_mut().find(|w| counts(w, point, own_pid)) {
        w.finder = w.pid != own_pid && is_finder(w.pid);
    }
    match pick_hit(&windows, point, own_pid) {
        Hit::Desktop => match std::env::home_dir() {
            Some(home) => DropFolder::Folder(home.join("Desktop")),
            None => DropFolder::NotFolder,
        },
        Hit::Other => DropFolder::NotFolder,
        Hit::Finder(hit) => {
            if finder_access(false) != Some(FinderAccess::Granted) {
                return DropFolder::FinderBlocked;
            }
            // ponytail: osascript process per lookup (~100 ms); move to NSAppleScript on a dedicated thread if it feels slow.
            let out = match std::process::Command::new("osascript").args(["-e", FINDER_WINDOWS_SCRIPT]).output() {
                Ok(out) if out.status.success() => out,
                Ok(out) => {
                    log::warn!("finder windows script failed: {}", String::from_utf8_lossy(&out.stderr).trim());
                    return DropFolder::NotFolder;
                }
                Err(e) => {
                    log::warn!("osascript failed to start: {e}");
                    return DropFolder::NotFolder;
                }
            };
            pick_finder_window(parse_finder_windows(&String::from_utf8_lossy(&out.stdout)), hit, point)
                .map_or(DropFolder::NotFolder, DropFolder::Folder)
        }
    }
}

/// Asks the Apple Events permission state for Finder; `ask` shows the system prompt when not yet decided.
/// Blocks while the prompt is up, so never call it with `ask` on the main thread.
#[cfg(target_os = "macos")]
pub fn finder_access(ask: bool) -> Option<FinderAccess> {
    use std::ffi::c_void;
    #[repr(C)]
    struct AEDesc {
        descriptor_type: u32,
        data_handle: *mut c_void,
    }
    #[link(name = "CoreServices", kind = "framework")]
    unsafe extern "C" {
        fn AECreateDesc(type_code: u32, data_ptr: *const c_void, data_size: isize, result: *mut AEDesc) -> i16;
        fn AEDisposeDesc(desc: *mut AEDesc) -> i16;
        fn AEDeterminePermissionToAutomateTarget(target: *const AEDesc, class: u32, id: u32, ask: u8) -> i32;
    }
    const TYPE_APPLICATION_BUNDLE_ID: u32 = u32::from_be_bytes(*b"bund");
    const TYPE_WILD_CARD: u32 = u32::from_be_bytes(*b"****");
    const BUNDLE: &[u8] = b"com.apple.finder";
    let mut desc = AEDesc { descriptor_type: 0, data_handle: std::ptr::null_mut() };
    // SAFETY: BUNDLE outlives the call (AECreateDesc copies it) and `desc` is a valid out pointer.
    if unsafe { AECreateDesc(TYPE_APPLICATION_BUNDLE_ID, BUNDLE.as_ptr().cast(), BUNDLE.len() as isize, &mut desc) } != 0 {
        return Some(FinderAccess::Unknown);
    }
    // SAFETY: `desc` was created above and is disposed exactly once after the check.
    let status = unsafe {
        let status = AEDeterminePermissionToAutomateTarget(&desc, TYPE_WILD_CARD, TYPE_WILD_CARD, ask as u8);
        AEDisposeDesc(&mut desc);
        status
    };
    Some(match status {
        0 => FinderAccess::Granted,
        -1743 => FinderAccess::Denied,
        _ => FinderAccess::Unknown,
    })
}

#[cfg(windows)]
pub fn key_down(key: &str) -> bool {
    use ::windows::Win32::UI::Input::KeyboardAndMouse::{GetAsyncKeyState, VK_CONTROL, VK_MENU, VK_SHIFT};
    let vk = match key {
        "alt" => VK_MENU,
        "ctrl" => VK_CONTROL,
        "shift" => VK_SHIFT,
        _ => return false,
    };
    // SAFETY: GetAsyncKeyState only reads global key state.
    (unsafe { GetAsyncKeyState(vk.0 as i32) } as u16 & 0x8000) != 0
}

#[cfg(windows)]
pub fn finder_access(_ask: bool) -> Option<FinderAccess> {
    None
}

#[cfg(windows)]
pub fn folder_at(point: (f64, f64)) -> DropFolder {
    use ::windows::Win32::Foundation::POINT;
    use ::windows::Win32::UI::WindowsAndMessaging::{GA_ROOT, GetAncestor, GetClassNameW, GetWindowThreadProcessId, WindowFromPoint};
    // SAFETY: plain user32 queries on whatever window is under the point; a stale HWND only makes them fail.
    unsafe {
        let hwnd = WindowFromPoint(POINT { x: point.0.round() as i32, y: point.1.round() as i32 });
        if hwnd.is_invalid() {
            return DropFolder::NotFolder;
        }
        let root = GetAncestor(hwnd, GA_ROOT);
        let mut pid = 0u32;
        GetWindowThreadProcessId(root, Some(&mut pid));
        if pid == std::process::id() {
            return DropFolder::NotFolder;
        }
        let mut buf = [0u16; 256];
        let len = GetClassNameW(root, &mut buf).max(0) as usize;
        let found = match String::from_utf16_lossy(&buf[..len]).as_str() {
            "Progman" | "WorkerW" => windows_desktop(),
            "CabinetWClass" => explorer_folder(root),
            _ => return DropFolder::NotFolder,
        };
        found.map_or(DropFolder::NotFolder, DropFolder::Folder)
    }
}

#[cfg(windows)]
fn windows_desktop() -> Option<PathBuf> {
    use ::windows::Win32::System::Com::CoTaskMemFree;
    use ::windows::Win32::UI::Shell::{FOLDERID_Desktop, KF_FLAG_DEFAULT, SHGetKnownFolderPath};
    // SAFETY: the returned string is read once and freed with CoTaskMemFree as the API requires.
    unsafe {
        let p = SHGetKnownFolderPath(&FOLDERID_Desktop, KF_FLAG_DEFAULT, None).ok()?;
        let path = p.to_string().ok();
        CoTaskMemFree(Some(p.0 as *const _));
        path.map(PathBuf::from)
    }
}

/// The folder shown in the active tab of the Explorer window `root`.
#[cfg(windows)]
fn explorer_folder(root: ::windows::Win32::Foundation::HWND) -> Option<PathBuf> {
    use ::windows::Win32::System::Com::{
        CLSCTX_ALL, COINIT_APARTMENTTHREADED, CoCreateInstance, CoInitializeEx, CoTaskMemFree, CoUninitialize, IServiceProvider,
    };
    use ::windows::Win32::System::Variant::{VARIANT, VT_I4};
    use ::windows::Win32::UI::Shell::{
        IFolderView, IPersistFolder2, IShellBrowser, IShellWindows, IWebBrowserApp, SHGetPathFromIDListW, SID_STopLevelBrowser,
        ShellWindows,
    };
    use ::windows::Win32::UI::WindowsAndMessaging::FindWindowExW;
    use ::windows::core::{Interface, PCWSTR, w};
    // SAFETY: COM is initialised for this thread for the duration of the lookup and released after;
    // every interface is dropped before CoUninitialize, and the PIDL is freed with CoTaskMemFree.
    unsafe {
        let init = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
        let result = (|| -> Option<PathBuf> {
            let windows: IShellWindows = CoCreateInstance(&ShellWindows, None, CLSCTX_ALL).ok()?;
            // Explorer keeps the active tab first among the window's tab children.
            let active_tab = FindWindowExW(Some(root), None, w!("ShellTabWindowClass"), PCWSTR::null()).ok();
            for i in 0..windows.Count().ok()? {
                let mut index = VARIANT::default();
                (*index.Anonymous.Anonymous).vt = VT_I4;
                (*index.Anonymous.Anonymous).Anonymous.lVal = i;
                let Ok(item) = windows.Item(&index) else { continue };
                let Ok(app) = item.cast::<IWebBrowserApp>() else { continue };
                if !app.HWND().is_ok_and(|h| h.0 == root.0 as isize) {
                    continue;
                }
                let Ok(browser) = item
                    .cast::<IServiceProvider>()
                    .and_then(|sp| sp.QueryService::<IShellBrowser>(&SID_STopLevelBrowser))
                else {
                    continue;
                };
                if active_tab.is_some_and(|tab| browser.GetWindow().ok() != Some(tab)) {
                    continue;
                }
                let folder: IPersistFolder2 = browser.QueryActiveShellView().ok()?.cast::<IFolderView>().ok()?.GetFolder().ok()?;
                let pidl = folder.GetCurFolder().ok()?;
                let mut buf = [0u16; 260];
                let ok = SHGetPathFromIDListW(pidl, &mut buf).as_bool();
                CoTaskMemFree(Some(pidl as *const _));
                if !ok {
                    return None;
                }
                let len = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
                return Some(PathBuf::from(String::from_utf16_lossy(&buf[..len])));
            }
            None
        })();
        if init.is_ok() {
            CoUninitialize();
        }
        result
    }
}

/// `"granted" | "denied" | "unknown"`; `None` where the permission doesn't exist (Windows).
#[tauri::command]
pub fn get_finder_access() -> Option<String> {
    finder_access(false).map(|a| {
        match a {
            FinderAccess::Granted => "granted",
            FinderAccess::Denied => "denied",
            FinderAccess::Unknown => "unknown",
        }
        .to_string()
    })
}

/// Shows the system prompt off the main thread (it blocks), then tells the settings to recheck.
#[tauri::command]
pub fn request_finder_access(app: tauri::AppHandle) {
    use tauri::Emitter;
    std::thread::spawn(move || {
        finder_access(true);
        let _ = app.emit("settings://changed", ());
    });
}

#[tauri::command]
pub fn open_automation_settings() -> crate::shared::error::AppResult<()> {
    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("open")
            .arg("x-apple.systempreferences:com.apple.preference.security?Privacy_Automation")
            .spawn()?;
        Ok(())
    }
    #[cfg(not(target_os = "macos"))]
    Err(crate::shared::error::AppError::Other("Automation settings are macOS only".into()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn w(pid: i32, layer: i64, b: (f64, f64, f64, f64), finder: bool) -> WinInfo {
        WinInfo { pid, layer, bounds: b, finder }
    }

    #[test]
    fn nothing_under_the_point_is_the_desktop() {
        assert!(matches!(pick_hit(&[w(9, 0, (0., 0., 10., 10.), false)], (50., 50.), 1), Hit::Desktop));
    }

    #[test]
    fn front_window_wins_and_other_layers_are_ignored() {
        let ws = [w(5, 25, (0., 0., 100., 100.), false), w(7, 0, (0., 0., 100., 100.), true), w(8, 0, (0., 0., 100., 100.), false)];
        assert_eq!(pick_hit(&ws, (10., 10.), 1), Hit::Finder((0., 0., 100., 100.)));
    }

    #[test]
    fn own_windows_are_not_folders() {
        let ws = [w(1, 0, (0., 0., 100., 100.), false), w(7, 0, (0., 0., 100., 100.), true)];
        assert!(matches!(pick_hit(&ws, (10., 10.), 1), Hit::Other));
    }

    #[test]
    fn own_always_on_top_window_is_not_a_folder() {
        let ws = [w(1, 3, (0., 0., 100., 100.), false), w(7, 0, (0., 0., 100., 100.), true)];
        assert!(matches!(pick_hit(&ws, (10., 10.), 1), Hit::Other));
    }

    #[test]
    fn own_drag_image_is_skipped() {
        let ws = [w(1, 500, (0., 0., 100., 100.), false), w(7, 0, (0., 0., 100., 100.), true)];
        assert!(matches!(pick_hit(&ws, (10., 10.), 1), Hit::Finder(_)));
    }

    #[test]
    fn finder_lines_without_a_path_are_skipped() {
        let out = "10,20,410,320\t/Users/me/Downloads/\n0,0,5,5\t\n0,0,5,5\tmissing value\n";
        assert_eq!(parse_finder_windows(out), vec![((10., 20., 400., 300.), PathBuf::from("/Users/me/Downloads/"))]);
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn finder_script_reads_windows_by_index() {
        // `repeat with w in every Finder window` resolves w to a file inside the window; every line failed.
        assert!(!FINDER_WINDOWS_SCRIPT.contains("repeat with w in"));
        assert!(FINDER_WINDOWS_SCRIPT.contains("set w to Finder window i"));
    }

    #[test]
    fn finder_window_matching_the_hit_bounds_wins() {
        // First listed: another Space, overlapping the point; second: the window on screen, 1 pt off.
        let out = "0,0,800,600\t/Users/me/Other/
100,100,500,400\t/Users/me/Here/
";
        let ws = parse_finder_windows(out);
        assert_eq!(pick_finder_window(ws.clone(), (100.5, 99., 401., 300.), (200., 200.)), Some(PathBuf::from("/Users/me/Here/")));
        // No bounds match: the first window containing the point.
        assert_eq!(pick_finder_window(ws.clone(), (300., 300., 50., 50.), (200., 200.)), Some(PathBuf::from("/Users/me/Other/")));
        assert_eq!(pick_finder_window(ws, (300., 300., 50., 50.), (900., 900.)), None);
    }

    #[cfg(target_os = "macos")]
    fn cursor() -> (f64, f64) {
        use objc2_core_graphics::{CGEvent, CGEventCreate};
        let p = CGEvent::location(CGEventCreate(None).as_deref());
        (p.x, p.y)
    }

    #[cfg(windows)]
    fn cursor() -> (f64, f64) {
        let mut p = ::windows::Win32::Foundation::POINT::default();
        // SAFETY: `p` is a valid out pointer.
        unsafe { ::windows::Win32::UI::WindowsAndMessaging::GetCursorPos(&mut p) }.unwrap();
        (p.x as f64, p.y as f64)
    }

    #[test]
    #[ignore = "needs a desktop session; put the cursor on the desktop or a Finder/Explorer window"]
    fn folder_at_desktop_corner() {
        let point = cursor();
        println!("finder_access = {:?}", finder_access(false));
        println!("folder_at({point:?}) = {:?}", folder_at(point));
    }
}
