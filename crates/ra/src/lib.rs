//! Safe wrapper over the vendored rcheevos evaluation runtime — the phase 3
//! heart of the RetroAchievements plan (`docs/plano-retroachievements.md`).
//! Only what the app needs: activate this game's achievements (definition
//! strings straight from the RA API), tick them per frame against the
//! snes9x work RAM, collect triggered ids, and save/load session progress.
pub mod runtime;
pub mod sys;
