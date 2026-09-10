//! Minimal ScreenScraper `jeuInfos.php` client — sections 4.2 and 4.3 of the
//! plan. Matches by hash + size + filename and pulls the two cartridge medias
//! the frame needs: `texture` (cut-out label) and `wheel` (transparent logo).
//!
//! This is deliberately thin: no cache, no rate-limit bookkeeping beyond
//! surfacing the quota the API reports. Phase 2 adds the SQLite cache.

use std::time::Duration;

use serde::Deserialize;

use crate::rom::RomId;

const ENDPOINT: &str = "https://api.screenscraper.fr/api2/jeuInfos.php";

/// Credentials. `dev_id` / `dev_password` are issued by ScreenScraper to a
/// registered developer; `user` / `password` are the end user's own account and
/// are optional but raise the quota (plan §4.2).
#[derive(Debug, Clone)]
pub struct Credentials {
    pub dev_id: String,
    pub dev_password: String,
    pub soft_name: String,
    pub user: Option<String>,
    pub user_password: Option<String>,
}

impl Credentials {
    /// Read from `SS_DEVID`, `SS_DEVPASSWORD`, `SS_SOFTNAME`, `SS_USER`,
    /// `SS_PASSWORD`. Returns `None` if the mandatory three are absent.
    pub fn from_env() -> Option<Self> {
        let dev_id = std::env::var("SS_DEVID").ok()?;
        let dev_password = std::env::var("SS_DEVPASSWORD").ok()?;
        let soft_name =
            std::env::var("SS_SOFTNAME").unwrap_or_else(|_| "snes-xperience".to_string());
        if dev_id.is_empty() || dev_password.is_empty() {
            return None;
        }
        Some(Self {
            dev_id,
            dev_password,
            soft_name,
            user: non_empty(std::env::var("SS_USER").ok()),
            user_password: non_empty(std::env::var("SS_PASSWORD").ok()),
        })
    }
}

fn non_empty(v: Option<String>) -> Option<String> {
    v.filter(|s| !s.is_empty())
}

#[derive(Debug, thiserror::Error)]
pub enum ScrapeError {
    #[error("network error talking to ScreenScraper: {0}")]
    Network(String),
    #[error("ScreenScraper HTTP {status}: {body}")]
    Http { status: u16, body: String },
    #[error("ScreenScraper returned no game for this ROM")]
    NotFound,
    #[error("could not parse ScreenScraper response: {0}")]
    Parse(String),
}

/// What Phase 0 needs to see per ROM.
#[derive(Debug, Clone)]
pub struct GameMedia {
    pub game_id: Option<String>,
    pub canonical_name: Option<String>,
    pub region: Option<String>,
    /// Cut-out cartridge label, for applying to the 3D-ish model.
    pub texture_url: Option<String>,
    /// Transparent-PNG logo (`wheel`).
    pub wheel_url: Option<String>,
    /// Requests left today, if the API reported it.
    pub requests_today: Option<String>,
    pub max_requests_per_day: Option<String>,
}

impl GameMedia {
    pub fn has_both(&self) -> bool {
        self.texture_url.is_some() && self.wheel_url.is_some()
    }
}

pub struct Client {
    creds: Credentials,
    agent: ureq::Agent,
}

impl Client {
    pub fn new(creds: Credentials) -> Self {
        let agent = ureq::AgentBuilder::new()
            .timeout_connect(Duration::from_secs(10))
            .timeout(Duration::from_secs(30))
            .build();
        Self { creds, agent }
    }

    pub fn lookup(&self, rom: &RomId, filename: &str) -> Result<GameMedia, ScrapeError> {
        let size = rom.rom_len.to_string();
        let mut req = self
            .agent
            .get(ENDPOINT)
            .query("output", "json")
            .query("devid", &self.creds.dev_id)
            .query("devpassword", &self.creds.dev_password)
            .query("softname", &self.creds.soft_name)
            .query("systemeid", "4") // 4 = Super Nintendo
            .query("romtype", "rom")
            .query("crc", &rom.crc32)
            .query("md5", &rom.md5)
            .query("sha1", &rom.sha1)
            .query("romnom", filename)
            .query("romtaille", &size);

        if let (Some(u), Some(p)) = (&self.creds.user, &self.creds.user_password) {
            req = req.query("ssid", u).query("sspassword", p);
        }

        let body = match req.call() {
            Ok(resp) => resp
                .into_string()
                .map_err(|e| ScrapeError::Network(e.to_string()))?,
            Err(ureq::Error::Status(code, resp)) => {
                let body = resp.into_string().unwrap_or_default();
                // ScreenScraper says 404 for "not found" but also for some auth
                // issues; disambiguate on the body text.
                if code == 404 && body.to_lowercase().contains("rom") {
                    return Err(ScrapeError::NotFound);
                }
                return Err(ScrapeError::Http {
                    status: code,
                    body: body.chars().take(300).collect(),
                });
            }
            Err(e) => return Err(ScrapeError::Network(e.to_string())),
        };

        parse(&body)
    }
}

fn parse(body: &str) -> Result<GameMedia, ScrapeError> {
    let trimmed = body.trim_start();
    if !trimmed.starts_with('{') {
        // API emits a bare error line for quota / bad key.
        if trimmed.to_lowercase().contains("rom") && trimmed.to_lowercase().contains("trouv") {
            return Err(ScrapeError::NotFound);
        }
        return Err(ScrapeError::Parse(trimmed.chars().take(300).collect()));
    }

    let root: Root = serde_json::from_str(body).map_err(|e| ScrapeError::Parse(e.to_string()))?;
    let response = root.response.ok_or(ScrapeError::NotFound)?;
    let jeu = response.jeu.ok_or(ScrapeError::NotFound)?;

    let canonical_name = pick_name(&jeu.noms);
    let (texture_url, wheel_url, region) = pick_media(&jeu.medias);

    let (requests_today, max_requests_per_day) = response
        .ssuser
        .map(|u| (u.requeststoday, u.maxrequestsperday))
        .unwrap_or((None, None));

    Ok(GameMedia {
        game_id: jeu.id.map(|v| v.to_string()),
        canonical_name,
        region,
        texture_url,
        wheel_url,
        requests_today,
        max_requests_per_day,
    })
}

fn pick_name(noms: &[Nom]) -> Option<String> {
    const PREF: [&str; 5] = ["wor", "us", "eu", "ss", "jp"];
    for want in PREF {
        if let Some(n) = noms.iter().find(|n| n.region.as_deref() == Some(want)) {
            if let Some(t) = &n.text {
                return Some(t.clone());
            }
        }
    }
    noms.iter().find_map(|n| n.text.clone())
}

fn pick_media(medias: &[Media]) -> (Option<String>, Option<String>, Option<String>) {
    let find = |kind: &str| -> Option<&Media> {
        const PREF: [&str; 4] = ["wor", "us", "eu", "ss"];
        for want in PREF {
            if let Some(m) = medias
                .iter()
                .find(|m| m.kind.as_deref() == Some(kind) && m.region.as_deref() == Some(want))
            {
                return Some(m);
            }
        }
        medias.iter().find(|m| m.kind.as_deref() == Some(kind))
    };
    let texture = find("texture");
    let wheel = find("wheel").or_else(|| find("wheel-hd"));
    let region = texture
        .and_then(|m| m.region.clone())
        .or_else(|| wheel.and_then(|m| m.region.clone()));
    (
        texture.and_then(|m| m.url.clone()),
        wheel.and_then(|m| m.url.clone()),
        region,
    )
}

// --- response shape (only the fields we read) -----------------------------

#[derive(Deserialize)]
struct Root {
    response: Option<Response>,
}

#[derive(Deserialize)]
struct Response {
    jeu: Option<Jeu>,
    ssuser: Option<SsUser>,
}

#[derive(Deserialize)]
struct SsUser {
    #[serde(default)]
    requeststoday: Option<String>,
    #[serde(default)]
    maxrequestsperday: Option<String>,
}

#[derive(Deserialize)]
struct Jeu {
    #[serde(default)]
    id: Option<serde_json::Value>,
    #[serde(default)]
    noms: Vec<Nom>,
    #[serde(default)]
    medias: Vec<Media>,
}

#[derive(Deserialize)]
struct Nom {
    #[serde(default)]
    region: Option<String>,
    #[serde(default, alias = "nom")]
    text: Option<String>,
}

#[derive(Deserialize)]
struct Media {
    #[serde(rename = "type", default)]
    kind: Option<String>,
    #[serde(default)]
    region: Option<String>,
    #[serde(default)]
    url: Option<String>,
}
