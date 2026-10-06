//! Which app is in front, its icon, and (macOS) re-activating it.

pub struct FrontApp {
    /// macOS bundle identifier, or the full exe path on Windows.
    pub bundle_id: String,
    pub name: String,
    /// .app bundle path (macOS) or exe path (Windows); used to load the icon.
    pub path: String,
    pub pid: i32,
}

#[cfg(target_os = "macos")]
pub fn frontmost() -> Option<FrontApp> {
    use objc2_app_kit::NSWorkspace;
    let app = NSWorkspace::sharedWorkspace().frontmostApplication()?;
    let bundle_id = app.bundleIdentifier()?.to_string();
    let name = app.localizedName().map(|n| n.to_string()).unwrap_or_else(|| bundle_id.clone());
    let path = app.bundleURL().and_then(|u| u.path()).map(|p| p.to_string()).unwrap_or_default();
    Some(FrontApp { bundle_id, name, path, pid: app.processIdentifier() })
}

#[cfg(target_os = "macos")]
pub fn icon_png(app: &FrontApp) -> Option<Vec<u8>> {
    use objc2::AnyThread;
    use objc2_app_kit::{NSBitmapImageFileType, NSBitmapImageRep, NSWorkspace};
    use objc2_foundation::{NSDictionary, NSPoint, NSRect, NSSize, NSString};
    if app.path.is_empty() {
        return None;
    }
    let icon = NSWorkspace::sharedWorkspace().iconForFile(&NSString::from_str(&app.path));
    // Let AppKit pick the representation nearest 64pt; the full multi-size TIFF can be
    // tens of MB with float samples the `image` crate cannot decode.
    let mut rect = NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(64.0, 64.0));
    let cg = unsafe { icon.CGImageForProposedRect_context_hints(&mut rect, None, None) }?;
    let rep = NSBitmapImageRep::initWithCGImage(NSBitmapImageRep::alloc(), &cg);
    let png = unsafe { rep.representationUsingType_properties(NSBitmapImageFileType::PNG, &NSDictionary::new()) }?;
    Some(png.to_vec())
}

#[cfg(target_os = "macos")]
pub fn activate(pid: i32) {
    use objc2_app_kit::{NSApplicationActivationOptions, NSRunningApplication};
    if let Some(app) = NSRunningApplication::runningApplicationWithProcessIdentifier(pid) {
        app.activateWithOptions(NSApplicationActivationOptions::empty());
    }
}

#[cfg(target_os = "macos")]
pub fn play_copy_sound() {
    use objc2_app_kit::NSSound;
    use objc2_foundation::NSString;
    if let Some(sound) = NSSound::soundNamed(&NSString::from_str("Pop")) {
        sound.play();
    }
}

#[cfg(windows)]
pub fn play_copy_sound() {
    use ::windows::Win32::System::Diagnostics::Debug::MessageBeep;
    use ::windows::Win32::UI::WindowsAndMessaging::MB_OK;
    let _ = unsafe { MessageBeep(MB_OK) };
}

#[cfg(windows)]
pub fn frontmost() -> Option<FrontApp> {
    use ::windows::Win32::Foundation::CloseHandle;
    use ::windows::Win32::System::Threading::{
        OpenProcess, PROCESS_NAME_WIN32, PROCESS_QUERY_LIMITED_INFORMATION, QueryFullProcessImageNameW,
    };
    use ::windows::Win32::UI::WindowsAndMessaging::{GetForegroundWindow, GetWindowThreadProcessId};
    use ::windows::core::PWSTR;
    unsafe {
        let hwnd = GetForegroundWindow();
        if hwnd.is_invalid() {
            return None;
        }
        let mut pid = 0u32;
        GetWindowThreadProcessId(hwnd, Some(&mut pid));
        if pid == 0 {
            return None;
        }
        let handle = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid).ok()?;
        let mut buf = [0u16; 1024];
        let mut len = buf.len() as u32;
        let queried = QueryFullProcessImageNameW(handle, PROCESS_NAME_WIN32, PWSTR(buf.as_mut_ptr()), &mut len);
        let _ = CloseHandle(handle);
        queried.ok()?;
        let path = String::from_utf16_lossy(&buf[..len as usize]);
        let name = std::path::Path::new(&path).file_stem()?.to_string_lossy().into_owned();
        Some(FrontApp { bundle_id: path.clone(), name, path, pid: pid as i32 })
    }
}

#[cfg(windows)]
pub fn icon_png(app: &FrontApp) -> Option<Vec<u8>> {
    let icon = windows_icons::get_icon_by_path(&app.path).ok()?;
    let icon = image::imageops::thumbnail(&icon, 64, 64);
    let mut out = std::io::Cursor::new(Vec::new());
    icon.write_to(&mut out, image::ImageFormat::Png).ok()?;
    Some(out.into_inner())
}

/// Windows hands focus back to the previous window when ours hides, so nothing to do.
#[cfg(windows)]
pub fn activate(_pid: i32) {}

#[cfg(test)]
mod tests {
    #[test]
    #[ignore = "needs a desktop session"]
    fn reads_frontmost_app_and_icon() {
        let app = super::frontmost().expect("frontmost app");
        assert!(!app.name.is_empty());
        let png = super::icon_png(&app).expect("icon");
        assert!(png.starts_with(&[0x89, b'P', b'N', b'G']));
    }
}
