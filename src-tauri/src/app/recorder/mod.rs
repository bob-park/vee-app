//! Screen recording engines: ScreenCaptureKit on macOS, FFmpeg + WASAPI loopback on Windows.
//! `Running::start` begins a recording of a selection, `stop` finalizes the file, `cancel` deletes it.
#![allow(dead_code, unused_imports)] // driven by the recorder state machine (Task 7)

#[cfg(target_os = "macos")]
mod mac;
pub mod plan;
#[cfg(windows)]
mod win;

#[cfg(target_os = "macos")]
pub use mac::Running;
pub use plan::{Container, container_for};
#[cfg(windows)]
pub use win::Running;
