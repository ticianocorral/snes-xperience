//! Portable app layout: every folder the app uses lives in one root — no
//! database. `roms/` (drop ROMs here), `core/` (the snes9x core), `assets/`
//! (local cover/logo art), `saves/`, `notes/`, plus `xperience.cfg` and
//! `library.json` at the root.
//!
//! macOS special case: the `.app` on this platform ships in `/Applications`
//! (or wherever Finder drags it, often read-only-ish and not somewhere a
//! user expects an app to scribble folders into). So on macOS the root
//! isn't next to the executable at all — it's `~/Documents/SNES Xperience`,
//! created on first launch, same spirit as how a normal Mac app keeps its
//! user data.
//!
//! Linux special case: AppImage is a read-only container that extracts to a
//! temp directory. So on Linux the root is `~/.local/share/SNES Xperience`
//! (following XDG directory conventions), created on first launch. This
//! also supports regular Linux builds next to the executable.
//!
//! Windows keeps the simpler "next to the executable" portable layout, since
//! a `.exe` anywhere the user put it is already writable and exactly where
//! they'd look for `roms/` next to it.

use std::path::Path;
use std::path::PathBuf;

/// The folder the app treats as its root — see the module doc for the macOS
/// special case.
pub fn app_root() -> PathBuf {
    #[cfg(target_os = "macos")]
    {
        let home = std::env::var_os("HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("."));
        macos_root_for(&home)
    }
    #[cfg(target_os = "linux")]
    {
        linux_app_root()
    }
    #[cfg(target_os = "windows")]
    {
        let exe = std::env::current_exe().unwrap_or_else(|_| PathBuf::from("."));
        exe.parent().unwrap_or_else(|| Path::new(".")).to_path_buf()
    }
}

/// `~/Documents/SNES Xperience` — split out from `app_root` so it can be
/// unit-tested with a synthetic home directory.
#[cfg(target_os = "macos")]
fn macos_root_for(home: &Path) -> PathBuf {
    home.join("Documents").join("SNES Xperience")
}

/// XDG_DATA_HOME/.../SNES Xperience, falling back to ~/.local/share if unset.
#[cfg(target_os = "linux")]
fn linux_app_root() -> PathBuf {
    let data_home = std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".local/share")))
        .unwrap_or_else(|| PathBuf::from("."));
    data_home.join("SNES Xperience")
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
    use std::path::Path;

    #[cfg(target_os = "macos")]
    #[test]
    fn macos_root_is_documents_snes_xperience() {
        assert_eq!(
            super::macos_root_for(Path::new("/Users/rex")),
            Path::new("/Users/rex/Documents/SNES Xperience")
        );
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn linux_root_uses_xdg_data_home() {
        assert_eq!(
            super::linux_app_root(),
            Path::new(
                std::env::var("XDG_DATA_HOME")
                    .as_deref()
                    .unwrap_or(&format!(
                        "{}/.local/share",
                        std::env::var("HOME").unwrap_or_default()
                    ))
            )
            .join("SNES Xperience")
        );
    }
}
