//! Emulation layer: load a libretro core at runtime and drive its frame loop.
//!
//! This crate is deliberately ignorant of the presentation layer — it hands up
//! raw frames, audio and an input matrix, and nothing here knows a bezel or a
//! selector screen exists (see the architecture section of the project plan).

mod core;
mod sys;

pub use crate::core::{AvInfo, Button, Core, CoreError, Frame, PixelFormat, MAX_PORTS};
