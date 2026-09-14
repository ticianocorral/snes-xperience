//! Portable app layout: every folder the app uses lives next to the
//! executable — no installation, no XDG dirs, no database. `roms/` (drop
//! ROMs here), `core/` (the snes9x core), `assets/` (local cover/logo art),
//! `saves/`, `notes/`, plus `xperience.cfg` and `library.json` at the root.
//!
//! macOS special case: a packaged `.app`'s real executable lives three
//! levels inside the bundle (`Name.app/Contents/MacOS/xperience`). "Next to
//! the executable" for a user means next to `Name.app` in Finder, not
//! buried inside it — `app_root` detects that shape and climbs past it.

use std::path::{Path, PathBuf};

/// The folder the app treats as its root — see the module doc for the macOS
/// `.app` special case.
pub fn app_root() -> PathBuf {
    let exe = std::env::current_exe().unwrap_or_else(|_| PathBuf::from("."));
    let dir = exe.parent().unwrap_or_else(|| Path::new("."));
    app_root_for(dir)
}

/// The exe-parent-to-app-root logic, taking a `&Path` instead of reading
/// `current_exe()` itself so it can be unit-tested with synthetic paths.
fn app_root_for(dir: &Path) -> PathBuf {
    #[cfg(target_os = "macos")]
    {
        use std::ffi::OsStr;
        let bundle_root = (|| {
            if dir.file_name() != Some(OsStr::new("MacOS")) {
                return None;
            }
            let contents = dir.parent()?;
            if contents.file_name() != Some(OsStr::new("Contents")) {
                return None;
            }
            let bundle = contents.parent()?;
            if bundle.extension() != Some(OsStr::new("app")) {
                return None;
            }
            bundle.parent().map(Path::to_path_buf)
        })();
        if let Some(outside) = bundle_root {
            return outside;
        }
    }
    dir.to_path_buf()
}

pub fn roms_dir() -> PathBuf {
    app_root().join("roms")
}

pub fn core_dir() -> PathBuf {
    app_root().join("core")
}

pub fn assets_dir() -> PathBuf {
    app_root().join("assets")
}

pub fn saves_dir() -> PathBuf {
    app_root().join("saves")
}

pub fn notes_dir() -> PathBuf {
    app_root().join("notes")
}

pub fn config_path() -> PathBuf {
    app_root().join("xperience.cfg")
}

/// Play counts / added-at / last-played-at, keyed by ROM hash — the only
/// state that needs to survive between runs (everything else is recomputed
/// by scanning `roms/` fresh each launch).
pub fn library_path() -> PathBuf {
    app_root().join("library.json")
}

/// A No-Intro DAT (XML) for canonical ROM titles — optional, supplied by
/// whoever runs the app (no direct download link exists on No-Intro's own
/// site to fetch it automatically).
pub fn nointro_dat_path() -> PathBuf {
    app_root().join("nointro.dat")
}

#[cfg(test)]
mod tests {
    use super::app_root_for;
    use std::path::Path;

    #[test]
    fn plain_folder_is_its_own_root() {
        assert_eq!(
            app_root_for(Path::new("/opt/snes-xperience")),
            Path::new("/opt/snes-xperience")
        );
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn macos_bundle_climbs_to_the_app_s_sibling() {
        assert_eq!(
            app_root_for(Path::new(
                "/Users/rex/Downloads/SNES Xperience.app/Contents/MacOS"
            )),
            Path::new("/Users/rex/Downloads")
        );
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn macos_non_bundle_path_is_unaffected() {
        // A dev build's target/debug/ isn't a bundle — no climbing.
        assert_eq!(
            app_root_for(Path::new("/repo/target/debug")),
            Path::new("/repo/target/debug")
        );
    }
}
