//! Shared bits for the `xperience-app` binaries.
//!
//! - [`config`] — `config.toml` (run-ahead, fullscreen, ScreenScraper, key binds).
//! - [`dirs`] — cross-platform base directories (data, config).
//! - [`runner`] — the emulator run-loop (`emu-run`, and `xperience` between games).
//! - [`settings`] — the settings screen (`xperience` only, opened with `O` on the shelf).
//! - [`shelf`] — the selector grid (`selector`, and `xperience` between games).

pub mod config;
pub mod dirs;
pub mod runner;
pub mod settings;
pub mod shelf;
