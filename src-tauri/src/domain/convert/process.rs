//! Running a converter CLI: no console window, killed when dropped, a deadline and the cancel flag.

use super::JobError;
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

/// Exit code of a Windows program whose DLLs are missing (`0xC0000135`, STATUS_DLL_NOT_FOUND).
const DLL_NOT_FOUND: i32 = -1073741515;
/// How often `exec` checks the process and the cancel flag.
const POLL: Duration = Duration::from_millis(50);

/// A command that opens no console window on Windows.
pub fn command(program: &Path) -> Command {
    #[allow(unused_mut)]
    let mut c = Command::new(program);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        c.creation_flags(CREATE_NO_WINDOW);
    }
    c
}

/// A running converter, killed and reaped when dropped: on cancel, on error, and on a panic mid-run.
/// After `try_wait` has reaped it, `kill` is a no-op (std keeps the status), so no stray PID is signalled.
pub(super) struct Killed(pub(super) Child);

impl Drop for Killed {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

/// Runs `cmd` with no stdio until it exits, killing it after `timeout` or once `cancel` is set
/// (`Cancelled`, also when it lands as the process exits). Exit 0 is `Ok`; missing DLLs
/// (the MSVC runtime) are `Runtime`; anything else is `Engine`.
pub(super) fn exec(cmd: &mut Command, timeout: Duration, cancel: &AtomicBool, name: &str) -> Result<(), JobError> {
    if cancel.load(Ordering::SeqCst) {
        return Err(JobError::Cancelled);
    }
    // ponytail: the tool's log is dropped; pipe and tail it if failures need a reason.
    let child = cmd.stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null()).spawn();
    let mut child = Killed(child.map_err(|e| engine_error(name, e))?);
    let deadline = std::time::Instant::now() + timeout;
    loop {
        let status = child.0.try_wait().map_err(|e| engine_error(name, e))?;
        // Every return drops `child`, which kills and reaps it unless it already exited.
        if cancel.load(Ordering::SeqCst) {
            return Err(JobError::Cancelled);
        }
        match status {
            Some(status) if status.success() => return Ok(()),
            Some(status) => {
                log::warn!("convert: {name} failed: {status}");
                return Err(exit_error(status.code()));
            }
            None if std::time::Instant::now() < deadline => std::thread::sleep(POLL),
            None => return Err(engine_error(name, "timed out")),
        }
    }
}

/// The error for a failed exit `code`.
fn exit_error(code: Option<i32>) -> JobError {
    if code == Some(DLL_NOT_FOUND) { JobError::Runtime } else { JobError::Engine }
}

pub(super) fn engine_error(step: &str, e: impl std::fmt::Display) -> JobError {
    log::warn!("convert: {step} failed: {e}");
    JobError::Engine
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_dlls_are_a_runtime_error() {
        assert_eq!(exit_error(Some(-1073741515)), JobError::Runtime);
        assert_eq!(exit_error(Some(1)), JobError::Engine);
        assert_eq!(exit_error(None), JobError::Engine);
    }
}
