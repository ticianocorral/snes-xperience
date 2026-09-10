//! SQLite catalogue — the aggressive local cache from plan §4.2. Holds every
//! ROM found on disk plus whatever metadata has been scraped so far, so the
//! selector can render immediately and fill in later ("preenchimento
//! progressivo", §3.1).

use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use rusqlite::{Connection, OptionalExtension};

use crate::library::ScannedRom;
use crate::screenscraper::GameInfo;

#[derive(Debug, thiserror::Error)]
pub enum CatalogError {
    #[error(transparent)]
    Sqlite(#[from] rusqlite::Error),
}

type Result<T> = std::result::Result<T, CatalogError>;

fn now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

/// A ROM as tracked on disk.
#[derive(Debug, Clone)]
pub struct RomRow {
    pub sha1: String,
    pub crc32: String,
    pub path: String,
    pub size: i64,
    pub internal_name: Option<String>,
    pub added_at: i64,
    pub last_played_at: Option<i64>,
    pub play_count: i64,
}

/// Scraped metadata for a ROM (all optional; absent row = never scraped).
#[derive(Debug, Clone, Default)]
pub struct MetaRow {
    pub name: Option<String>,
    pub year: Option<String>,
    pub developer: Option<String>,
    pub publisher: Option<String>,
    pub genre: Option<String>,
    pub players: Option<String>,
    pub region: Option<String>,
    pub synopsis: Option<String>,
    pub cover_path: Option<String>,
    pub texture_path: Option<String>,
    pub wheel_path: Option<String>,
    pub scraped_at: Option<i64>,
}

#[derive(Debug, Clone)]
pub struct CatalogEntry {
    pub rom: RomRow,
    pub meta: Option<MetaRow>,
}

impl CatalogEntry {
    /// Best display name: scraped canonical, else internal header name, else
    /// the file stem.
    pub fn title(&self) -> String {
        if let Some(n) = self.meta.as_ref().and_then(|m| m.name.clone()) {
            return n;
        }
        if let Some(n) = &self.rom.internal_name {
            return n.clone();
        }
        Path::new(&self.rom.path)
            .file_stem()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_else(|| self.rom.path.clone())
    }
}

/// Row order for the shelf.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Order {
    /// Plan §3.1: last-played first, then most-recently-added.
    Shelf,
    Name,
}

pub struct Catalog {
    conn: Connection,
}

impl Catalog {
    pub fn open(path: &Path) -> Result<Self> {
        if let Some(dir) = path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        let conn = Connection::open(path)?;
        conn.pragma_update(None, "journal_mode", "WAL")?;
        conn.pragma_update(None, "foreign_keys", "ON")?;
        let me = Self { conn };
        me.migrate()?;
        Ok(me)
    }

    /// In-memory catalogue, for tests.
    pub fn open_memory() -> Result<Self> {
        let conn = Connection::open_in_memory()?;
        let me = Self { conn };
        me.migrate()?;
        Ok(me)
    }

    fn migrate(&self) -> Result<()> {
        self.conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS rom (
                 sha1           TEXT PRIMARY KEY,
                 crc32          TEXT NOT NULL,
                 path           TEXT NOT NULL,
                 size           INTEGER NOT NULL,
                 internal_name  TEXT,
                 added_at       INTEGER NOT NULL,
                 last_played_at INTEGER,
                 play_count     INTEGER NOT NULL DEFAULT 0
             );
             CREATE TABLE IF NOT EXISTS meta (
                 sha1          TEXT PRIMARY KEY REFERENCES rom(sha1) ON DELETE CASCADE,
                 name          TEXT,
                 year          TEXT,
                 developer     TEXT,
                 publisher     TEXT,
                 genre         TEXT,
                 players       TEXT,
                 region        TEXT,
                 synopsis      TEXT,
                 cover_path    TEXT,
                 texture_path  TEXT,
                 wheel_path    TEXT,
                 scraped_at    INTEGER
             );",
        )?;
        Ok(())
    }

    /// Insert a freshly-scanned ROM or refresh its path/size/name. Returns
    /// `true` if it was new to the catalogue.
    pub fn upsert_rom(&self, r: &ScannedRom) -> Result<bool> {
        let exists: bool = self
            .conn
            .query_row(
                "SELECT 1 FROM rom WHERE sha1 = ?1",
                [&r.id.sha1],
                |_| Ok(()),
            )
            .optional()?
            .is_some();

        let path = r.path.to_string_lossy();
        if exists {
            self.conn.execute(
                "UPDATE rom SET path = ?2, size = ?3, internal_name = ?4 WHERE sha1 = ?1",
                rusqlite::params![r.id.sha1, path, r.file_size as i64, r.id.internal_name],
            )?;
            Ok(false)
        } else {
            let added = r
                .modified
                .and_then(|m| m.duration_since(UNIX_EPOCH).ok())
                .map(|d| d.as_secs() as i64)
                .unwrap_or_else(now);
            self.conn.execute(
                "INSERT INTO rom (sha1, crc32, path, size, internal_name, added_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                rusqlite::params![
                    r.id.sha1,
                    r.id.crc32,
                    path,
                    r.file_size as i64,
                    r.id.internal_name,
                    added
                ],
            )?;
            Ok(true)
        }
    }

    /// Drop catalogue rows whose file is no longer among `present` hashes.
    pub fn prune_missing(&self, present: &[String]) -> Result<usize> {
        let mut n = 0;
        let mut stmt = self.conn.prepare("SELECT sha1 FROM rom")?;
        let known: Vec<String> = stmt
            .query_map([], |row| row.get(0))?
            .filter_map(std::result::Result::ok)
            .collect();
        for sha1 in known {
            if !present.iter().any(|p| p == &sha1) {
                self.conn
                    .execute("DELETE FROM rom WHERE sha1 = ?1", [&sha1])?;
                n += 1;
            }
        }
        Ok(n)
    }

    /// Store scraped metadata. `art` gives the on-disk paths of any downloaded
    /// images (cover, texture, wheel).
    pub fn set_meta(&self, sha1: &str, info: &GameInfo, art: &ArtPaths) -> Result<()> {
        self.conn.execute(
            "INSERT INTO meta
                 (sha1, name, year, developer, publisher, genre, players, region,
                  synopsis, cover_path, texture_path, wheel_path, scraped_at)
             VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13)
             ON CONFLICT(sha1) DO UPDATE SET
                 name=excluded.name, year=excluded.year, developer=excluded.developer,
                 publisher=excluded.publisher, genre=excluded.genre, players=excluded.players,
                 region=excluded.region, synopsis=excluded.synopsis,
                 cover_path=COALESCE(excluded.cover_path, meta.cover_path),
                 texture_path=COALESCE(excluded.texture_path, meta.texture_path),
                 wheel_path=COALESCE(excluded.wheel_path, meta.wheel_path),
                 scraped_at=excluded.scraped_at",
            rusqlite::params![
                sha1,
                info.canonical_name,
                info.year,
                info.developer,
                info.publisher,
                info.genre,
                info.players,
                info.region,
                info.synopsis,
                art.cover,
                art.texture,
                art.wheel,
                now(),
            ],
        )?;
        Ok(())
    }

    pub fn mark_played(&self, sha1: &str) -> Result<()> {
        self.conn.execute(
            "UPDATE rom SET last_played_at = ?2, play_count = play_count + 1 WHERE sha1 = ?1",
            rusqlite::params![sha1, now()],
        )?;
        Ok(())
    }

    /// ROMs with no `meta` row yet, oldest-added first, capped at `limit`.
    pub fn unscraped(&self, limit: usize) -> Result<Vec<RomRow>> {
        let mut stmt = self.conn.prepare(
            "SELECT r.* FROM rom r LEFT JOIN meta m ON m.sha1 = r.sha1
             WHERE m.sha1 IS NULL ORDER BY r.added_at ASC LIMIT ?1",
        )?;
        let rows = stmt
            .query_map([limit as i64], rom_from_row)?
            .collect::<std::result::Result<_, _>>()?;
        Ok(rows)
    }

    pub fn list(&self, order: Order) -> Result<Vec<CatalogEntry>> {
        let sql = format!(
            "SELECT r.sha1, r.crc32, r.path, r.size, r.internal_name, r.added_at,
                    r.last_played_at, r.play_count,
                    m.name, m.year, m.developer, m.publisher, m.genre, m.players,
                    m.region, m.synopsis, m.cover_path, m.texture_path, m.wheel_path,
                    m.scraped_at
             FROM rom r LEFT JOIN meta m ON m.sha1 = r.sha1
             ORDER BY {}",
            match order {
                Order::Shelf => "r.last_played_at IS NULL, r.last_played_at DESC, r.added_at DESC",
                Order::Name => "COALESCE(m.name, r.internal_name, r.path) COLLATE NOCASE ASC",
            }
        );
        let mut stmt = self.conn.prepare(&sql)?;
        let rows = stmt
            .query_map([], |row| {
                let rom = RomRow {
                    sha1: row.get(0)?,
                    crc32: row.get(1)?,
                    path: row.get(2)?,
                    size: row.get(3)?,
                    internal_name: row.get(4)?,
                    added_at: row.get(5)?,
                    last_played_at: row.get(6)?,
                    play_count: row.get(7)?,
                };
                let scraped_at: Option<i64> = row.get(19)?;
                let meta = scraped_at.map(|s| MetaRow {
                    name: row.get(8).ok().flatten(),
                    year: row.get(9).ok().flatten(),
                    developer: row.get(10).ok().flatten(),
                    publisher: row.get(11).ok().flatten(),
                    genre: row.get(12).ok().flatten(),
                    players: row.get(13).ok().flatten(),
                    region: row.get(14).ok().flatten(),
                    synopsis: row.get(15).ok().flatten(),
                    cover_path: row.get(16).ok().flatten(),
                    texture_path: row.get(17).ok().flatten(),
                    wheel_path: row.get(18).ok().flatten(),
                    scraped_at: Some(s),
                });
                Ok(CatalogEntry { rom, meta })
            })?
            .collect::<std::result::Result<_, _>>()?;
        Ok(rows)
    }

    pub fn counts(&self) -> Result<(i64, i64)> {
        let roms: i64 = self
            .conn
            .query_row("SELECT COUNT(*) FROM rom", [], |r| r.get(0))?;
        let scraped: i64 = self
            .conn
            .query_row("SELECT COUNT(*) FROM meta", [], |r| r.get(0))?;
        Ok((roms, scraped))
    }
}

/// On-disk locations of downloaded art for one game.
#[derive(Debug, Clone, Default)]
pub struct ArtPaths {
    pub cover: Option<String>,
    pub texture: Option<String>,
    pub wheel: Option<String>,
}

fn rom_from_row(row: &rusqlite::Row) -> rusqlite::Result<RomRow> {
    Ok(RomRow {
        sha1: row.get("sha1")?,
        crc32: row.get("crc32")?,
        path: row.get("path")?,
        size: row.get("size")?,
        internal_name: row.get("internal_name")?,
        added_at: row.get("added_at")?,
        last_played_at: row.get("last_played_at")?,
        play_count: row.get("play_count")?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rom::{Mapper, RomId};

    fn fake_rom(sha1: &str, name: &str, modified_secs: u64) -> ScannedRom {
        ScannedRom {
            path: format!("/roms/{name}.sfc").into(),
            file_size: 0x8000,
            modified: Some(UNIX_EPOCH + std::time::Duration::from_secs(modified_secs)),
            id: RomId {
                header_len: 0,
                rom_len: 0x8000,
                crc32: "AAAAAAAA".into(),
                md5: "0".repeat(32),
                sha1: sha1.into(),
                internal_name: Some(name.to_uppercase()),
                mapper: Mapper::LoRom,
            },
        }
    }

    #[test]
    fn upsert_is_idempotent_and_reports_new() {
        let cat = Catalog::open_memory().unwrap();
        let r = fake_rom("aaaa", "zelda", 100);
        assert!(cat.upsert_rom(&r).unwrap());
        assert!(!cat.upsert_rom(&r).unwrap());
        assert_eq!(cat.counts().unwrap(), (1, 0));
    }

    #[test]
    fn shelf_order_puts_played_then_recent() {
        let cat = Catalog::open_memory().unwrap();
        cat.upsert_rom(&fake_rom("old", "a", 100)).unwrap();
        cat.upsert_rom(&fake_rom("new", "b", 200)).unwrap();
        cat.upsert_rom(&fake_rom("played", "c", 50)).unwrap();
        cat.mark_played("played").unwrap();

        let ids: Vec<_> = cat
            .list(Order::Shelf)
            .unwrap()
            .into_iter()
            .map(|e| e.rom.sha1)
            .collect();
        assert_eq!(ids, ["played", "new", "old"]);
    }

    #[test]
    fn unscraped_lists_only_missing_meta() {
        let cat = Catalog::open_memory().unwrap();
        cat.upsert_rom(&fake_rom("a", "a", 1)).unwrap();
        cat.upsert_rom(&fake_rom("b", "b", 2)).unwrap();
        cat.set_meta(
            "a",
            &GameInfo {
                canonical_name: Some("A".into()),
                ..Default::default()
            },
            &ArtPaths::default(),
        )
        .unwrap();
        let left: Vec<_> = cat
            .unscraped(10)
            .unwrap()
            .into_iter()
            .map(|r| r.sha1)
            .collect();
        assert_eq!(left, ["b"]);
        assert_eq!(cat.counts().unwrap(), (2, 1));
    }
}
