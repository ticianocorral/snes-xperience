//! `config.toml`: run-ahead default, start-fullscreen, ScreenScraper
//! credentials, and keyboard binds — the same file the in-app settings
//! screen (`O` on the shelf) edits and saves.
//!
//! Lookup order: `--config PATH`, then `$XPERIENCE_CONFIG`, then
//! `$HOME/.config/snes-xperience/config.toml`. When the last one is used and is
//! missing, a default file is written so the user has something to edit.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};
use serde::Deserialize;
use xperience_domain::Credentials;
use xperience_platform::{KeyMap, PadButton, UiEvent};

pub struct Config {
    pub runahead: u32,
    pub fullscreen: bool,
    pub keymap: KeyMap,
    pub screenscraper: ScreenScraperSettings,
    /// Where it was read from (or freshly written), for logging and for
    /// `save()`.
    pub source: Option<PathBuf>,
}

/// ScreenScraper credentials as stored in `config.toml` (plan §4.2: "campo
/// para o usuário cadastrar as próprias credenciais"). `$SS_DEVID`/
/// `$SS_DEVPASSWORD` still work as a fallback when this is off or empty —
/// see `Config::resolve_screenscraper`.
#[derive(Default, Clone)]
pub struct ScreenScraperSettings {
    pub enabled: bool,
    pub dev_id: String,
    pub dev_password: String,
}

#[derive(Deserialize, Default)]
struct Raw {
    #[serde(default)]
    runahead: Option<u32>,
    #[serde(default)]
    fullscreen: Option<bool>,
    #[serde(default)]
    screenscraper: RawScreenScraper,
    #[serde(default)]
    keyboard: BTreeMap<String, String>,
}

#[derive(Deserialize, Default)]
struct RawScreenScraper {
    #[serde(default)]
    enabled: Option<bool>,
    #[serde(default)]
    dev_id: Option<String>,
    #[serde(default)]
    dev_password: Option<String>,
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
            screenscraper: ScreenScraperSettings::default(),
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
                let _ = std::fs::write(&path, cfg.to_toml());
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
        if let Some(e) = raw.screenscraper.enabled {
            self.screenscraper.enabled = e;
        }
        if let Some(id) = raw.screenscraper.dev_id {
            self.screenscraper.dev_id = id;
        }
        if let Some(pw) = raw.screenscraper.dev_password {
            self.screenscraper.dev_password = pw;
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

    /// Serialize the live config back to the `config.toml` text format.
    pub fn to_toml(&self) -> String {
        let mut s = String::from(
            "# SNES Xperience configuration.\n\
             # Edited by the in-app settings screen (O on the shelf) — hand edits\n\
             # survive a save, but comments outside a value don't.\n\
             # runahead: speculative frames to hide input lag (0 disables).\n\
             # fullscreen: start in fullscreen.\n\
             # [screenscraper]: your own screenscraper.fr credentials, free account.\n\
             # [keyboard]: action = \"SDL key name\" (e.g. \"Left Shift\", \"F2\", \"]\").\n\n",
        );
        s.push_str(&format!("runahead = {}\n", self.runahead));
        s.push_str(&format!("fullscreen = {}\n\n", self.fullscreen));
        s.push_str("[screenscraper]\n");
        s.push_str(&format!("enabled = {}\n", self.screenscraper.enabled));
        s.push_str(&format!("dev_id = {:?}\n", self.screenscraper.dev_id));
        s.push_str(&format!(
            "dev_password = {:?}\n\n",
            self.screenscraper.dev_password
        ));
        s.push_str("[keyboard]\n");
        for (action, key) in self.keymap.describe() {
            s.push_str(&format!("{action} = {key:?}\n"));
        }
        s
    }

    /// Write the live config back to `source`. Errors if this `Config` was
    /// never tied to a path (no `$HOME`, no `--config`, no `$XPERIENCE_CONFIG`
    /// — nowhere to put it).
    pub fn save(&self) -> Result<()> {
        let path = self
            .source
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("no config path to save to"))?;
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir).with_context(|| format!("creating {}", dir.display()))?;
        }
        std::fs::write(path, self.to_toml()).with_context(|| format!("writing {}", path.display()))
    }

    /// Credentials to scrape with: the config-stored ones if the settings
    /// screen has them switched on and filled in, else `$SS_DEVID`/
    /// `$SS_DEVPASSWORD` (unchanged fallback for the pre-settings workflow).
    pub fn resolve_screenscraper(&self) -> Option<Credentials> {
        let ss = &self.screenscraper;
        if ss.enabled && !ss.dev_id.is_empty() && !ss.dev_password.is_empty() {
            return Some(Credentials {
                dev_id: ss.dev_id.clone(),
                dev_password: ss.dev_password.clone(),
                soft_name: "snes-xperience".to_string(),
                user: None,
                user_password: None,
            });
        }
        Credentials::from_env()
    }
}

fn ui_from_token(t: &str) -> Option<UiEvent> {
    UiEvent::BINDABLE.into_iter().find(|e| e.token() == Some(t))
}

fn default_path() -> Option<PathBuf> {
    Some(crate::dirs::config_dir().join("config.toml"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn defaults() -> Config {
        Config {
            runahead: 1,
            fullscreen: false,
            keymap: KeyMap::defaults(),
            screenscraper: ScreenScraperSettings::default(),
            source: None,
        }
    }

    #[test]
    fn empty_config_keeps_defaults() {
        let mut cfg = defaults();
        cfg.apply(toml::from_str("").unwrap()).unwrap();
        assert_eq!(cfg.runahead, 1);
        assert!(!cfg.fullscreen);
        assert!(!cfg.screenscraper.enabled);
    }

    #[test]
    fn overrides_and_rebinds_apply() {
        let mut cfg = defaults();
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
        let mut cfg = defaults();
        let raw: Raw = toml::from_str("[keyboard]\nwarp = \"W\"").unwrap();
        assert!(cfg.apply(raw).is_err());
    }

    #[test]
    fn screenscraper_section_applies_and_resolves() {
        let mut cfg = defaults();
        let raw: Raw = toml::from_str(
            "[screenscraper]\nenabled = true\ndev_id = \"me\"\ndev_password = \"secret\"",
        )
        .unwrap();
        cfg.apply(raw).unwrap();
        let creds = cfg.resolve_screenscraper().expect("credentials");
        assert_eq!(creds.dev_id, "me");
        assert_eq!(creds.dev_password, "secret");
    }

    #[test]
    fn screenscraper_disabled_falls_back_to_env() {
        let mut cfg = defaults();
        cfg.screenscraper = ScreenScraperSettings {
            enabled: false,
            dev_id: "me".to_string(),
            dev_password: "secret".to_string(),
        };
        // Disabled means ignored even though filled in; falls through to
        // from_env(), which is None in a test process with no env set.
        assert!(cfg.resolve_screenscraper().is_none() || std::env::var("SS_DEVID").is_ok());
    }

    #[test]
    fn to_toml_round_trips() {
        let mut cfg = defaults();
        cfg.runahead = 2;
        cfg.fullscreen = true;
        cfg.screenscraper = ScreenScraperSettings {
            enabled: true,
            dev_id: "abc".to_string(),
            dev_password: "xyz".to_string(),
        };
        let text = cfg.to_toml();
        let raw: Raw = toml::from_str(&text).unwrap();
        let mut round = defaults();
        round.apply(raw).unwrap();
        assert_eq!(round.runahead, 2);
        assert!(round.fullscreen);
        assert!(round.screenscraper.enabled);
        assert_eq!(round.screenscraper.dev_id, "abc");
        assert_eq!(round.screenscraper.dev_password, "xyz");
        assert_eq!(round.keymap.describe(), cfg.keymap.describe());
    }
}
