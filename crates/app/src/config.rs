//! `xperience.cfg`: run-ahead default, start-fullscreen, and keyboard binds —
//! the same file the in-app settings screen (`O` on the shelf) edits and
//! saves. TOML syntax under the hood (same as before); only the name and
//! location changed — it now sits in the app's root (`xperience_app::dirs`,
//! next to the executable on Windows/Linux, `~/Documents/SNES Xperience` on
//! macOS), not under `~/.config` (plan: app portátil).
//!
//! Lookup order: `--config PATH`, then `$XPERIENCE_CONFIG`, then
//! `xperience_app::dirs::config_path()`. When the last one is used and is
//! missing, a default file is written so the user has something to edit.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};
use serde::Deserialize;
use xperience_platform::{KeyMap, PadButton};

pub struct Config {
    pub runahead: u32,
    pub fullscreen: bool,
    /// Check GitHub for a newer app release, and the buildbot for a fresher
    /// snes9x core, once at startup (plan revision) — a settings-screen
    /// toggle, on by default. Either check that finds something newer shows
    /// a one-time notice on the idle screen; a network failure just means no
    /// notice, never an error.
    pub check_updates_on_start: bool,
    pub keymap: KeyMap,
    /// Where it was read from (or freshly written), for logging and for
    /// `save()`.
    pub source: Option<PathBuf>,
}

#[derive(Deserialize, Default)]
struct Raw {
    #[serde(default)]
    runahead: Option<u32>,
    #[serde(default)]
    fullscreen: Option<bool>,
    #[serde(default)]
    check_updates_on_start: Option<bool>,
    #[serde(default)]
    keyboard: BTreeMap<String, String>,
}

impl Config {
    pub fn load(explicit: Option<&Path>) -> Result<Self> {
        let (path, required) = match explicit {
            Some(p) => (Some(p.to_path_buf()), true),
            None => match std::env::var_os("XPERIENCE_CONFIG") {
                Some(p) => (Some(PathBuf::from(p)), true),
                None => (Some(crate::dirs::config_path()), false),
            },
        };

        let mut cfg = Config {
            runahead: 1,
            // Plan revision: "sempre abrir em fullscreen como padrao" — a
            // fresh install (no `xperience.cfg` yet) starts fullscreen; the
            // settings screen's own toggle still turns it off from there.
            fullscreen: true,
            check_updates_on_start: true,
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
        if let Some(c) = raw.check_updates_on_start {
            self.check_updates_on_start = c;
        }
        for (action, key) in &raw.keyboard {
            let Ok(b) = action.parse::<PadButton>() else {
                // Not a gameplay button — either a typo, or (most likely for
                // anyone upgrading) one of the console/UI commands this
                // section used to rebind (eject, pause, save_state, ...)
                // before those became mouse/gamepad-only. Either way,
                // nothing left to bind it to; warn and move on rather than
                // refusing to start over a stale config line.
                log::warn!("[keyboard] {action}: not a bindable action any more, ignoring");
                continue;
            };
            self.keymap
                .bind_pad(key, b)
                .map_err(anyhow::Error::msg)
                .with_context(|| format!("[keyboard] {action}"))?;
        }
        Ok(())
    }

    /// Serialize the live config back to the `xperience.cfg` text format.
    pub fn to_toml(&self) -> String {
        let mut s = String::from(
            "# SNES Xperience configuration.\n\
             # Edited by the in-app settings screen (O on the shelf) — hand edits\n\
             # survive a save, but comments outside a value don't.\n\
             # runahead: speculative frames to hide input lag (0 disables).\n\
             # fullscreen: start in fullscreen.\n\
             # check_updates_on_start: look for a newer release/snes9x core at launch.\n\
             # [keyboard]: action = \"SDL key name\" (e.g. \"Left Shift\", \"F2\", \"]\").\n\n",
        );
        s.push_str(&format!("runahead = {}\n", self.runahead));
        s.push_str(&format!("fullscreen = {}\n", self.fullscreen));
        s.push_str(&format!(
            "check_updates_on_start = {}\n\n",
            self.check_updates_on_start
        ));
        s.push_str("[keyboard]\n");
        for (action, key) in self.keymap.describe() {
            s.push_str(&format!("{action} = {key:?}\n"));
        }
        s
    }

    /// Write the live config back to `source`. Errors if this `Config` was
    /// never tied to a path.
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
}

#[cfg(test)]
mod tests {
    use super::*;

    fn defaults() -> Config {
        Config {
            runahead: 1,
            fullscreen: false,
            check_updates_on_start: true,
            keymap: KeyMap::defaults(),
            source: None,
        }
    }

    #[test]
    fn empty_config_keeps_defaults() {
        let mut cfg = defaults();
        cfg.apply(toml::from_str("").unwrap()).unwrap();
        assert_eq!(cfg.runahead, 1);
        assert!(!cfg.fullscreen);
        assert!(cfg.check_updates_on_start);
    }

    #[test]
    fn check_updates_on_start_can_be_turned_off() {
        let mut cfg = defaults();
        cfg.apply(toml::from_str("check_updates_on_start = false").unwrap())
            .unwrap();
        assert!(!cfg.check_updates_on_start);
    }

    #[test]
    fn overrides_and_rebinds_apply() {
        let mut cfg = defaults();
        // "pause" is a pre-revision console-command binding — ignored now,
        // not an error (see `unknown_action_is_ignored`).
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
    fn unknown_action_is_ignored() {
        // Not an error: a typo, or (more likely) a pre-revision config with a
        // now-removed console-command binding (eject, pause, ...) — either
        // way there's nothing to bind it to, so `apply` warns and moves on
        // rather than refusing to start over one stale line.
        let mut cfg = defaults();
        let raw: Raw = toml::from_str("[keyboard]\nwarp = \"W\"\nb = \"Space\"").unwrap();
        cfg.apply(raw).unwrap();
        assert!(cfg
            .keymap
            .describe()
            .contains(&("b".to_string(), "Space".to_string())));
    }

    #[test]
    fn to_toml_round_trips() {
        let mut cfg = defaults();
        cfg.runahead = 2;
        cfg.fullscreen = true;
        cfg.check_updates_on_start = false;
        let text = cfg.to_toml();
        let raw: Raw = toml::from_str(&text).unwrap();
        let mut round = defaults();
        round.apply(raw).unwrap();
        assert_eq!(round.runahead, 2);
        assert!(round.fullscreen);
        assert!(!round.check_updates_on_start);
        assert_eq!(round.keymap.describe(), cfg.keymap.describe());
    }
}
