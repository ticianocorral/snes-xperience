//! Domain layer: ROM identity, catalogue and metadata. No SDL, no libretro.

pub mod catalog;
pub mod library;
pub mod rom;
pub mod screenscraper;

pub use catalog::{ArtPaths, Catalog, CatalogEntry, CatalogError, MetaRow, Order, RomRow};
pub use library::{scan, ScannedRom, ROM_EXTS};
pub use rom::{Mapper, RomError, RomId};
pub use screenscraper::{Client, Credentials, GameInfo, ScrapeError};
