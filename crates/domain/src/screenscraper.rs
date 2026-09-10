//! ScreenScraper `jeuInfos.php` client — sections 4.2 and 4.3 of the plan.
//! Matches by hash + size + filename and pulls the ficha fields plus the medias
//! we care about: `box-2D` (cover, for the shelf), `texture` (cut-out cartridge
//! label) and `wheel` (transparent logo).

use std::io::Read;
use std::path::Path;
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
    /// `SS_PASSWORD`. Returns `None` if the mandatory two are absent.
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
    #[error("ScreenScraper quota exhausted")]
    QuotaExhausted,
    #[error("could not parse ScreenScraper response: {0}")]
    Parse(String),
    #[error("saving art: {0}")]
    Io(String),
}

/// Everything the selector's game card can show, plus the frame's medias.
#[derive(Debug, Clone, Default)]
pub struct GameInfo {
    pub game_id: Option<String>,
    pub canonical_name: Option<String>,
    pub region: Option<String>,
    pub year: Option<String>,
    pub developer: Option<String>,
    pub publisher: Option<String>,
    pub genre: Option<String>,
    pub players: Option<String>,
    pub synopsis: Option<String>,
    /// Front cover (`box-2D`).
    pub cover_url: Option<String>,
    /// Cut-out cartridge label (`texture`), for the console model.
    pub texture_url: Option<String>,
    /// Transparent-PNG logo (`wheel`).
    pub wheel_url: Option<String>,
    /// Requests used / allowed today, if the API reported them.
    pub requests_today: Option<String>,
    pub max_requests_per_day: Option<String>,
}

impl GameInfo {
    /// Phase 0 check: both cartridge medias present.
    pub fn has_cartridge_media(&self) -> bool {
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

    pub fn lookup(&self, rom: &RomId, filename: &str) -> Result<GameInfo, ScrapeError> {
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
                let low = body.to_lowercase();
                if code == 404 && low.contains("rom") {
                    return Err(ScrapeError::NotFound);
                }
                if code == 429 || low.contains("quota") || low.contains("maximum threads") {
                    return Err(ScrapeError::QuotaExhausted);
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

    /// Download a media URL (typically appending the dev credentials, which
    /// ScreenScraper wants on media requests too) to `dest`.
    pub fn download(&self, url: &str, dest: &Path) -> Result<(), ScrapeError> {
        let resp = self
            .agent
            .get(url)
            .query("devid", &self.creds.dev_id)
            .query("devpassword", &self.creds.dev_password)
            .query("softname", &self.creds.soft_name)
            .call()
            .map_err(|e| match e {
                ureq::Error::Status(429, _) => ScrapeError::QuotaExhausted,
                other => ScrapeError::Network(other.to_string()),
            })?;
        let mut bytes = Vec::new();
        resp.into_reader()
            .read_to_end(&mut bytes)
            .map_err(|e| ScrapeError::Network(e.to_string()))?;
        if let Some(dir) = dest.parent() {
            std::fs::create_dir_all(dir).map_err(|e| ScrapeError::Io(e.to_string()))?;
        }
        std::fs::write(dest, &bytes).map_err(|e| ScrapeError::Io(e.to_string()))?;
        Ok(())
    }
}

/// Region preference used across names, texts and medias.
const REGIONS: [&str; 5] = ["wor", "us", "eu", "ss", "jp"];
/// Language preference for descriptive text.
const LANGS: [&str; 3] = ["en", "pt", "es"];

fn parse(body: &str) -> Result<GameInfo, ScrapeError> {
    let trimmed = body.trim_start();
    if !trimmed.starts_with('{') {
        let low = trimmed.to_lowercase();
        if low.contains("rom") && low.contains("trouv") {
            return Err(ScrapeError::NotFound);
        }
        if low.contains("quota") || low.contains("maximum") {
            return Err(ScrapeError::QuotaExhausted);
        }
        return Err(ScrapeError::Parse(trimmed.chars().take(300).collect()));
    }

    let root: Root = serde_json::from_str(body).map_err(|e| ScrapeError::Parse(e.to_string()))?;
    let response = root.response.ok_or(ScrapeError::NotFound)?;
    let jeu = response.jeu.ok_or(ScrapeError::NotFound)?;

    let cover_url = pick_media(&jeu.medias, &["box-2D", "box-2d"]);
    let texture_url = pick_media(&jeu.medias, &["texture"]);
    let wheel_url = pick_media(&jeu.medias, &["wheel", "wheel-hd"]);

    let (requests_today, max_requests_per_day) = response
        .ssuser
        .map(|u| (u.requeststoday, u.maxrequestsperday))
        .unwrap_or((None, None));

    Ok(GameInfo {
        game_id: jeu.id.map(|v| v.to_string()),
        canonical_name: pick_regional(&jeu.noms),
        region: jeu
            .medias
            .iter()
            .find(|m| m.kind.as_deref() == Some("box-2D"))
            .and_then(|m| m.region.clone()),
        year: pick_regional(&jeu.dates).and_then(|d| year_of(&d)),
        developer: jeu.developpeur.and_then(|t| t.text),
        publisher: jeu.editeur.and_then(|t| t.text),
        genre: jeu.genres.first().and_then(|g| pick_lang(&g.noms)),
        players: jeu.joueurs.and_then(|t| t.text),
        synopsis: pick_lang(&jeu.synopsis),
        cover_url,
        texture_url,
        wheel_url,
        requests_today,
        max_requests_per_day,
    })
}

fn year_of(date: &str) -> Option<String> {
    let y: String = date.chars().take_while(|c| c.is_ascii_digit()).collect();
    (y.len() == 4).then_some(y)
}

/// First entry matching the region preference (falls back to any).
fn pick_regional(items: &[Regional]) -> Option<String> {
    for want in REGIONS {
        if let Some(t) = items
            .iter()
            .find(|n| n.region.as_deref() == Some(want))
            .and_then(|n| n.text.clone())
        {
            return Some(t);
        }
    }
    items.iter().find_map(|n| n.text.clone())
}

fn pick_lang(items: &[Localized]) -> Option<String> {
    for want in LANGS {
        if let Some(t) = items
            .iter()
            .find(|n| n.langue.as_deref() == Some(want))
            .and_then(|n| n.text.clone())
        {
            return Some(t);
        }
    }
    items.iter().find_map(|n| n.text.clone())
}

fn pick_media(medias: &[Media], kinds: &[&str]) -> Option<String> {
    for kind in kinds {
        for want in REGIONS {
            if let Some(m) = medias
                .iter()
                .find(|m| m.kind.as_deref() == Some(*kind) && m.region.as_deref() == Some(want))
            {
                return m.url.clone();
            }
        }
        if let Some(m) = medias.iter().find(|m| m.kind.as_deref() == Some(*kind)) {
            return m.url.clone();
        }
    }
    None
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
    noms: Vec<Regional>,
    #[serde(default)]
    dates: Vec<Regional>,
    #[serde(default)]
    developpeur: Option<Text>,
    #[serde(default)]
    editeur: Option<Text>,
    #[serde(default)]
    joueurs: Option<Text>,
    #[serde(default)]
    genres: Vec<Genre>,
    #[serde(default)]
    synopsis: Vec<Localized>,
    #[serde(default)]
    medias: Vec<Media>,
}

/// `{ "region": "...", "text": "..." }` (name / date entries; `nom` alias seen).
#[derive(Deserialize)]
struct Regional {
    #[serde(default)]
    region: Option<String>,
    #[serde(default, alias = "nom")]
    text: Option<String>,
}

/// `{ "langue": "en", "text": "..." }` (synopsis / genre names).
#[derive(Deserialize)]
struct Localized {
    #[serde(default)]
    langue: Option<String>,
    #[serde(default)]
    text: Option<String>,
}

/// `{ "id": "...", "text": "..." }` (developer / publisher / players).
#[derive(Deserialize)]
struct Text {
    #[serde(default)]
    text: Option<String>,
}

#[derive(Deserialize)]
struct Genre {
    #[serde(default)]
    noms: Vec<Localized>,
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
