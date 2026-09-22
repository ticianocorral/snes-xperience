//! The catalogue: a folder scan (plan §4.1) plus a small JSON sidecar for
//! what a scan alone can't know — when a ROM was first seen and how many
//! times it's been played. No database: the app is portable, everything it
//! needs lives in plain files next to it (`xperience_app::dirs`).

use std::borrow::Cow;
use std::cell::RefCell;
use std::cmp::Reverse;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use crate::library;
use crate::nointro::NoIntroDat;
use crate::tosec;

/// Turn whatever optional extras are known for a game into label/value
/// pairs, ready for the shelf's panel ("se o DAT tiver informacoes do jogo,
/// preencher no painel"). Two independent sources, merged: a user-supplied
/// No-Intro DAT's own year/publisher always win when present; the bundled
/// TOSEC table (plan revision: "baixar o que esta la e embutir no app" —
/// TOSEC has no such fields itself, see `tosec.rs`'s own doc comment for how
/// these were pulled out of its naming convention) fills in year/publisher
/// when the DAT doesn't have them, or wasn't loaded at all — so a fresh
/// install with no DAT set up still shows *something* for most SNES ROMs,
/// not an empty panel. `NoIntroGameInfo`'s own category/description are
/// parsed but deliberately not surfaced here (plan revision: "nao mostrar
/// descricao, categoria e nome interno" — too close to raw dat/ROM-header
/// trivia for the shelf, not useful to a player).
fn info_lines(
    nointro: Option<&crate::nointro::NoIntroGameInfo>,
    crc32: &str,
) -> Vec<(String, String)> {
    let bundled = tosec::lookup(crc32);
    let mut lines = Vec::new();

    let year = nointro
        .and_then(|i| i.year.as_deref())
        .or_else(|| bundled.and_then(|t| t.year));
    if let Some(year) = year {
        lines.push(("ano".to_string(), year.to_string()));
    }

    let publisher = nointro
        .and_then(|i| i.publisher.as_deref())
        .or_else(|| bundled.and_then(|t| t.publisher));
    if let Some(publisher) = publisher {
        lines.push(("editora".to_string(), publisher.to_string()));
    }

    lines
}

#[derive(Debug, thiserror::Error)]
pub enum CatalogError {
    #[error("scanning {path}: {source}")]
    Scan {
        path: String,
        #[source]
        source: std::io::Error,
    },
    #[error("{path}: {source}")]
    Io {
        path: String,
        #[source]
        source: std::io::Error,
    },
    #[error("{path}: {source}")]
    Json {
        path: String,
        #[source]
        source: serde_json::Error,
    },
}

type Result<T> = std::result::Result<T, CatalogError>;

fn now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

/// A ROM as tracked on disk, plus the little state that persists across
/// runs (everything else is recomputed by scanning fresh each `Catalog::open`).
#[derive(Debug, Clone)]
pub struct RomRow {
    pub sha1: String,
    pub crc32: String,
    pub path: String,
    pub size: u64,
    pub internal_name: Option<String>,
    /// Canonical title from the No-Intro DAT, if one was loaded and matched
    /// by CRC32 (plan §4.1).
    pub nointro_name: Option<String>,
    /// Extra facts for the shelf panel — label/value pairs (e.g. "ano"/
    /// "1994"). A loaded No-Intro DAT's own fields win when present; the
    /// year/publisher bundled from TOSEC (plan revision) fill in the gaps,
    /// so this can be non-empty even with no DAT loaded at all. Empty only
    /// when neither source has anything for this ROM's CRC32.
    pub nointro_extra: Vec<(String, String)>,
    pub added_at: i64,
    pub last_played_at: Option<i64>,
    pub play_count: u32,
    /// The player's own "favorito" marker (plan revision: "adicionar
    /// marcador de favorito nos jogos; mostrar em uma categoria acima dos
    /// ultimos jogados") — persisted in the sidecar, shown as a strip above
    /// the recent one and a small gold corner dot on the tiles.
    pub favorite: bool,
}

#[derive(Debug, Clone)]
pub struct CatalogEntry {
    pub rom: RomRow,
}

impl RomRow {
    /// The game's display title — No-Intro name when the DAT resolved one,
    /// the internal header name otherwise, the file stem as a last resort.
    /// `Cow` so the common cases borrow instead of allocating a `String` per
    /// call (this is on the shelf's per-frame sort/filter paths).
    pub fn title(&self) -> Cow<'_, str> {
        if let Some(n) = &self.nointro_name {
            return Cow::Borrowed(n);
        }
        if let Some(n) = &self.internal_name {
            return Cow::Borrowed(n);
        }
        Cow::Owned(
            Path::new(&self.path)
                .file_stem()
                .map(|s| s.to_string_lossy().into_owned())
                .unwrap_or_else(|| self.path.clone()),
        )
    }
}

impl CatalogEntry {
    /// Best display name: No-Intro canonical, else the SNES header's
    /// internal title, else the file stem.
    pub fn title(&self) -> Cow<'_, str> {
        self.rom.title()
    }
}

/// Row order for the shelf.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Order {
    /// Plan §3.1: last-played first, then most-recently-added.
    Shelf,
    Name,
}

/// What actually needs to survive between runs, keyed by sha1 in the JSON
/// sidecar.
#[derive(Serialize, Deserialize, Default, Clone)]
struct Persisted {
    added_at: i64,
    last_played_at: Option<i64>,
    play_count: u32,
    /// `#[serde(default)]` so sidecars written before favorites existed
    /// load as "nothing is a favorite" instead of failing the whole store.
    #[serde(default)]
    favorite: bool,
}

pub struct Catalog {
    store_path: PathBuf,
    entries: RefCell<Vec<RomRow>>,
}

impl Catalog {
    /// Scan `roms_dir`, merge in whatever `store_path` (the JSON sidecar)
    /// remembers about each ROM by hash, and resolve No-Intro names if `dat`
    /// is given. Scans once, up front — automatic where the old `library
    /// scan` step was a manual one, so a corrected title is there from the
    /// shelf's first frame (plan: "já colocar com o nome certo").
    pub fn open(roms_dir: &Path, store_path: &Path, dat: Option<&NoIntroDat>) -> Result<Self> {
        // The hash cache rides next to this sidecar: a warm boot with an
        // unchanged `roms/` folder skips re-reading/re-hashing every ROM.
        let hashcache_path = store_path.with_file_name("hashcache.json");
        let mut hash_cache = library::HashCache::load(&hashcache_path);
        let scanned =
            library::scan_with(roms_dir, &mut hash_cache).map_err(|source| CatalogError::Scan {
                path: roms_dir.display().to_string(),
                source,
            })?;
        let persisted = load_store(store_path)?;
        let now = now();
        let mut entries: Vec<RomRow> = scanned
            .into_iter()
            .map(|r| {
                let p = persisted.get(&r.id.sha1).cloned().unwrap_or(Persisted {
                    added_at: r
                        .modified
                        .and_then(|m| m.duration_since(UNIX_EPOCH).ok())
                        .map(|d| d.as_secs() as i64)
                        .unwrap_or(now),
                    last_played_at: None,
                    play_count: 0,
                    favorite: false,
                });
                let nointro_hit = dat.and_then(|d| d.lookup(&r.id.crc32));
                let nointro_name = nointro_hit.map(|i| i.name.clone());
                let nointro_extra = info_lines(nointro_hit, &r.id.crc32);
                RomRow {
                    sha1: r.id.sha1,
                    crc32: r.id.crc32,
                    path: r.path.to_string_lossy().into_owned(),
                    size: r.file_size,
                    internal_name: r.id.internal_name,
                    nointro_name,
                    nointro_extra,
                    added_at: p.added_at,
                    last_played_at: p.last_played_at,
                    play_count: p.play_count,
                    favorite: p.favorite,
                }
            })
            .collect();
        entries.sort_by(|a, b| a.path.cmp(&b.path));
        hash_cache.save(&hashcache_path);
        Ok(Self {
            store_path: store_path.to_path_buf(),
            entries: RefCell::new(entries),
        })
    }

    /// Flip a game's favorite marker (the shelf panel's
    /// "favoritar"/"remover favorito" button) and persist.
    pub fn set_favorite(&self, sha1: &str, favorite: bool) -> Result<()> {
        {
            let mut entries = self.entries.borrow_mut();
            if let Some(row) = entries.iter_mut().find(|r| r.sha1 == sha1) {
                row.favorite = favorite;
            }
        }
        self.save()
    }

    pub fn mark_played(&self, sha1: &str) -> Result<()> {
        {
            let mut entries = self.entries.borrow_mut();
            if let Some(row) = entries.iter_mut().find(|r| r.sha1 == sha1) {
                row.last_played_at = Some(now());
                row.play_count += 1;
            }
        }
        self.save()
    }

    pub fn list(&self, order: Order) -> Result<Vec<CatalogEntry>> {
        let mut rows: Vec<RomRow> = self.entries.borrow().clone();
        match order {
            Order::Shelf => rows.sort_by_key(|r| {
                (
                    r.last_played_at.is_none(),
                    Reverse(r.last_played_at.unwrap_or(0)),
                    Reverse(r.added_at),
                )
            }),
            Order::Name => rows.sort_by_cached_key(|r| r.title().to_lowercase()),
        }
        Ok(rows.into_iter().map(|rom| CatalogEntry { rom }).collect())
    }

    pub fn counts(&self) -> Result<usize> {
        Ok(self.entries.borrow().len())
    }

    fn save(&self) -> Result<()> {
        let map: HashMap<String, Persisted> = self
            .entries
            .borrow()
            .iter()
            .map(|r| {
                (
                    r.sha1.clone(),
                    Persisted {
                        added_at: r.added_at,
                        last_played_at: r.last_played_at,
                        play_count: r.play_count,
                        favorite: r.favorite,
                    },
                )
            })
            .collect();
        let text = serde_json::to_string_pretty(&map).map_err(|source| CatalogError::Json {
            path: self.store_path.display().to_string(),
            source,
        })?;
        std::fs::write(&self.store_path, text).map_err(|source| CatalogError::Io {
            path: self.store_path.display().to_string(),
            source,
        })
    }
}

/// A missing or corrupted sidecar just means "nothing remembered yet" — not
/// worth a hard failure of the whole app over play-count history.
fn load_store(path: &Path) -> Result<HashMap<String, Persisted>> {
    if !path.is_file() {
        return Ok(HashMap::new());
    }
    let text = std::fs::read_to_string(path).map_err(|source| CatalogError::Io {
        path: path.display().to_string(),
        source,
    })?;
    Ok(serde_json::from_str(&text).unwrap_or_default())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    fn row(sha1: &str, name: &str, added_at: i64) -> RomRow {
        RomRow {
            sha1: sha1.to_string(),
            crc32: "AAAAAAAA".to_string(),
            path: format!("/roms/{name}.sfc"),
            size: 0x8000,
            internal_name: Some(name.to_uppercase()),
            nointro_name: None,
            nointro_extra: Vec::new(),
            added_at,
            last_played_at: None,
            play_count: 0,
            favorite: false,
        }
    }

    fn fake_catalog(rows: Vec<RomRow>) -> Catalog {
        let store_path = std::env::temp_dir().join(format!(
            "xperience-catalog-test-{}-{:?}.json",
            std::process::id(),
            std::thread::current().id()
        ));
        let _ = std::fs::remove_file(&store_path);
        Catalog {
            store_path,
            entries: RefCell::new(rows),
        }
    }

    #[test]
    fn info_lines_falls_back_to_bundled_tosec_when_no_dat_hit() {
        // Chrono Trigger (US), a real entry in the bundled TOSEC table —
        // update this CRC32 if `tosec_data.txt` is ever regenerated from a
        // TOSEC release that drops or renames it (see `tosec::tests` for a
        // release-independent sanity check on the data file itself).
        let lines = info_lines(None, "2D206BF7");
        assert_eq!(
            lines,
            vec![
                ("ano".to_string(), "1995".to_string()),
                ("editora".to_string(), "Square".to_string()),
            ]
        );
    }

    #[test]
    fn info_lines_prefers_the_dat_over_bundled_tosec() {
        let dat_info = crate::nointro::NoIntroGameInfo {
            name: "Chrono Trigger (World)".to_string(),
            year: Some("1994".to_string()), // deliberately different from TOSEC's "1995"
            publisher: None,                // left for TOSEC to fill in
            category: None,
            description: None,
        };
        let lines = info_lines(Some(&dat_info), "2D206BF7");
        assert_eq!(
            lines,
            vec![
                ("ano".to_string(), "1994".to_string()), // the DAT's, not TOSEC's
                ("editora".to_string(), "Square".to_string()), // TOSEC filled the gap
            ]
        );
    }

    #[test]
    fn info_lines_empty_when_neither_source_knows_the_crc32() {
        assert!(info_lines(None, "00000000").is_empty());
    }

    #[test]
    fn shelf_order_puts_played_then_recent() {
        let cat = fake_catalog(vec![
            row("old", "a", 100),
            row("new", "b", 200),
            row("played", "c", 50),
        ]);
        cat.mark_played("played").unwrap();

        let ids: Vec<_> = cat
            .list(Order::Shelf)
            .unwrap()
            .into_iter()
            .map(|e| e.rom.sha1)
            .collect();
        assert_eq!(ids, ["played", "new", "old"]);
        let _ = std::fs::remove_file(&cat.store_path);
    }

    #[test]
    fn title_prefers_nointro_then_internal_then_file_stem() {
        let mut r = row("a", "internal", 1);
        let entry = CatalogEntry { rom: r.clone() };
        assert_eq!(entry.title(), "INTERNAL");

        r.nointro_name = Some("Canonical Name".to_string());
        let entry = CatalogEntry { rom: r.clone() };
        assert_eq!(entry.title(), "Canonical Name");

        r.internal_name = None;
        r.nointro_name = None;
        let entry = CatalogEntry { rom: r };
        assert_eq!(entry.title(), "internal"); // falls back to the file stem
    }

    #[test]
    fn mark_played_persists_across_reopen() {
        let dir = std::env::temp_dir().join(format!(
            "xperience-catalog-roundtrip-{}",
            std::process::id()
        ));
        let roms_dir = dir.join("roms");
        std::fs::create_dir_all(&roms_dir).unwrap();
        let rom_path = roms_dir.join("Test.sfc");
        let mut bytes = vec![0u8; 0x8000];
        bytes[0x7FC0..0x7FC0 + 4].copy_from_slice(b"TEST");
        std::fs::write(&rom_path, &bytes).unwrap();
        // Give the file a stable mtime so this test isn't flaky under fast re-runs.
        std::thread::sleep(Duration::from_millis(5));

        let store_path = dir.join("library.json");
        let cat = Catalog::open(&roms_dir, &store_path, None).unwrap();
        let sha1 = cat.list(Order::Name).unwrap()[0].rom.sha1.clone();
        cat.mark_played(&sha1).unwrap();

        let reopened = Catalog::open(&roms_dir, &store_path, None).unwrap();
        let entry = &reopened.list(Order::Name).unwrap()[0];
        assert_eq!(entry.rom.play_count, 1);
        assert!(entry.rom.last_played_at.is_some());

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn favorite_persists_across_reopen() {
        let dir =
            std::env::temp_dir().join(format!("xperience-catalog-favorite-{}", std::process::id()));
        let roms_dir = dir.join("roms");
        std::fs::create_dir_all(&roms_dir).unwrap();
        let rom_path = roms_dir.join("Test.sfc");
        let mut bytes = vec![0u8; 0x8000];
        bytes[0x7FC0..0x7FC0 + 4].copy_from_slice(b"TEST");
        std::fs::write(&rom_path, &bytes).unwrap();
        std::thread::sleep(Duration::from_millis(5));

        let store_path = dir.join("library.json");
        let cat = Catalog::open(&roms_dir, &store_path, None).unwrap();
        let sha1 = cat.list(Order::Name).unwrap()[0].rom.sha1.clone();
        assert!(!cat.list(Order::Name).unwrap()[0].rom.favorite);
        cat.set_favorite(&sha1, true).unwrap();

        let reopened = Catalog::open(&roms_dir, &store_path, None).unwrap();
        assert!(reopened.list(Order::Name).unwrap()[0].rom.favorite);

        let _ = std::fs::remove_dir_all(&dir);
    }
}
