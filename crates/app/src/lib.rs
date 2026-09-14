//! Shared bits for the `xperience-app` binaries.
//!
//! - [`config`] — `xperience.cfg` (run-ahead, fullscreen, key binds).
//! - [`core_update`] — download/update the snes9x core from the libretro buildbot.
//! - [`dirs`] — the portable app layout (one root: next to the executable
//!   on Windows/Linux, `~/Documents/SNES Xperience` on macOS).
//! - [`idle`] — the idle/root screen (`xperience`'s home: TV off, "Inserir cartucho").
//! - [`runner`] — the emulator run-loop (`emu-run`, and `xperience` between games).
//! - [`settings`] — the settings screen (`xperience` only, opened with `O` on the shelf).
//! - [`shelf`] — the selector grid (`selector`, and `xperience` between games).

pub mod config;
pub mod core_update;
pub mod dirs;
pub mod idle;
pub mod runner;
pub mod settings;
pub mod shelf;
