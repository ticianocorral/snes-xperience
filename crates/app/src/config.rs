//! `config.toml`: run-ahead default, start-fullscreen, and keyboard binds.
//!
//! Lookup order: `--config PATH`, then `$XPERIENCE_CONFIG`, then
//! `$HOME/.config/snes-xperience/config.toml`. When the last one is used and is
//! missing, a default file is written so the user has something to edit.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};
use serde::Deserialize;
use xperience_platform::{KeyMap, PadButton, UiEvent};

pub struct Config {
    pub runahead: u32,
    pub fullscreen: bool,
    pub keymap: KeyMap,
    /// Where it was read from (or freshly written), for logging.
    pub source: Option<PathBuf>,
}

#[derive(Deserialize, Default)]
struct Raw {
    #[serde(default)]
    runahead: Option<u32>,
    #[serde(default)]
    fullscreen: Option<bool>,
    #[serde(default)]
    keyboard: BTreeMap<String, String>,
}

impl Config {
    pub fn load(explicit: Option<&Path>) -> Result<Self> {
        let (path, required) = match explicit {
            Some(p) => (Some(p.to_path_buf()), true),
            None => match std::env::var_os("XPERIENCE_CONFIG") {
                Some(p) => (Some(PathBuf::from(p)), true),
                None => (default_path(), false),
            },
        };

        let mut cfg = Config {
            runahead: 1,
            fullscreen: false,
            keymap: KeyMap::defaults(),
            source: path.clone(),
        };

        let Some(path) = path else {
            return Ok(cfg);
        };

        match std::fs::read_to_string(&path) {
            Ok(text) => {
                let raw: Raw =
                    toml::from_str(&text).with_context(|| format!("parsing {}", path.display()))?;
                cfg.apply(raw)?;
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound && !required => {
                if let Some(dir) = path.parent() {
                    let _ = std::fs::create_dir_all(dir);
                }
                let _ = std::fs::write(&path, default_toml(&cfg.keymap));
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                bail!("config not found: {}", path.display());
            }
            Err(e) => {
                return Err(e).with_context(|| format!("reading {}", path.display()));
            }
        }
        Ok(cfg)
    }

    fn apply(&mut self, raw: Raw) -> Result<()> {
        if let Some(r) = raw.runahead {
            self.runahead = r;
        }
        if let Some(f) = raw.fullscreen {
            self.fullscreen = f;
        }
        for (action, key) in &raw.keyboard {
            let res = if let Ok(b) = action.parse::<PadButton>() {
                self.keymap.bind_pad(key, b)
            } else if let Some(e) = ui_from_token(action) {
                self.keymap.bind_ui(key, e)
            } else {
                bail!("[keyboard] unknown action {action:?}");
            };
            res.map_err(anyhow::Error::msg)
                .with_context(|| format!("[keyboard] {action}"))?;
        }
        Ok(())
    }
}

fn ui_from_token(t: &str) -> Option<UiEvent> {
    UiEvent::BINDABLE.into_iter().find(|e| e.token() == Some(t))
}

fn default_path() -> Option<PathBuf> {
    let home = std::env::var_os("HOME")?;
    Some(PathBuf::from(home).join(".config/snes-xperience/config.toml"))
}

fn default_toml(keymap: &KeyMap) -> String {
    let mut s = String::from(
        "# SNES Xperience configuration.\n\
         # runahead: speculative frames to hide input lag (0 disables).\n\
         # fullscreen: start in fullscreen.\n\
         # [keyboard]: action = \"SDL key name\" (e.g. \"Left Shift\", \"F2\", \"]\").\n\n",
    );
    s.push_str("runahead = 1\n");
    s.push_str("fullscreen = false\n\n[keyboard]\n");
    for (action, key) in keymap.describe() {
        s.push_str(&format!("{action} = {key:?}\n"));
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_config_keeps_defaults() {
        let mut cfg = Config {
            runahead: 1,
            fullscreen: false,
            keymap: KeyMap::defaults(),
            source: None,
        };
        cfg.apply(toml::from_str("").unwrap()).unwrap();
        assert_eq!(cfg.runahead, 1);
        assert!(!cfg.fullscreen);
    }

    #[test]
    fn overrides_and_rebinds_apply() {
        let mut cfg = Config {
            runahead: 1,
            fullscreen: false,
            keymap: KeyMap::defaults(),
            source: None,
        };
        let raw: Raw = toml::from_str(
            "runahead = 3\nfullscreen = true\n[keyboard]\nb = \"Space\"\npause = \"Escape\"",
        )
        .unwrap();
        cfg.apply(raw).unwrap();
        assert_eq!(cfg.runahead, 3);
        assert!(cfg.fullscreen);
        // Rebound key resolves; describe() reflects it.
        let d = cfg.keymap.describe();
        assert!(d.contains(&("b".to_string(), "Space".to_string())));
    }

    #[test]
    fn unknown_action_is_an_error() {
        let mut cfg = Config {
            runahead: 1,
            fullscreen: false,
            keymap: KeyMap::defaults(),
            source: None,
        };
        let raw: Raw = toml::from_str("[keyboard]\nwarp = \"W\"").unwrap();
        assert!(cfg.apply(raw).is_err());
    }

    #[test]
    fn default_toml_round_trips() {
        let km = KeyMap::defaults();
        let text = default_toml(&km);
        let raw: Raw = toml::from_str(&text).unwrap();
        let mut cfg = Config {
            runahead: 0,
            fullscreen: true,
            keymap: KeyMap::defaults(),
            source: None,
        };
        cfg.apply(raw).unwrap();
        assert_eq!(cfg.runahead, 1);
        assert!(!cfg.fullscreen);
        assert_eq!(cfg.keymap.describe(), km.describe());
    }
}
