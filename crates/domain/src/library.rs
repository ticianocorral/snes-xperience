//! Folder scan + hash — the front half of plan §4.2 ("Varredura de pasta,
//! identificação por hash"). Recursive, one pass, no watching.

use std::collections::HashMap;
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use crate::rom::{Mapper, RomId};

/// SNES ROM file extensions we pick up.
pub const ROM_EXTS: [&str; 7] = ["sfc", "smc", "fig", "swc", "bs", "st", "bin"];

/// Archive extension we also pick up: a `.zip` holding one ROM (plan
/// revision: "add suporte a roms em formato zip"). The ROM inside is hashed
/// from the extracted bytes — the DAT lookup and everything downstream never
/// sees the wrapper.
pub const ARCHIVE_EXTS: [&str; 1] = ["zip"];

/// The ROM bytes for `path`, transparently extracting the first ROM entry
/// when it's a supported archive. Everything downstream (hashing, the core,
/// the shelf) wants raw ROM bytes; only the file system hands out zips.
pub fn load_rom(path: &Path) -> std::io::Result<Vec<u8>> {
    let bytes = fs::read(path)?;
    if !is_archive(path) {
        return Ok(bytes);
    }
    extract_rom_from_zip(&bytes, path).map_err(|e| {
        std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            format!("{}: {e}", path.display()),
        )
    })
}

/// First ROM-looking entry inside a zip's bytes — deterministic (entries in
/// archive order, names case-insensitively matched against `ROM_EXTS`),
/// skipping macOS cruft (`__MACOSX/`, dotfiles) and directories.
fn extract_rom_from_zip(bytes: &[u8], path: &Path) -> Result<Vec<u8>, String> {
    let mut archive = zip::ZipArchive::new(std::io::Cursor::new(bytes))
        .map_err(|e| format!("opening zip: {e}"))?;
    let mut pick: Option<(usize, String)> = None;
    for i in 0..archive.len() {
        let Ok(file) = archive.by_index(i) else {
            continue;
        };
        let name = file.name().to_string();
        if file.is_dir() || name.starts_with('.') || name.contains("__MACOSX") {
            continue;
        }
        let ext = name
            .rsplit('.')
            .next()
            .map(|e| e.to_ascii_lowercase())
            .unwrap_or_default();
        if !ROM_EXTS.contains(&ext.as_str()) {
            continue;
        }
        pick = Some((i, name));
        break;
    }
    let Some((i, name)) = pick else {
        return Err(format!(
            "nenhuma rom ({}) dentro do zip",
            ROM_EXTS.join("/")
        ));
    };
    let entries = archive.len();
    let mut file = archive.by_index(i).map_err(|e| format!("{e}"))?;
    if entries > 1 {
        log::info!("zip {}: usando {} de {entries} entradas", path.display(), name);
    }
    let mut out = Vec::with_capacity(file.size() as usize);
    file.read_to_end(&mut out)
        .map_err(|e| format!("lendo {name} dentro do zip: {e}"))?;
    Ok(out)
}

fn is_archive(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .map(|e| ARCHIVE_EXTS.contains(&e.to_ascii_lowercase().as_str()))
        .unwrap_or(false)
}

/// Hash/identify the ROM at `path` — `RomId::from_path`, except an archive
/// gets its inner ROM's bytes hashed instead of the zip wrapper's.
fn rom_id(path: &Path) -> Result<RomId, crate::rom::RomError> {
    if !is_archive(path) {
        return RomId::from_path(path);
    }
    match load_rom(path) {
        Ok(bytes) => RomId::from_bytes(&bytes),
        Err(source) => Err(crate::rom::RomError::Read {
            path: path.display().to_string(),
            source,
        }),
    }
}

/// One ROM file found on disk, already hashed.
#[derive(Debug, Clone)]
pub struct ScannedRom {
    pub path: PathBuf,
    pub id: RomId,
    pub file_size: u64,
    /// mtime, if the OS gave us one — used to order "recently added".
    pub modified: Option<SystemTime>,
}

/// Recursively walk `dir` for ROM files and hash each. Unreadable files and
/// hash failures are logged and skipped, not fatal.
pub fn scan(dir: &Path) -> std::io::Result<Vec<ScannedRom>> {
    let mut cache = HashCache::default();
    scan_with(dir, &mut cache)
}

/// Same scan, consulting/filling `cache`: a file whose path, size and mtime
/// all match the cache is *not* re-read or re-hashed, so a warm boot with an
/// unchanged `roms/` folder costs nothing. The catalog persists the cache
/// next to its own sidecar.
pub fn scan_with(dir: &Path, cache: &mut HashCache) -> std::io::Result<Vec<ScannedRom>> {
    let mut out = Vec::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(d) = stack.pop() {
        let entries = match std::fs::read_dir(&d) {
            Ok(e) => e,
            Err(e) => {
                log::warn!("scan: skipping {}: {e}", d.display());
                continue;
            }
        };
        for entry in entries.flatten() {
            let path = entry.path();
            let name = entry.file_name();
            if name.to_string_lossy().starts_with('.') {
                continue; // hidden files / dirs
            }
            let ft = match entry.file_type() {
                Ok(ft) => ft,
                Err(_) => continue,
            };
            if ft.is_dir() {
                stack.push(path);
                continue;
            }
            if !ft.is_file() || !(has_rom_ext(&path) || is_archive(&path)) {
                continue;
            }
            let meta = entry.metadata().ok();
            let size = meta.as_ref().map(|m| m.len()).unwrap_or(0);
            let modified = meta.as_ref().and_then(|m| m.modified().ok());
            let id = match cache.lookup(&path, size, modified) {
                Some(id) => id,
                None => match rom_id(&path) {
                    Ok(id) => {
                        cache.remember(&path, size, modified, &id);
                        id
                    }
                    Err(e) => {
                        log::warn!("scan: {}: {e}", path.display());
                        continue;
                    }
                },
            };
            out.push(ScannedRom {
                file_size: size,
                modified,
                path,
                id,
            });
        }
    }
    out.sort_by(|a, b| a.path.cmp(&b.path));
    Ok(out)
}

fn has_rom_ext(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .map(|e| ROM_EXTS.contains(&e.to_ascii_lowercase().as_str()))
        .unwrap_or(false)
}

/// Path-keyed ROM ids, persisted next to the catalog's own sidecar: a file
/// whose path, size and mtime all match is *not* re-read or re-hashed on the
/// next boot (plan revision: the startup scan hashed every ROM, serially,
/// three times over, on every launch).
#[derive(Serialize, Deserialize, Default)]
pub struct HashCache {
    entries: HashMap<String, CachedRom>,
}

#[derive(Serialize, Deserialize)]
struct CachedRom {
    size: u64,
    mtime_secs: i64,
    mtime_nanos: u32,
    header_len: u64,
    rom_len: u64,
    crc32: String,
    sha1: String,
    internal_name: Option<String>,
    mapper: Mapper,
}

impl HashCache {
    pub fn load(path: &Path) -> Self {
        match fs::read(path) {
            Ok(bytes) => serde_json::from_slice(&bytes).unwrap_or_default(),
            Err(_) => Self::default(),
        }
    }

    pub fn save(&self, path: &Path) {
        if let Ok(bytes) = serde_json::to_vec(self) {
            let _ = fs::write(path, bytes);
        }
    }

    /// The cached ids for `path`, only if size and mtime still match.
    fn lookup(&self, path: &Path, size: u64, modified: Option<SystemTime>) -> Option<RomId> {
        let (secs, nanos) = stamp(modified)?;
        let c = self.entries.get(path.to_string_lossy().as_ref())?;
        if c.size != size || c.mtime_secs != secs || c.mtime_nanos != nanos {
            return None;
        }
        Some(RomId {
            header_len: c.header_len as usize,
            rom_len: c.rom_len as usize,
            crc32: c.crc32.clone(),
            sha1: c.sha1.clone(),
            internal_name: c.internal_name.clone(),
            mapper: c.mapper,
        })
    }

    fn remember(&mut self, path: &Path, size: u64, modified: Option<SystemTime>, id: &RomId) {
        let (mtime_secs, mtime_nanos) = stamp(modified).unwrap_or((0, 0));
        self.entries.insert(
            path.to_string_lossy().into_owned(),
            CachedRom {
                size,
                mtime_secs,
                mtime_nanos,
                header_len: id.header_len as u64,
                rom_len: id.rom_len as u64,
                crc32: id.crc32.clone(),
                sha1: id.sha1.clone(),
                internal_name: id.internal_name.clone(),
                mapper: id.mapper,
            },
        );
    }
}

/// `(secs, nanos)` since the epoch for an mtime; `None` when the OS gave no
/// timestamp or it predates the epoch (those files just always re-hash).
fn stamp(modified: Option<SystemTime>) -> Option<(i64, u32)> {
    let d = modified?.duration_since(UNIX_EPOCH).ok()?;
    Some((d.as_secs() as i64, d.subsec_nanos()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hash_cache_roundtrip_and_mtime_invalidation() {
        let dir = std::env::temp_dir().join(format!("hashcache-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join("Game (USA).sfc");
        std::fs::write(&path, vec![0u8; 0x8000]).unwrap();
        let meta = std::fs::metadata(&path).unwrap();
        let modified = meta.modified().ok();
        let size = meta.len();

        let id = RomId::from_path(&path).unwrap();
        let mut cache = HashCache::default();
        cache.remember(&path, size, modified, &id);

        // Same path/size/mtime: served from the cache, identical ids.
        let hit = cache.lookup(&path, size, modified).expect("cache hit");
        assert_eq!(hit.crc32, id.crc32);
        assert_eq!(hit.sha1, id.sha1);

        // A changed mtime invalidates the entry.
        let later = modified.unwrap() + std::time::Duration::from_secs(5);
        assert!(cache.lookup(&path, size, Some(later)).is_none());

        // A different file path never hits another entry's slot.
        assert!(cache
            .lookup(&dir.join("Other (USA).sfc"), size, modified)
            .is_none());

        // Persistence round-trip.
        let file = dir.join("hashcache.json");
        cache.save(&file);
        let reloaded = HashCache::load(&file);
        assert!(reloaded.lookup(&path, size, modified).is_some());

        std::fs::remove_dir_all(&dir).unwrap();
    }

    /// A minimal valid zip holding `name` -> `bytes`, written to `dir`.
    fn write_zip(dir: &Path, name: &str, bytes: &[u8]) -> PathBuf {
        let path = dir.join("game.zip");
        let file = std::fs::File::create(&path).unwrap();
        let mut w = zip::ZipWriter::new(file);
        w.start_file(name, zip::write::SimpleFileOptions::default())
            .unwrap();
        std::io::Write::write_all(&mut w, bytes).unwrap();
        w.finish().unwrap();
        path
    }

    #[test]
    fn scan_and_load_see_the_rom_inside_a_zip() {
        let dir = std::env::temp_dir().join(format!("zip-rom-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);

        let rom = vec![0u8; 0x8000];
        let zip_path = write_zip(&dir, "Game (USA).sfc", &rom);

        // The scan picks the zip up and hashes the ROM *inside* it — its
        // sha1 matches the plain file's, not the zip wrapper's bytes.
        let scanned = scan(&dir).unwrap();
        assert_eq!(scanned.len(), 1);
        let plain = dir.join("plain.sfc");
        std::fs::write(&plain, &rom).unwrap();
        assert_eq!(scanned[0].id.sha1, RomId::from_path(&plain).unwrap().sha1);

        // `load_rom` hands the core raw ROM bytes, transparently extracted.
        let loaded = load_rom(&zip_path).unwrap();
        assert_eq!(loaded, rom);

        // A zip with no ROM entry in it is skipped, not fatal.
        let empty_zip = write_zip(&dir, "readme.txt", b"hello");
        let scanned = scan(&dir).unwrap();
        assert_eq!(scanned.len(), 1, "the txt-only zip must be skipped");
        let _ = empty_zip;

        std::fs::remove_dir_all(&dir).unwrap();
    }
}
