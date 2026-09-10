//! Shared bits for the `xperience-app` binaries.
//!
//! - [`config`] — `config.toml` (run-ahead, fullscreen, key binds).
//! - [`runner`] — the emulator run-loop (`emu-run`, and `xperience` between games).
//! - [`shelf`] — the selector grid (`selector`, and `xperience` between games).

pub mod config;
pub mod runner;
pub mod shelf;
