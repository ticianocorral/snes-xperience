//! No-Intro DAT parsing (plan §4.1): canonical ROM titles by CRC32. No
//! network — the DAT is a file the user supplies locally (no direct download
//! link exists on No-Intro's own site); see `xperience_app::dirs::
//! nointro_dat_path`. Entirely optional: without it, the catalog falls back
//! to the SNES header's internal title or the file name, same as before.

use std::collections::HashMap;
use std::fs;
use std::path::Path;

#[derive(Debug, thiserror::Error)]
pub enum NoIntroError {
    #[error("could not read DAT {path}: {source}")]
    Read {
        path: String,
        #[source]
        source: std::io::Error,
    },
    #[error("could not parse DAT: {0}")]
    Parse(#[from] roxmltree::Error),
}

/// Canonical names, keyed by the ROM's headerless CRC32 in upper hex (e.g.
/// `"05FBB855"`) — matches `RomId::crc32`'s own formatting exactly, so
/// lookups need no reformatting on that side.
pub struct NoIntroDat {
    by_crc32: HashMap<String, String>,
}

impl NoIntroDat {
    /// Parse a No-Intro `<datafile>` XML: one `<game name="...">` per title,
    /// one or more `<rom crc="...">` children (SNES games are single-ROM, but
    /// the format allows more).
    pub fn load(path: &Path) -> Result<Self, NoIntroError> {
        let text = fs::read_to_string(path).map_err(|source| NoIntroError::Read {
            path: path.display().to_string(),
            source,
        })?;
        let doc = roxmltree::Document::parse(&text)?;
        let mut by_crc32 = HashMap::new();
        for game in doc.descendants().filter(|n| n.has_tag_name("game")) {
            let Some(name) = game.attribute("name") else {
                continue;
            };
            for rom in game.children().filter(|n| n.has_tag_name("rom")) {
                if let Some(crc) = rom.attribute("crc") {
                    by_crc32.insert(crc.to_ascii_uppercase(), name.to_string());
                }
            }
        }
        log::info!(
            "no-intro DAT: {} entries loaded from {}",
            by_crc32.len(),
            path.display()
        );
        Ok(Self { by_crc32 })
    }

    pub fn lookup(&self, crc32: &str) -> Option<&str> {
        self.by_crc32
            .get(&crc32.to_ascii_uppercase())
            .map(String::as_str)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write_dat(contents: &str) -> std::path::PathBuf {
        let path =
            std::env::temp_dir().join(format!("xperience-nointro-test-{}.dat", std::process::id()));
        fs::write(&path, contents).unwrap();
        path
    }

    #[test]
    fn resolves_name_by_crc32_case_insensitively() {
        let path = write_dat(
            r#"<?xml version="1.0"?>
<datafile>
  <game name="Super Test World (World)">
    <rom name="Super Test World (World).sfc" size="1048576" crc="deadbeef" md5="x" sha1="y"/>
  </game>
</datafile>"#,
        );
        let dat = NoIntroDat::load(&path).unwrap();
        assert_eq!(dat.lookup("DEADBEEF"), Some("Super Test World (World)"));
        assert_eq!(dat.lookup("00000000"), None);
        let _ = fs::remove_file(&path);
    }

    #[test]
    fn missing_file_is_an_error() {
        assert!(NoIntroDat::load(Path::new("/does/not/exist.dat")).is_err());
    }
}
