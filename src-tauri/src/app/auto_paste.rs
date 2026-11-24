//! Auto next paste: after each paste, the clipboard moves to the next card in panel order.
//!
//! Vee never pastes and never swallows the key. A listen-only key watch (installed only while
//! the `autoNextPaste` setting is on) signals a worker, which swaps the clipboard behind the paste.
//! The setting is flipped by a global shortcut (`toggle`).

use crate::{
    app::{
        state::AppState,
        window::{self, AutoPasteNote, ToastPayload},
    },
    domain::{
        clip::{self, ClipContent},
        settings,
    },
    shared::error::AppResult,
};
use rusqlite::Connection;
use std::sync::{
    Mutex, OnceLock,
    atomic::{AtomicBool, Ordering},
    mpsc::{Receiver, Sender, channel},
};
use std::time::Duration;
use tauri::{AppHandle, Manager};

/// How long the pasting app gets to read the clipboard before it is swapped.
const DELAY: Duration = Duration::from_millis(150);

/// How long our own advance keeps the watcher quiet: its 200ms poll, with room to spare.
/// Short on purpose, so a real copy made right after a paste is still recorded.
const SUPPRESS_MS: u64 = 500;
/// Failed writes before an advance gives up: one stale card is skipped, a clipboard that
/// can't be written at all (held by another app) must not walk the whole history.
const MAX_SKIPS: usize = 3;

/// The OS clipboard change count: it moves on every write by anyone, seen by the watcher or not.
pub fn clipboard_seq() -> i64 {
    platform::clipboard_seq()
}

/// Writes the first writable card after `cursor` and returns its id; `None` when nothing was written
/// (no cursor, no card left, or `MAX_SKIPS` writes failed). The db is locked only for the lookups, never across a write.
pub fn advance_with(
    db: &Mutex<Connection>,
    cursor: Option<i64>,
    mut write: impl FnMut(&ClipContent, Vec<(String, Vec<u8>)>) -> AppResult<()>,
) -> AppResult<Option<i64>> {
    let Some(mut at) = cursor else { return Ok(None) };
    let mut failed = 0;
    while failed < MAX_SKIPS {
        let (id, content, formats) = {
            let conn = db.lock().unwrap();
            let Some(id) = clip::next_after(&conn, at)? else { return Ok(None) };
            (id, clip::content(&conn, id)?, clip::formats(&conn, id)?)
        };
        // A clip deleted between the two lookups counts as unwritable.
        match content.map(|content| write(&content, formats)) {
            Some(Ok(())) => return Ok(Some(id)),
            Some(Err(e)) => {
                failed += 1;
                log::warn!("auto next paste skipped clip {id}: {e}");
            }
            None => {}
        }
        at = id;
    }
    Ok(None)
}

/// Moves the clipboard to the card after the current one. Silent: no touch, toast, sound or event.
pub fn advance(app: &AppHandle) {
    // A paste into Vee's own windows (the panel search) is not part of the sequence.
    if app.webview_windows().values().any(|w| w.is_focused().unwrap_or(false)) {
        return;
    }
    let state = app.state::<AppState>();
    let cursor = state.paste_cursor(clipboard_seq());
    match advance_with(&state.db, cursor, |content, formats| window::write_clipboard(&state, content, formats)) {
        Ok(Some(id)) => {
            state.set_paste_cursor(id, clipboard_seq());
            state.suppress_watcher_for(SUPPRESS_MS);
        }
        Ok(None) => {}
        Err(e) => log::warn!("auto next paste failed: {e}"),
    }
}

/// Waits `delay` so the pasting app reads the clipboard first, then drops every press made meanwhile:
/// they pasted the same card, so they must not advance again.
pub fn settle(rx: &Receiver<()>, delay: Duration) {
    std::thread::sleep(delay);
    while rx.try_recv().is_ok() {}
}

/// Whether a key-down is the paste shortcut: V with only Cmd (macOS) / Ctrl (Windows), not a held-key repeat.
pub fn is_paste(key_is_v: bool, primary: bool, other_mods: bool, repeat: bool) -> bool {
    key_is_v && primary && !other_mods && !repeat
}

/// Where the key watch reports a paste. The callbacks do nothing else.
static PASTED: OnceLock<Sender<()>> = OnceLock::new();
/// The installed key watch, if any.
static WATCH: Mutex<Option<platform::Watch>> = Mutex::new(None);

fn pasted() {
    if let Some(tx) = PASTED.get() {
        let _ = tx.send(());
    }
}

/// Starts the worker that advances after each paste, then installs the key watch if the setting is on.
pub fn spawn(app: AppHandle) {
    let (tx, rx) = channel();
    let _ = PASTED.set(tx);
    let worker = app.clone();
    std::thread::spawn(move || {
        while rx.recv().is_ok() {
            settle(&rx, DELAY);
            // No run loop drains autoreleased Cocoa objects on this thread (see the clipboard watcher).
            #[cfg(target_os = "macos")]
            objc2::rc::autoreleasepool(|_| advance(&worker));
            #[cfg(not(target_os = "macos"))]
            advance(&worker);
        }
    });
    sync(&app);
}

/// Whether the key watch should be installed: the setting is on and the OS hasn't refused access
/// (`None` where there is no permission to ask for).
fn wanted(on: bool, access: Option<bool>) -> bool {
    on && access != Some(false)
}

/// How often a watch that is on but not installed (access not granted yet) is retried.
const RETRY: Duration = Duration::from_secs(3);
/// A retry thread is running.
static RETRYING: AtomicBool = AtomicBool::new(false);

/// Installs or removes the key watch to match the setting and the OS permission.
/// Returns whether the setting is on but no watch could be installed.
fn apply(app: &AppHandle) -> bool {
    let on = settings::get(&app.state::<AppState>().db.lock().unwrap(), "autoNextPaste") == "on";
    let mut watch = WATCH.lock().unwrap();
    match (wanted(on, platform::access()), watch.is_some()) {
        (true, false) => *watch = platform::start(),
        // Also drops a tap the OS killed when access was revoked, so a new grant installs a fresh one.
        (false, true) => {
            if let Some(watch) = watch.take() {
                watch.stop();
            }
        }
        _ => {}
    }
    on && watch.is_none()
}

/// Installs the key watch when the setting is on (and macOS allows it), removes it otherwise.
/// While it is on but can't be installed, keeps retrying, so a grant takes effect without visiting Settings.
pub fn sync(app: &AppHandle) {
    if apply(app) && !RETRYING.swap(true, Ordering::SeqCst) {
        let app = app.clone();
        std::thread::spawn(move || {
            loop {
                std::thread::sleep(RETRY);
                if !apply(&app) {
                    break;
                }
            }
            RETRYING.store(false, Ordering::SeqCst);
        });
    }
}

/// What the toggle shortcut does: off turns on only when the OS hasn't refused access; on always turns off.
pub fn next_state(on: bool, access: Option<bool>) -> AutoPasteNote {
    match (on, access) {
        (true, _) => AutoPasteNote::Off,
        (false, Some(false)) => AutoPasteNote::NeedsAccess,
        (false, _) => AutoPasteNote::On,
    }
}

/// The global shortcut: flips the setting and says so in a toast.
pub fn toggle(app: &AppHandle) {
    let state = app.state::<AppState>();
    let on = settings::get(&state.db.lock().unwrap(), "autoNextPaste") == "on";
    let note = next_state(on, platform::access());
    if note == AutoPasteNote::NeedsAccess {
        // macOS shows its prompt only the first time; the toast covers every other press.
        platform::request_access();
    } else {
        let value = if note == AutoPasteNote::On { "on" } else { "off" };
        if let Err(e) = settings::set(&state.db.lock().unwrap(), "autoNextPaste", value) {
            log::error!("failed to save autoNextPaste: {e}");
            window::show_toast(app, ToastPayload::failed());
            return;
        }
        sync(app);
        settings::commands::changed(app);
    }
    window::show_toast(app, ToastPayload::auto_next_paste(note));
}

/// Whether Vee may watch key presses (macOS Input Monitoring). `None` where no permission exists.
#[tauri::command]
pub fn get_input_access(app: AppHandle) -> Option<bool> {
    // Re-checked when the settings window regains focus, so a grant made meanwhile installs the watch.
    sync(&app);
    platform::access()
}

/// Shows the system prompt the first time; later calls do nothing (macOS asks once).
#[tauri::command]
pub fn request_input_access(app: AppHandle) {
    platform::request_access();
    sync(&app);
}

#[tauri::command]
pub fn open_input_access_settings() -> AppResult<()> {
    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("open").arg("x-apple.systempreferences:com.apple.preference.security?Privacy_ListenEvent").spawn()?;
        Ok(())
    }
    #[cfg(not(target_os = "macos"))]
    Err(crate::shared::error::AppError::Other("Input Monitoring settings are macOS only".into()))
}

#[cfg(target_os = "macos")]
mod platform {
    use super::{is_paste, pasted};
    use objc2_core_foundation::{CFMachPort, CFRetained, CFRunLoop, kCFRunLoopCommonModes, kCFRunLoopDefaultMode};
    use objc2_core_graphics::{
        CGEvent, CGEventField, CGEventFlags, CGEventTapLocation, CGEventTapOptions, CGEventTapPlacement, CGEventTapProxy, CGEventType,
        CGPreflightListenEventAccess, CGRequestListenEventAccess,
    };
    use std::cell::RefCell;
    use std::ffi::c_void;
    use std::ptr::NonNull;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::mpsc::channel;

    /// How long the watch thread sleeps in its run loop between looks at the stop flag.
    const STOP_CHECK_SECS: f64 = 0.5;

    // shortcut: matches the physical V key (ANSI key code 9), so layouts that move V (Dvorak) are missed;
    // upgrade to the event's unicode string if that matters.
    const KEY_V: i64 = 9;

    /// The watch thread's stop flag. A flag, not `CFRunLoopStop`: a stop that lands before
    /// the thread enters its run loop would be lost and leave the tap alive.
    pub struct Watch(Arc<AtomicBool>);

    impl Watch {
        pub fn stop(self) {
            self.0.store(true, Ordering::SeqCst);
        }
    }

    thread_local! {
        /// The tap of this watch thread, so its callback can re-enable it.
        static TAP: RefCell<Option<CFRetained<CFMachPort>>> = const { RefCell::new(None) };
    }

    pub fn access() -> Option<bool> {
        Some(CGPreflightListenEventAccess())
    }

    pub fn clipboard_seq() -> i64 {
        objc2_app_kit::NSPasteboard::generalPasteboard().changeCount() as i64
    }

    pub fn request_access() {
        CGRequestListenEventAccess();
    }

    unsafe extern "C-unwind" fn on_event(_: CGEventTapProxy, kind: CGEventType, event: NonNull<CGEvent>, _: *mut c_void) -> *mut CGEvent {
        if kind == CGEventType::TapDisabledByTimeout || kind == CGEventType::TapDisabledByUserInput {
            TAP.with_borrow(|tap| {
                if let Some(tap) = tap {
                    CGEvent::tap_enable(tap, true);
                }
            });
        } else if kind == CGEventType::KeyDown {
            // SAFETY: the tap hands us a valid event for the duration of the callback.
            let ev = Some(unsafe { event.as_ref() });
            let flags = CGEvent::flags(ev);
            let others = CGEventFlags::MaskShift | CGEventFlags::MaskControl | CGEventFlags::MaskAlternate;
            if is_paste(
                CGEvent::integer_value_field(ev, CGEventField::KeyboardEventKeycode) == KEY_V,
                flags.contains(CGEventFlags::MaskCommand),
                flags.intersects(others),
                CGEvent::integer_value_field(ev, CGEventField::KeyboardEventAutorepeat) != 0,
            ) {
                pasted();
            }
        }
        event.as_ptr()
    }

    /// A listen-only tap on its own thread. `None` without Input Monitoring access.
    pub fn start() -> Option<Watch> {
        if !CGPreflightListenEventAccess() {
            return None;
        }
        let (tx, rx) = channel();
        let stop = Arc::new(AtomicBool::new(false));
        let stopped = stop.clone();
        std::thread::spawn(move || {
            // SAFETY: `on_event` matches the callback type and uses no user info.
            let tap = unsafe {
                CGEvent::tap_create(
                    CGEventTapLocation::SessionEventTap,
                    CGEventTapPlacement::HeadInsertEventTap,
                    CGEventTapOptions::ListenOnly,
                    1 << CGEventType::KeyDown.0,
                    Some(on_event),
                    std::ptr::null_mut(),
                )
            };
            let ready = tap.and_then(|tap| {
                let source = CFMachPort::new_run_loop_source(None, Some(&tap), 0)?;
                let run_loop = CFRunLoop::current()?;
                // SAFETY: reading an immutable CoreFoundation constant.
                run_loop.add_source(Some(&source), unsafe { kCFRunLoopCommonModes });
                Some(tap)
            });
            let Some(tap) = ready else {
                let _ = tx.send(false);
                return;
            };
            TAP.set(Some(tap.clone()));
            let _ = tx.send(true);
            while !stopped.load(Ordering::SeqCst) {
                // SAFETY: reading an immutable CoreFoundation constant.
                CFRunLoop::run_in_mode(unsafe { kCFRunLoopDefaultMode }, STOP_CHECK_SECS, false);
            }
            CGEvent::tap_enable(&tap, false);
            tap.invalidate();
            TAP.set(None);
        });
        rx.recv().unwrap_or(false).then_some(Watch(stop))
    }
}

#[cfg(windows)]
mod platform {
    use super::{is_paste, pasted};
    use std::sync::mpsc::channel;
    use windows::Win32::{
        Foundation::{LPARAM, LRESULT, WPARAM},
        System::Threading::GetCurrentThreadId,
        UI::{
            Input::KeyboardAndMouse::{GetAsyncKeyState, VK_CONTROL, VK_LWIN, VK_MENU, VK_RWIN, VK_SHIFT, VK_V},
            WindowsAndMessaging::{
                CallNextHookEx, GetMessageW, KBDLLHOOKSTRUCT, MSG, PM_NOREMOVE, PeekMessageW, PostThreadMessageW, SetWindowsHookExW,
                UnhookWindowsHookEx, WH_KEYBOARD_LL, WM_KEYDOWN, WM_QUIT,
            },
        },
    };

    /// The hook thread's id.
    pub struct Watch(u32);

    impl Watch {
        pub fn stop(self) {
            // SAFETY: posting a message to a thread id has no memory-safety preconditions.
            let _ = unsafe { PostThreadMessageW(self.0, WM_QUIT, WPARAM(0), LPARAM(0)) };
        }
    }

    pub fn access() -> Option<bool> {
        None
    }

    pub fn request_access() {}

    // Declared here because the `windows` crate gates it behind a feature (`Win32_System_DataExchange`) Vee doesn't enable.
    #[link(name = "user32")]
    unsafe extern "system" {
        fn GetClipboardSequenceNumber() -> u32;
    }

    pub fn clipboard_seq() -> i64 {
        // SAFETY: takes no arguments and only reads a counter.
        i64::from(unsafe { GetClipboardSequenceNumber() })
    }

    fn down(vk: u16) -> bool {
        // SAFETY: GetAsyncKeyState only reads global key state.
        (unsafe { GetAsyncKeyState(i32::from(vk)) } as u16 & 0x8000) != 0
    }

    unsafe extern "system" fn on_key(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
        if code >= 0 {
            // SAFETY: for a low-level keyboard hook `lparam` points at a KBDLLHOOKSTRUCT.
            let key = unsafe { &*(lparam.0 as *const KBDLLHOOKSTRUCT) };
            if key.vkCode == u32::from(VK_V.0) && wparam.0 as u32 == WM_KEYDOWN {
                // The hook runs before the key state is updated, so V already down means a held-key repeat.
                let repeat = down(VK_V.0);
                let others = down(VK_SHIFT.0) || down(VK_MENU.0) || down(VK_LWIN.0) || down(VK_RWIN.0);
                if is_paste(true, down(VK_CONTROL.0), others, repeat) {
                    pasted();
                }
            }
        }
        // SAFETY: passing the unmodified arguments on, as every hook must.
        unsafe { CallNextHookEx(None, code, wparam, lparam) }
    }

    /// A low-level keyboard hook on its own thread, which pumps the messages the hook needs.
    pub fn start() -> Option<Watch> {
        let (tx, rx) = channel();
        std::thread::spawn(move || {
            let mut msg = MSG::default();
            // SAFETY: plain Win32 calls on this thread with valid pointers.
            unsafe {
                // Creates this thread's message queue, so `stop` can post to it right away.
                let _ = PeekMessageW(&mut msg, None, 0, 0, PM_NOREMOVE);
                let Ok(hook) = SetWindowsHookExW(WH_KEYBOARD_LL, Some(on_key), None, 0) else {
                    let _ = tx.send(None);
                    return;
                };
                let _ = tx.send(Some(Watch(GetCurrentThreadId())));
                // 0 is WM_QUIT and -1 an error: both end the loop.
                while GetMessageW(&mut msg, None, 0, 0).0 > 0 {}
                let _ = UnhookWindowsHookEx(hook);
            }
        });
        rx.recv().ok().flatten()
    }
}

#[cfg(not(any(target_os = "macos", windows)))]
mod platform {
    pub struct Watch;

    impl Watch {
        pub fn stop(self) {}
    }

    pub fn access() -> Option<bool> {
        None
    }

    pub fn request_access() {}

    pub fn clipboard_seq() -> i64 {
        0
    }

    pub fn start() -> Option<Watch> {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::clip::NewClip;
    use crate::shared::{db, error::AppError};

    /// Clips a, b, c (c newest) and their ids.
    fn history() -> (Mutex<Connection>, tempfile::TempDir, [i64; 3]) {
        let (conn, dir) = (db::open_in_memory().unwrap(), tempfile::tempdir().unwrap());
        let ids = ["a", "b", "c"].map(|t| {
            let now = i64::from(t.as_bytes()[0]);
            clip::upsert(&conn, dir.path(), NewClip::Text(t.into()), None, now).unwrap()
        });
        (Mutex::new(conn), dir, ids)
    }

    fn text(content: &ClipContent) -> String {
        match content {
            ClipContent::Text(t) => t.clone(),
            other => panic!("expected text, got {other:?}"),
        }
    }

    #[test]
    fn writes_the_card_after_the_cursor() {
        let (conn, _dir, [_, b, c]) = history();
        let mut written = Vec::new();
        let cursor = advance_with(&conn, Some(c), |content, _| {
            written.push(text(content));
            Ok(())
        });
        assert_eq!(cursor.unwrap(), Some(b));
        assert_eq!(written, ["b"]);
    }

    #[test]
    fn no_cursor_means_no_advance() {
        let (conn, _dir, _) = history();
        let mut calls = 0;
        let cursor = advance_with(&conn, None, |_, _| {
            calls += 1;
            Ok(())
        });
        assert_eq!(cursor.unwrap(), None);
        assert_eq!(calls, 0);
    }

    #[test]
    fn stays_on_the_last_card() {
        let (conn, _dir, [a, ..]) = history();
        let mut calls = 0;
        let cursor = advance_with(&conn, Some(a), |_, _| {
            calls += 1;
            Ok(())
        });
        assert_eq!(cursor.unwrap(), None);
        assert_eq!(calls, 0);
    }

    #[test]
    fn skips_cards_that_fail_to_write() {
        let (conn, _dir, [a, _, c]) = history();
        let mut written = Vec::new();
        let cursor = advance_with(&conn, Some(c), |content, _| {
            if text(content) == "b" {
                return Err(AppError::Other("gone".into()));
            }
            written.push(text(content));
            Ok(())
        });
        assert_eq!(cursor.unwrap(), Some(a));
        assert_eq!(written, ["a"]);
    }

    #[test]
    fn gives_up_after_a_few_failed_writes() {
        // A clipboard that can't be written at all (held by another app) must not walk the whole history.
        let (conn, dir) = (db::open_in_memory().unwrap(), tempfile::tempdir().unwrap());
        let ids: Vec<i64> = (0..10).map(|i| clip::upsert(&conn, dir.path(), NewClip::Text(format!("t{i}")), None, i).unwrap()).collect();
        let mut calls = 0;
        let cursor = advance_with(&Mutex::new(conn), ids.last().copied(), |_, _| {
            calls += 1;
            Err(AppError::Other("clipboard busy".into()))
        });
        assert_eq!(cursor.unwrap(), None);
        assert_eq!(calls, MAX_SKIPS);
    }

    #[test]
    fn passes_the_saved_formats_to_the_writer() {
        let (conn, _dir, [_, b, c]) = history();
        clip::set_formats(&conn.lock().unwrap(), b, &[("public.html".to_string(), b"<b>b</b>".to_vec())]).unwrap();
        let mut formats = Vec::new();
        advance_with(&conn, Some(c), |_, f| {
            formats = f;
            Ok(())
        })
        .unwrap();
        assert_eq!(formats, [("public.html".to_string(), b"<b>b</b>".to_vec())]);
    }

    #[test]
    fn only_a_plain_first_press_of_the_paste_key_counts() {
        assert!(is_paste(true, true, false, false));
        assert!(!is_paste(false, true, false, false)); // another key
        assert!(!is_paste(true, false, false, false)); // no Cmd/Ctrl
        assert!(!is_paste(true, true, true, false)); // Cmd+Shift+V and friends
        assert!(!is_paste(true, true, false, true)); // key repeat
    }

    #[test]
    fn the_watch_is_wanted_only_while_on_and_not_refused() {
        assert!(wanted(true, None)); // Windows: no permission to ask for
        assert!(wanted(true, Some(true)));
        assert!(!wanted(true, Some(false))); // not granted yet, or revoked
        assert!(!wanted(false, Some(true)));
    }

    #[test]
    fn next_state_flips_and_refuses_to_turn_on_without_access() {
        use crate::app::window::AutoPasteNote::{NeedsAccess, Off, On};
        assert_eq!(next_state(false, None), On); // Windows: no permission to ask for
        assert_eq!(next_state(false, Some(true)), On);
        assert_eq!(next_state(false, Some(false)), NeedsAccess);
        assert_eq!(next_state(true, Some(true)), Off);
        assert_eq!(next_state(true, Some(false)), Off); // revoked while on: still turns off
        assert_eq!(next_state(true, None), Off);
    }

    #[test]
    fn settle_drops_presses_made_during_the_wait() {
        let (tx, rx) = std::sync::mpsc::channel();
        tx.send(()).unwrap();
        tx.send(()).unwrap();
        settle(&rx, Duration::from_millis(10));
        assert!(rx.try_recv().is_err());
    }
}
