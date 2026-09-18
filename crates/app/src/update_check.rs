//! Startup update checks (plan revision: "verificar se tem update para nova
//! versao" / "verificar se o snes9x esta atualizado") — both best-effort and
//! silent on any network hiccup, gated by `Config::check_updates_on_start`.
//! Meant to run on a background thread (`std::thread::spawn`), reporting
//! back through an `mpsc::Sender` the same shape `core_update`'s download
//! worker already uses — `idle::run` drains it with `try_recv()`.

use std::path::{Path, PathBuf};
use std::sync::mpsc::Sender;
use std::time::Duration;

use serde::{Deserialize, Serialize};

const RELEASES_API: &str =
    "https://api.github.com/repos/ticianocorral/snes-xperience/releases/latest";

/// What's worth telling the player about at startup — `idle::run` only ever
/// receives one of these when there's actually something to say (see
/// `check`), so both fields are never "empty" at once.
pub struct UpdateNotice {
    /// The newer release's tag (e.g. `"v0.10.0"`), if GitHub has one.
    pub app_update: Option<String>,
    /// Whether the installed snes9x core is older than what the buildbot
    /// currently serves — only ever `true` for a core this app downloaded
    /// itself (see `CoreInstallMeta`); a hand-placed core has no baseline to
    /// compare against and is never flagged.
    pub core_stale: bool,
}

/// What `core_update::download_and_install` records alongside the core file
/// itself — the only way to tell "the buildbot has shipped a newer build
/// since this one" without re-downloading the whole thing: an ETag/
/// Content-Length fingerprint from the moment it was fetched.
#[derive(Serialize, Deserialize)]
pub struct CoreInstallMeta {
    pub url: String,
    pub etag: Option<String>,
    pub content_length: Option<u64>,
}

fn core_meta_path(core_dir: &Path) -> PathBuf {
    core_dir.join(".core_meta.json")
}

/// Best-effort — a failure here just means a later staleness check has
/// nothing to compare against, not a failed download.
pub fn save_core_meta(core_dir: &Path, meta: &CoreInstallMeta) {
    if let Ok(text) = serde_json::to_string(meta) {
        let _ = std::fs::write(core_meta_path(core_dir), text);
    }
}

fn load_core_meta(core_dir: &Path) -> Option<CoreInstallMeta> {
    let text = std::fs::read_to_string(core_meta_path(core_dir)).ok()?;
    serde_json::from_str(&text).ok()
}

fn agent() -> ureq::Agent {
    ureq::AgentBuilder::new()
        .timeout_connect(Duration::from_secs(5))
        .timeout(Duration::from_secs(8))
        .build()
}

/// `true` if the buildbot is now serving a different build than the one
/// recorded at install time — `false` with nothing to compare against (no
/// sidecar, i.e. a hand-placed core) or on any network/header hiccup, never
/// a false "yes" from a fluke.
fn core_is_stale(core_dir: &Path) -> bool {
    let Some(meta) = load_core_meta(core_dir) else {
        return false;
    };
    let Ok(resp) = agent().head(&meta.url).call() else {
        return false;
    };
    let etag = resp.header("ETag").map(str::to_string);
    if let (Some(a), Some(b)) = (&etag, &meta.etag) {
        return a != b;
    }
    let len = resp
        .header("Content-Length")
        .and_then(|s| s.parse::<u64>().ok());
    if let (Some(a), Some(b)) = (len, meta.content_length) {
        return a != b;
    }
    false
}

#[derive(Deserialize)]
struct GhRelease {
    tag_name: String,
}

/// `Some(tag)` if GitHub's latest release is newer than `current` — `None`
/// on any network/parse hiccup, or when already current. Versions are
/// compared as dot-separated integers (a leading `v` is stripped first), not
/// full semver — good enough for this project's plain `MAJOR.MINOR.PATCH`
/// tags.
fn newer_release(current: &str) -> Option<String> {
    let resp = agent()
        .get(RELEASES_API)
        .set("User-Agent", "snes-xperience-update-check")
        .call()
        .ok()?;
    let release: GhRelease = resp.into_json().ok()?;
    let tag = release.tag_name.trim_start_matches('v');
    (parse_version(tag) > parse_version(current)).then_some(release.tag_name)
}

fn parse_version(v: &str) -> Vec<u32> {
    v.split('.').map(|p| p.parse().unwrap_or(0)).collect()
}

/// Run both checks and report back on `tx` — but only if there's actually
/// something to say; a fully up-to-date app/core sends nothing at all, so
/// the receiver just sees the channel go quiet rather than an explicit
/// "you're fine" message.
pub fn check(core_dir: PathBuf, app_version: String, tx: Sender<UpdateNotice>) {
    let app_update = newer_release(&app_version);
    let core_stale = core_is_stale(&core_dir);
    if app_update.is_some() || core_stale {
        let _ = tx.send(UpdateNotice {
            app_update,
            core_stale,
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_comparison_is_numeric_not_lexicographic() {
        assert!(parse_version("0.10.0") > parse_version("0.9.0"));
        assert!(parse_version("1.0.0") > parse_version("0.99.0"));
        assert!(parse_version("0.9.0") == parse_version("0.9.0"));
        assert!(parse_version("0.9") < parse_version("0.9.1"));
    }

    /// Hits the real GitHub API — not run by default (`cargo test` skips
    /// `#[ignore]`d tests), only a manual sanity check:
    /// `cargo test -p xperience-app --lib -- --ignored hits_the_real_github_api`.
    #[test]
    #[ignore]
    fn hits_the_real_github_api() {
        assert!(newer_release("0.0.0").is_some());
        assert!(newer_release("999.0.0").is_none());
    }
}
