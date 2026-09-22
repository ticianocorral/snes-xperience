//! RetroAchievements client (plan: `docs/plano-retroachievements.md`,
//! fase 1) — account only for now: the login test against the real API.
//! Later phases grow this into game identification and the unlock runtime;
//! everything here is inert unless the user configured a token, and every
//! call is opt-in (the only automatic one would be a later "identify on
//! cart insert", still gated on a configured account).
//!
//! Auth is the classic web-API form (username `u` + web key `y`) — the key
//! the user generates on retroachievements.org under *Settings → Web API*.
//! Third-party clients are welcome as long as they identify themselves, so
//! the User-Agent names the app and its version.

use std::io::Read;
use std::sync::mpsc::Receiver;
use std::time::Duration;

pub const API_BASE: &str = "https://retroachievements.org";

fn agent() -> ureq::Agent {
    ureq::AgentBuilder::new()
        .timeout_connect(Duration::from_secs(5))
        .timeout(Duration::from_secs(10))
        .build()
}

/// Percent-encode the few characters a username/token can carry that would
/// break a query string — full urlencoding machinery isn't a dependency
/// worth adding for two fields.
fn encode(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char)
            }
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

/// Validate the configured credentials against the real API — returns a
/// human label for the settings row (`"<user> — <N> pontos"`), or an error
/// message that fits in the same row. Runs on the caller's thread; the
/// settings screen wraps it in the standard worker + `mpsc` shape.
pub fn test_login(user: &str, token: &str) -> Result<String, String> {
    if user.is_empty() || token.is_empty() {
        return Err("preencha usuário e token".to_string());
    }
    let resp = agent()
        .get(&format!(
            "{API_BASE}/API/API_GetUserProfile.php?u={}&y={}",
            encode(user),
            encode(token)
        ))
        .set(
            "User-Agent",
            concat!("snes-xperience/", env!("CARGO_PKG_VERSION")),
        )
        .call()
        .map_err(|e| format!("rede: {e}"))?;
    let body: serde_json::Value = resp.into_json().map_err(|e| format!("resposta: {e}"))?;
    // Failure shapes: {"Success": false} or an error code object; success
    // carries the profile with at least "User".
    if body.get("Success").is_some_and(|s| s == false) {
        return Err("usuário ou token inválidos".to_string());
    }
    let Some(name) = body.get("User").and_then(|v| v.as_str()) else {
        return Err("resposta inesperada".to_string());
    };
    let points = body.get("TotalPoints").and_then(|v| v.as_i64());
    Ok(match points {
        Some(p) => format!("{name} — {p} pontos"),
        None => name.to_string(),
    })
}

// ---- fase 2: game identification ------------------------------------

/// The RA ROM hash for a SNES image: MD5 of the file's bytes, skipping the
/// 512-byte copier header when the size says one is present (`size %
/// 0x2000 == 512`) — a faithful port of `rc_hash_snes` from rcheevos
/// (src/rhash/hash_rom.c), which is what the server hashes its registered
/// ROMs with. `rom` is the raw image (`library::load_rom` already
/// transparently unzips).
pub fn snes_ra_hash(rom: &[u8]) -> String {
    let head = if rom.len() % 0x2000 == 512 {
        &rom[512..]
    } else {
        rom
    };
    use md5::Digest;
    let mut h = md5::Md5::new();
    h.update(head);
    h.finalize().iter().map(|b| format!("{b:02x}")).collect()
}

/// What phase 2 needs to know about a game: its RA title and how many
/// achievements its set has. The unlock runtime (phase 3) will grow this
/// with the conditions themselves.
#[derive(Debug, Clone)]
pub struct RaGame {
    pub title: String,
    pub achievements: usize,
}

/// Where the per-game cache lives — `saves/ra-cache/<hash>.json`, one file
/// per identified game, holding the raw `API_GetGameExtended` reply so a
/// later phase can pull conditions/badges from it without re-fetching.
fn cache_path(hash: &str) -> std::path::PathBuf {
    crate::dirs::saves_dir()
        .join("ra-cache")
        .join(format!("{hash}.json"))
}

/// The RA hash of the ROM at `path` (zip-transparent) — reading a few MB
/// just to MD5 them; only ever called once per game per shelf visit.
pub fn hash_rom(path: &std::path::Path) -> Result<String, String> {
    let rom = xperience_domain::library::load_rom(path).map_err(|e| e.to_string())?;
    Ok(snes_ra_hash(&rom))
}

/// The cached identification for `hash`, if a previous fetch left one.
pub fn cached_game(hash: &str) -> Option<RaGame> {
    let text = std::fs::read_to_string(cache_path(hash)).ok()?;
    if text.trim().is_empty() {
        return None; // known-unknown
    }
    game_from_json(&serde_json::from_str(&text).ok()?)
}

/// Whether a previous fetch concluded "the server doesn't know this hash"
/// — an empty cache file, so the shelf never re-asks.
pub fn is_cached_unknown(hash: &str) -> bool {
    std::fs::metadata(cache_path(hash))
        .map(|m| m.len() == 0)
        .unwrap_or(false)
}

/// Parse the (already cached) `API_GetGameExtended` reply into the phase-2
/// summary. `None` when the shape isn't what we expect — treated as
/// not-identified rather than an error.
fn game_from_json(body: &serde_json::Value) -> Option<RaGame> {
    let title = body.get("Title")?.as_str()?.to_string();
    let achievements = body
        .get("Achievements")
        .and_then(|a| a.as_object())
        .map(|m| m.len())
        .unwrap_or(0);
    Some(RaGame {
        title,
        achievements,
    })
}

/// Ask the server which game `hash` is, and cache the reply next to the
/// saves. Ok(None) = the server doesn't know this hash (cached as an empty
/// file so it's never asked twice). Meant for a worker thread; the shelf
/// calls it throttled to one in flight.
pub fn fetch_game(user: &str, token: &str, hash: &str) -> Result<Option<RaGame>, String> {
    let resp = agent()
        .get(&format!(
            "{API_BASE}/API/API_GetGameExtended.php?u={}&y={}&m={}",
            encode(user),
            encode(token),
            hash
        ))
        .set(
            "User-Agent",
            concat!("snes-xperience/", env!("CARGO_PKG_VERSION")),
        )
        .call()
        .map_err(|e| format!("rede: {e}"))?;
    let body: serde_json::Value = resp.into_json().map_err(|e| format!("resposta: {e}"))?;
    if body.get("Success").is_some_and(|s| s == false) {
        // Not registered on RA — remember that so we don't re-ask.
        let _ = write_cache(hash, "");
        return Ok(None);
    }
    let Some(game) = game_from_json(&body) else {
        return Err("resposta inesperada".to_string());
    };
    let text = serde_json::to_string(&body).unwrap_or_default();
    write_cache(hash, &text)?;
    Ok(Some(game))
}

fn write_cache(hash: &str, text: &str) -> Result<(), String> {
    let path = cache_path(hash);
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| format!("cache: {e}"))?;
    }
    std::fs::write(&path, text).map_err(|e| format!("cache: {e}"))
}

// ---- fase 3: live session -------------------------------------------

use xperience_ra::runtime::{Achievement, Session};

/// Parse the cached `API_GetGameExtended` JSON into the runtime's
/// achievement list (id, texts, points, badge, `MemAddr` definition).
pub fn parse_achievements(body: &serde_json::Value) -> Vec<Achievement> {
    let Some(map) = body.get("Achievements").and_then(|a| a.as_object()) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for (_k, a) in map {
        let (Some(id), Some(title), Some(memaddr)) = (
            a.get("ID").and_then(|v| v.as_u64()).map(|v| v as u32),
            a.get("Title").and_then(|v| v.as_str()),
            a.get("MemAddr").and_then(|v| v.as_str()),
        ) else {
            continue;
        };
        out.push(Achievement {
            id,
            title: title.to_string(),
            description: a
                .get("Description")
                .and_then(|v| v.as_str())
                .unwrap_or_default()
                .to_string(),
            points: a
                .get("Points")
                .and_then(|v| v.as_str())
                .and_then(|s| s.parse().ok())
                .unwrap_or(0),
            badge: a
                .get("BadgeName")
                .and_then(|v| v.as_str())
                .unwrap_or_default()
                .to_string(),
            memaddr: memaddr.to_string(),
        });
    }
    out
}

/// Award one unlocked achievement. `hardcore` maps straight to the API's
/// `h` flag. Returns the server message on success.
pub fn award(
    user: &str,
    token: &str,
    achievement_id: u32,
    hash: &str,
    hardcore: bool,
) -> Result<String, String> {
    let resp = agent()
        .post(&format!(
            "{API_BASE}/API/API_AwardAchievement.php?u={}&y={}&a={}&h={}&m={}",
            encode(user),
            encode(token),
            achievement_id,
            if hardcore { "1" } else { "0" },
            hash
        ))
        .set(
            "User-Agent",
            concat!("snes-xperience/", env!("CARGO_PKG_VERSION")),
        )
        .call()
        .map_err(|e| format!("rede: {e}"))?;
    let body: serde_json::Value = resp.into_json().map_err(|e| format!("resposta: {e}"))?;
    if body.get("Success").is_some_and(|s| s == false) {
        let err = body
            .get("Error")
            .and_then(|v| v.as_str())
            .unwrap_or("recusado");
        return Err(err.to_string());
    }
    Ok(body
        .get("Score")
        .map(|s| s.to_string())
        .unwrap_or_else(|| "ok".into()))
}

/// Where a game's "already earned" ids live — `saves/ra-earned/<hash>.json`.
/// The runtime re-arms on reset, so this local set is what keeps unlocks
/// from being submitted twice.
fn earned_path(hash: &str) -> std::path::PathBuf {
    crate::dirs::saves_dir()
        .join("ra-earned")
        .join(format!("{hash}.json"))
}

/// Where a game's serialized rcheevos session lives —
/// `saves/ra-progress/<hash>.rap`.
pub fn progress_path(hash: &str) -> std::path::PathBuf {
    crate::dirs::saves_dir()
        .join("ra-progress")
        .join(format!("{hash}.rap"))
}

fn load_earned(hash: &str) -> std::collections::HashSet<u32> {
    std::fs::read_to_string(earned_path(hash))
        .ok()
        .and_then(|t| serde_json::from_str(&t).ok())
        .unwrap_or_default()
}

fn save_earned(hash: &str, earned: &std::collections::HashSet<u32>) {
    let path = earned_path(hash);
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    if let Ok(t) = serde_json::to_string(earned) {
        let _ = std::fs::write(path, t);
    }
}

/// What one unlocked achievement looks like to the UI — the OSD block and
/// the submit worker both get one of these.
#[derive(Debug, Clone)]
pub struct Unlock {
    pub id: u32,
    pub title: String,
    pub points: u32,
    /// BadgeName — the badge image id on the RA CDN.
    pub badge: String,
}

/// A live RA session for the inserted cartridge. `None` everywhere the
/// account isn't configured / the game isn't on RA.
pub struct Active {
    pub hash: String,
    pub game_title: String,
    pub hardcore: bool,
    session: Session,
    earned: std::collections::HashSet<u32>,
    pub user: String,
    pub token: String,
}

impl Active {
    /// Identify the ROM at `path` and arm the runtime. `Ok(None)` = the
    /// account is on but this ROM has no achievement set (or isn't on RA);
    /// `Err` = account/cache trouble worth a log line.
    pub fn start(
        path: &std::path::Path,
        user: &str,
        token: &str,
        hardcore: bool,
    ) -> Result<Option<Active>, String> {
        let hash = hash_rom(path)?;
        let Some(cached) = cached_game(&hash) else {
            return Ok(None);
        };
        // The full API reply was cached verbatim (fase 2) — parse the
        // achievements out of it again here.
        let json: serde_json::Value = match std::fs::read_to_string(cache_path(&hash))
            .ok()
            .and_then(|t| serde_json::from_str(&t).ok())
        {
            Some(j) => j,
            None => return Ok(None),
        };
        let achievements = parse_achievements(&json);
        if achievements.is_empty() {
            return Ok(None);
        }
        let mut session = Session::from_parts(achievements);
        let progress = std::fs::read(progress_path(&hash)).unwrap_or_default();
        session.load_progress(&progress);
        let earned = load_earned(&hash);
        // Progress restored: everything TRIGGERED counts as earned even if
        // the earned file was lost.
        Ok(Some(Active {
            hash,
            game_title: cached.title,
            hardcore,
            session,
            earned,
            user: user.to_string(),
            token: token.to_string(),
        }))
    }

    /// Evaluate one frame against the work RAM. Returns unlocks to notify
    /// and submit (already persisted as earned — a crash can't double-award
    /// even if the submit is lost; the server deduplicates anyway).
    pub fn tick(&mut self, ram: &[u8]) -> Vec<Unlock> {
        let mut unlocks = Vec::new();
        for id in self.session.tick(ram) {
            if !self.earned.insert(id) {
                continue;
            }
            save_earned(&self.hash, &self.earned);
            if let Some(a) = self.session.achievements.iter().find(|a| a.id == id) {
                unlocks.push(Unlock {
                    id: a.id,
                    title: a.title.clone(),
                    points: a.points,
                    badge: a.badge.clone(),
                });
            }
        }
        unlocks
    }

    /// Which achievements are earned (runtime state ∪ local set), for the
    /// pause-book listing.
    pub fn earned_snapshot(&self) -> (usize, std::collections::HashSet<u32>) {
        let total = self.session.achievements.len();
        let mut earned = self.earned.clone();
        for id in self.session.triggered_ids() {
            if earned.insert(id) {
                save_earned(&self.hash, &earned);
            }
        }
        (total, earned)
    }

    /// The achievement list, for the pause book.
    pub fn achievements(&self) -> &[Achievement] {
        &self.session.achievements
    }

    /// Bank hit counts / trigger state for the next power-on.
    pub fn save_progress(&mut self) {
        let blob = self.session.save_progress();
        if !blob.is_empty() {
            let path = progress_path(&self.hash);
            if let Some(dir) = path.parent() {
                let _ = std::fs::create_dir_all(dir);
            }
            let _ = std::fs::write(path, blob);
        }
    }

    /// Console power-on: hit counts re-arm. Earned stays authoritative.
    pub fn reset(&mut self) {
        self.session.reset();
    }

    /// Reload the earned set from disk (the shelf/session server-sync
    /// merges unlocks earned elsewhere into it) so a synced unlock is
    /// never re-submitted by this session.
    pub fn refresh_earned(&mut self) {
        self.earned = load_earned(&self.hash);
    }

    /// Fire-and-forget submit of one unlock (worker thread, 2 retries).
    /// Persisted failures go to `pending` for a later manual retry.
    pub fn submit_unlock(&self, unlock: &Unlock) {
        let (user, token, hash, hardcore) = (
            self.user.clone(),
            self.token.clone(),
            self.hash.clone(),
            self.hardcore,
        );
        let unlock = unlock.clone();
        std::thread::spawn(move || {
            for attempt in 0..3 {
                match award(&user, &token, unlock.id, &hash, hardcore) {
                    Ok(msg) => {
                        log::info!(
                            "ra: conquista {} enviada ({msg}, tentativa {})",
                            unlock.id,
                            attempt + 1
                        );
                        return;
                    }
                    Err(e) => {
                        log::warn!("ra: submit {} falhou ({e})", unlock.id);
                        std::thread::sleep(Duration::from_secs(2));
                    }
                }
            }
            // Out of retries: record for a future pass instead of dropping.
            let path = crate::dirs::saves_dir().join("ra-pending.jsonl");
            if let Ok(mut f) = std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(path)
            {
                use std::io::Write;
                let _ = writeln!(
                    f,
                    "{}\t{}\t{}\t{}",
                    hash,
                    unlock.id,
                    if hardcore { 1 } else { 0 },
                    unlock.title
                );
            }
        });
    }
}

// ---- shelf view (fase 2/4) -------------------------------------------

/// Everything the shelf's achievements view needs for one game, straight
/// from the local caches — `None` when the game isn't identified (no cache
/// from fase 2's identification).
pub struct ShelfAchievements {
    pub title: String,
    pub achievements: Vec<Achievement>,
    pub earned: std::collections::HashSet<u32>,
}

impl ShelfAchievements {
    /// Total points across the set / points already earned.
    pub fn points(&self) -> (u64, u64) {
        let all: u64 = self.achievements.iter().map(|a| a.points as u64).sum();
        let got: u64 = self
            .achievements
            .iter()
            .filter(|a| self.earned.contains(&a.id))
            .map(|a| a.points as u64)
            .sum();
        (all, got)
    }
}

/// Load the list for the ROM at `path` from the local caches (no network).
pub fn shelf_achievements(rom_path: &std::path::Path) -> Option<ShelfAchievements> {
    let hash = hash_rom(rom_path).ok()?;
    let text = std::fs::read_to_string(cache_path(&hash)).ok()?;
    if text.trim().is_empty() {
        return None;
    }
    let body: serde_json::Value = serde_json::from_str(&text).ok()?;
    let title = body.get("Title")?.as_str()?.to_string();
    let achievements = parse_achievements(&body);
    if achievements.is_empty() {
        return None;
    }
    let earned = std::fs::read_to_string(earned_path(&hash))
        .ok()
        .and_then(|t| serde_json::from_str(&t).ok())
        .unwrap_or_default();
    Some(ShelfAchievements {
        title,
        achievements,
        earned,
    })
}

// ---- server sync (a "fase futura" do plano) ---------------------------

/// The game's numeric RA id from the cached `API_GetGameExtended` reply.
fn cached_game_id(hash: &str) -> Option<u64> {
    let text = std::fs::read_to_string(cache_path(hash)).ok()?;
    let body: serde_json::Value = serde_json::from_str(&text).ok()?;
    body.get("ID").and_then(|v| v.as_u64())
}

/// One list of earned achievement ids for `game_id` — `hardcore` picks the
/// hardcore (1) or softcore (0) tally; the union of both is what the app
/// treats as "earned".
pub fn fetch_unlocks(
    user: &str,
    token: &str,
    game_id: u64,
    hardcore: bool,
) -> Result<std::collections::HashSet<u32>, String> {
    let resp = agent()
        .get(&format!(
            "{API_BASE}/API/API_GetUserUnlocks.php?u={}&y={}&g={}&h={}",
            encode(user),
            encode(token),
            game_id,
            if hardcore { "1" } else { "0" }
        ))
        .set(
            "User-Agent",
            concat!("snes-xperience/", env!("CARGO_PKG_VERSION")),
        )
        .call()
        .map_err(|e| format!("rede: {e}"))?;
    let body: serde_json::Value = resp.into_json().map_err(|e| format!("resposta: {e}"))?;
    let ids = body
        .get("UserUnlocks")
        .and_then(|v| v.as_array())
        .ok_or_else(|| "resposta inesperada".to_string())?;
    Ok(ids
        .iter()
        .filter_map(|v| v.as_u64().map(|n| n as u32))
        .collect())
}

/// The whole background sync job for one ROM: hash it, read the cached
/// game id, fetch the softcore + hardcore unlock lists, merge them into
/// the local earned set (union — a local unlock is never un-earned) and
/// persist. Runs on the caller's thread; the standard worker+mpsc shape
/// wraps it.
pub fn sync_unlocks_job(
    rom_path: &std::path::Path,
    user: &str,
    token: &str,
) -> Result<usize, String> {
    let hash = hash_rom(rom_path)?;
    let Some(game_id) = cached_game_id(&hash) else {
        return Err("jogo sem id na cache".to_string());
    };
    let mut merged = load_earned(&hash);
    let before = merged.len();
    for hardcore in [true, false] {
        merged.extend(fetch_unlocks(user, token, game_id, hardcore)?);
    }
    let added = merged.len() - before;
    if added > 0 {
        save_earned(&hash, &merged);
    }
    Ok(added)
}

/// Spawn the sync on a worker thread for the shelf / a starting session.
pub fn sync_unlocks(
    rom_path: &std::path::Path,
    user: &str,
    token: &str,
) -> Receiver<Result<usize, String>> {
    let (tx, rx) = std::sync::mpsc::channel();
    let (rom, user, token) = (rom_path.to_path_buf(), user.to_string(), token.to_string());
    std::thread::spawn(move || {
        let _ = tx.send(sync_unlocks_job(&rom, &user, &token));
    });
    rx
}

// ---- badges ----------------------------------------------------------

/// Where a downloaded badge lives — `saves/ra-cache/badges/<name>.png`.
pub fn badge_path(name: &str) -> Option<std::path::PathBuf> {
    let p = crate::dirs::saves_dir()
        .join("ra-cache")
        .join("badges")
        .join(format!("{name}.png"));
    p.is_file().then_some(p)
}

/// Download a badge PNG once; later calls read the cache. Returns the path.
pub fn download_badge(name: &str) -> Option<std::path::PathBuf> {
    if name.is_empty() {
        return None;
    }
    if let Some(p) = badge_path(name) {
        return Some(p);
    }
    let resp = agent()
        .get(&format!("{API_BASE}/Badge/{name}.png"))
        .set(
            "User-Agent",
            concat!("snes-xperience/", env!("CARGO_PKG_VERSION")),
        )
        .call()
        .ok()?;
    let mut bytes = Vec::new();
    resp.into_reader().read_to_end(&mut bytes).ok()?;
    let dir = crate::dirs::saves_dir().join("ra-cache").join("badges");
    std::fs::create_dir_all(&dir).ok()?;
    let path = dir.join(format!("{name}.png"));
    std::fs::write(&path, &bytes).ok()?;
    Some(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encode_leaves_safe_chars_and_escapes_the_rest() {
        assert_eq!(encode("user.name-1_~"), "user.name-1_~");
        assert_eq!(encode("a b&c=d"), "a%20b%26c%3Dd");
    }

    #[test]
    fn empty_credentials_fail_before_any_network() {
        assert!(test_login("", "token").is_err());
        assert!(test_login("user", "").is_err());
    }

    #[test]
    fn snes_hash_strips_copier_header() {
        // A 0x2000-multiple image with a 512-byte copier header hashes the
        // same as the headerless image; one without any header is hashed
        // as-is.
        let rom = vec![0xABu8; 0x4000];
        let headered = {
            let mut v = vec![0u8; 512];
            v.extend_from_slice(&rom);
            v
        };
        assert_eq!(snes_ra_hash(&headered), snes_ra_hash(&rom));
        assert_ne!(snes_ra_hash(&rom), snes_ra_hash(&vec![0xAB; 0x4001]));
        // Known-answer: an all-zero 32 KiB image, no header.
        assert_eq!(
            snes_ra_hash(&vec![0u8; 0x8000]),
            "bb7df04e1b0a2570657527a7e108ae23"
        );
    }

    #[test]
    fn game_json_parses_title_and_achievement_count() {
        let body: serde_json::Value = serde_json::json!({
            "Title": "Chrono Trigger",
            "Achievements": {
                "1": {"ID": 1, "Title": "a"},
                "2": {"ID": 2, "Title": "b"},
                "3": {"ID": 3, "Title": "c"}
            }
        });
        let g = game_from_json(&body).unwrap();
        assert_eq!(g.title, "Chrono Trigger");
        assert_eq!(g.achievements, 3);
        assert!(game_from_json(&serde_json::json!({})).is_none());
    }

    /// Hits the real API — manual sanity check only:
    /// `cargo test -p xperience-app --lib ra -- --ignored`
    #[test]
    #[ignore]
    fn bad_token_is_rejected_by_the_real_api() {
        let r = test_login(
            "definitely-not-a-real-user-xyz",
            "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
        );
        assert!(r.is_err());
    }
}
