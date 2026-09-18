//! Rename ROM files to their canonical No-Intro name (plan revision:
//! "renomear automaticamente no padrao no-intro"), a settings-screen action
//! (button, not automatic on scan — the user picked that explicitly, since
//! renaming files it didn't create itself is a bigger deal than everything
//! else this app does to `roms/`). Matches by CRC32 against the loaded
//! `nointro.dat`, the same lookup the shelf's own title resolution already
//! uses — a ROM whose file name already matches the canonical one is left
//! alone.
//!
//! Every per-game folder (`saves/<title>/`, `notes/<title>/`) and local art
//! (`assets/{cover,logo,cartridge}/<title>.*`) is keyed by the ROM's file
//! stem, not its hash — so renaming the ROM out from under them would orphan
//! any save state, cheat, note or playtime already recorded, and any cover/
//! logo/cartridge art already dropped in. Each rename moves all of those
//! along with it.

use std::fs;
use std::path::Path;

use xperience_domain::{library, NoIntroDat};

use crate::runner::sanitize_dir_name;

/// One ROM actually renamed — the old and new title, for the settings
/// screen's summary line.
pub struct Renamed {
    pub old: String,
    pub new: String,
}

pub enum RenameOutcome {
    Renamed(Vec<Renamed>),
    /// No `nointro.dat` at the root, or it failed to parse.
    NoDat,
    /// A DAT loaded fine, but nothing in `roms/` needed a new name.
    NothingToDo,
}

/// Scan `roms_dir`, rename every ROM whose CRC32 matches a `nointro.dat`
/// entry under a different name, moving its save/notes folders and local
/// art to match. Best-effort throughout: a single file that can't be
/// renamed (permissions, a same-named file already there) is skipped and
/// logged, not fatal to the rest of the batch.
pub fn rename_to_nointro(
    roms_dir: &Path,
    dat_path: &Path,
    saves_dir: &Path,
    notes_dir: &Path,
    assets_dir: &Path,
) -> RenameOutcome {
    let Ok(dat) = NoIntroDat::load(dat_path) else {
        return RenameOutcome::NoDat;
    };
    let Ok(scanned) = library::scan(roms_dir) else {
        return RenameOutcome::NothingToDo;
    };

    let mut renamed = Vec::new();
    for rom in scanned {
        let Some(info) = dat.lookup(&rom.id.crc32) else {
            continue;
        };
        let Some(old_stem) = rom.path.file_stem().and_then(|s| s.to_str()) else {
            continue;
        };
        let new_stem = sanitize_dir_name(&info.name);
        if old_stem == new_stem {
            continue; // already named canonically
        }
        let ext = rom
            .path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("sfc");
        let new_path = match rom.path.parent() {
            Some(dir) => dir.join(format!("{new_stem}.{ext}")),
            None => continue,
        };
        if new_path.exists() {
            log::warn!(
                "rom rename: {} already exists, leaving {old_stem} alone",
                new_path.display()
            );
            continue;
        }
        if let Err(e) = fs::rename(&rom.path, &new_path) {
            log::warn!("rom rename: {old_stem} -> {new_stem}: {e}");
            continue;
        }

        // Carry the ROM's own progress along — same folder/file convention
        // `runner.rs`/`shelf.rs` already use to find it.
        let old_dir_name = sanitize_dir_name(old_stem);
        move_entry(saves_dir, &old_dir_name, &new_stem, &[]);
        move_entry(notes_dir, &old_dir_name, &new_stem, &[]);
        for sub in ["cover", "logo", "cartridge"] {
            move_entry(
                &assets_dir.join(sub),
                old_stem,
                &new_stem,
                &["png", "jpg", "jpeg"],
            );
        }

        renamed.push(Renamed {
            old: old_stem.to_string(),
            new: new_stem,
        });
    }

    if renamed.is_empty() {
        RenameOutcome::NothingToDo
    } else {
        RenameOutcome::Renamed(renamed)
    }
}

/// Move whatever's under `dir` for `old_key` to `new_key` — a same-named
/// subfolder when `extensions` is empty (saves/notes), or the first file
/// matching one of `extensions` otherwise (local art). A no-op if nothing's
/// there; best-effort if the move itself fails (logged, not propagated —
/// the ROM rename it's attached to has already happened either way).
fn move_entry(dir: &Path, old_key: &str, new_key: &str, extensions: &[&str]) {
    let (old, new) = if extensions.is_empty() {
        (dir.join(old_key), dir.join(new_key))
    } else {
        let Some(old) = extensions
            .iter()
            .map(|ext| dir.join(format!("{old_key}.{ext}")))
            .find(|p| p.is_file())
        else {
            return;
        };
        let ext = old
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("png")
            .to_string();
        (old, dir.join(format!("{new_key}.{ext}")))
    };
    if !old.exists() || new.exists() {
        return;
    }
    if let Err(e) = fs::rename(&old, &new) {
        log::warn!(
            "rom rename: moving {} -> {}: {e}",
            old.display(),
            new.display()
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn scratch_dir(name: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "xperience-rom-rename-test-{name}-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn write_rom(path: &Path) {
        let mut bytes = vec![0u8; 0x8000];
        bytes[0x7FC0..0x7FC0 + 4].copy_from_slice(b"TEST");
        let mut f = fs::File::create(path).unwrap();
        f.write_all(&bytes).unwrap();
    }

    #[test]
    fn renames_matched_rom_and_moves_its_folders() {
        let root = scratch_dir("basic");
        let roms_dir = root.join("roms");
        let saves_dir = root.join("saves");
        let notes_dir = root.join("notes");
        let assets_dir = root.join("assets");
        for d in [&roms_dir, &saves_dir, &notes_dir] {
            fs::create_dir_all(d).unwrap();
        }
        fs::create_dir_all(assets_dir.join("cover")).unwrap();

        let rom_path = roms_dir.join("oldname.sfc");
        write_rom(&rom_path);
        let crc = xperience_domain::RomId::from_path(&rom_path).unwrap().crc32;

        fs::create_dir_all(saves_dir.join("oldname")).unwrap();
        fs::write(saves_dir.join("oldname").join("sram.srm"), b"progress").unwrap();
        fs::write(assets_dir.join("cover").join("oldname.png"), b"art").unwrap();

        let dat_path = root.join("nointro.dat");
        fs::write(
            &dat_path,
            format!(
                r#"<?xml version="1.0"?><datafile><game name="Canonical Name (World)"><rom name="x" crc="{crc}"/></game></datafile>"#
            ),
        )
        .unwrap();

        let outcome = rename_to_nointro(&roms_dir, &dat_path, &saves_dir, &notes_dir, &assets_dir);
        match outcome {
            RenameOutcome::Renamed(r) => {
                assert_eq!(r.len(), 1);
                assert_eq!(r[0].old, "oldname");
                assert_eq!(r[0].new, "Canonical Name (World)");
            }
            _ => panic!("expected a rename"),
        }

        assert!(roms_dir.join("Canonical Name (World).sfc").is_file());
        assert!(!rom_path.exists());
        assert!(saves_dir
            .join("Canonical Name (World)")
            .join("sram.srm")
            .is_file());
        assert!(assets_dir
            .join("cover")
            .join("Canonical Name (World).png")
            .is_file());

        fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn already_canonical_name_is_left_alone() {
        let root = scratch_dir("noop");
        let roms_dir = root.join("roms");
        fs::create_dir_all(&roms_dir).unwrap();
        let rom_path = roms_dir.join("Canonical Name.sfc");
        write_rom(&rom_path);
        let crc = xperience_domain::RomId::from_path(&rom_path).unwrap().crc32;

        let dat_path = root.join("nointro.dat");
        fs::write(
            &dat_path,
            format!(
                r#"<?xml version="1.0"?><datafile><game name="Canonical Name"><rom name="x" crc="{crc}"/></game></datafile>"#
            ),
        )
        .unwrap();

        let outcome = rename_to_nointro(&roms_dir, &dat_path, &root, &root, &root);
        assert!(matches!(outcome, RenameOutcome::NothingToDo));
        assert!(rom_path.is_file());

        fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn missing_dat_reports_no_dat() {
        let root = scratch_dir("missing-dat");
        let outcome = rename_to_nointro(&root, &root.join("nointro.dat"), &root, &root, &root);
        assert!(matches!(outcome, RenameOutcome::NoDat));
        fs::remove_dir_all(&root).ok();
    }
}
