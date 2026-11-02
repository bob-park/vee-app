//! Screen & System Audio Recording permission (macOS) and the OS versions recording needs.

use crate::shared::error::AppResult;

/// Lowest macOS major version that can record (ScreenCaptureKit `SCRecordingOutput`).
#[cfg(target_os = "macos")]
const MIN_MACOS: u32 = 15;
/// Lowest Windows build that can record (Windows 10 2004).
#[cfg(windows)]
const MIN_WINDOWS_BUILD: u32 = 19041;

/// Whether Vee may capture the screen. macOS only flips this after a relaunch.
pub fn granted() -> bool {
    #[cfg(target_os = "macos")]
    return objc2_core_graphics::CGPreflightScreenCaptureAccess();
    #[cfg(not(target_os = "macos"))]
    true
}

#[cfg_attr(not(any(target_os = "macos", windows, test)), allow(dead_code))]
fn version_ok(major_or_build: u32, min: u32) -> bool {
    major_or_build >= min
}

/// Whether this OS is new enough to record (capture works everywhere).
#[allow(dead_code)] // used by `app::recorder` (Task 7)
pub fn recording_supported() -> bool {
    #[cfg(target_os = "macos")]
    {
        let major = objc2_foundation::NSProcessInfo::processInfo().operatingSystemVersion().majorVersion;
        version_ok(u32::try_from(major).unwrap_or(0), MIN_MACOS)
    }
    #[cfg(windows)]
    return version_ok(windows_build(), MIN_WINDOWS_BUILD);
    #[cfg(not(any(target_os = "macos", windows)))]
    false
}

/// `CurrentBuildNumber` (REG_SZ) from the registry; 0 if unreadable.
#[cfg(windows)]
fn windows_build() -> u32 {
    use windows::{
        Win32::{
            Foundation::ERROR_SUCCESS,
            System::Registry::{HKEY_LOCAL_MACHINE, RRF_RT_REG_SZ, RegGetValueW},
        },
        core::w,
    };
    let mut buf = [0u16; 32];
    let mut len = std::mem::size_of_val(&buf) as u32;
    // SAFETY: `buf` outlives the call and `len` holds its size in bytes.
    let err = unsafe {
        RegGetValueW(
            HKEY_LOCAL_MACHINE,
            w!("SOFTWARE\\Microsoft\\Windows NT\\CurrentVersion"),
            w!("CurrentBuildNumber"),
            RRF_RT_REG_SZ,
            None,
            Some(buf.as_mut_ptr().cast()),
            Some(&mut len),
        )
    };
    if err != ERROR_SUCCESS {
        return 0;
    }
    let chars = (len as usize / 2).min(buf.len());
    String::from_utf16_lossy(&buf[..chars]).trim_end_matches('\0').parse().unwrap_or(0)
}

/// `None` where the permission doesn't exist (Windows).
#[tauri::command]
pub fn get_screen_access() -> Option<bool> {
    #[cfg(target_os = "macos")]
    return Some(granted());
    #[cfg(not(target_os = "macos"))]
    None
}

/// Shows the system prompt the first time; later calls do nothing (macOS asks once).
#[tauri::command]
pub fn request_screen_access() {
    #[cfg(target_os = "macos")]
    objc2_core_graphics::CGRequestScreenCaptureAccess();
}

#[tauri::command]
pub fn open_screen_access_settings() -> AppResult<()> {
    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("open").arg("x-apple.systempreferences:com.apple.preference.security?Privacy_ScreenCapture").spawn()?;
        Ok(())
    }
    #[cfg(not(target_os = "macos"))]
    Err(crate::shared::error::AppError::Other("Screen Recording settings are macOS only".into()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_ok_compares_inclusive() {
        assert!(version_ok(15, 15));
        assert!(!version_ok(14, 15));
        assert!(version_ok(19041, 19041));
        assert!(!version_ok(19040, 19041));
    }
}
