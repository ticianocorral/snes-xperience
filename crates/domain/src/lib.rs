//! Domain layer: ROM identity, catalogue and No-Intro naming. No SDL.

pub mod catalog;
pub mod cheats;
pub mod library;
pub mod nointro;
pub mod rom;
pub mod tosec;

pub use catalog::{Catalog, CatalogEntry, CatalogError, Order, RomRow};
pub use cheats::{for_title as cheats_for_title, CheatDef};
pub use library::{scan, ScannedRom, ROM_EXTS};
pub use nointro::{NoIntroDat, NoIntroError};
pub use rom::{Mapper, RomError, RomId};
pub use tosec::TosecInfo;
