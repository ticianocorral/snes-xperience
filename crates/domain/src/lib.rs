//! Domain layer: ROM identity, catalogue and metadata. No SDL, no libretro.

pub mod rom;
pub mod screenscraper;

pub use rom::{Mapper, RomError, RomId};
pub use screenscraper::{Client, Credentials, GameMedia, ScrapeError};
