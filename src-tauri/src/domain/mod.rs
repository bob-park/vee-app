//! Business domains. Each owns its tables and exposes its public API from `mod.rs`.

pub mod clip;
pub mod convert;
// Task 6 wires the editor window in; until then only tests use it.
#[cfg_attr(not(test), allow(dead_code, unused_imports))]
pub mod edit;
pub mod settings;
pub mod source_app;
pub mod whats_new;
