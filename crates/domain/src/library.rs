//! Folder scan + hash — the front half of plan §4.2 ("Varredura de pasta,
//! identificação por hash"). Recursive, one pass, no watching.

use std::path::{Path, PathBuf};
use std::time::SystemTime;

use crate::rom::RomId;

/// SNES ROM file extensions we pick up.
pub const ROM_EXTS: [&str; 7] = ["sfc", "smc", "fig", "swc", "bs", "st", "bin"];

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
            if !ft.is_file() || !has_rom_ext(&path) {
                continue;
            }
            match RomId::from_path(&path) {
                Ok(id) => {
                    let meta = entry.metadata().ok();
                    out.push(ScannedRom {
                        file_size: meta.as_ref().map(|m| m.len()).unwrap_or(0),
                        modified: meta.and_then(|m| m.modified().ok()),
                        path,
                        id,
                    });
                }
                Err(e) => log::warn!("scan: {}: {e}", path.display()),
            }
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
