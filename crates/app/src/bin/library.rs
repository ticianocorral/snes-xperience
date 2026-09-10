//! Phase 2 backbone: build and fill the game catalogue (plan §4.2). No UI yet.
//!
//!   library scan   --roms DIR   [--catalog PATH] [--prune]
//!   library scrape               [--catalog PATH] [--limit N] [--art DIR] [--sleep MS]
//!   library list                 [--catalog PATH] [--order shelf|name]
//!
//! Catalogue defaults to $HOME/.local/share/snes-xperience/catalog.db, art to
//! <catalog dir>/art. `scrape` needs SS_DEVID / SS_DEVPASSWORD in the env.

use std::path::{Path, PathBuf};
use std::time::Duration;

use anyhow::{anyhow, bail, Context, Result};
use xperience_domain::{
    download_art, library, Catalog, Client, Credentials, Order, RomId, ScrapeError,
};

fn main() -> Result<()> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();
    let mut args = std::env::args().skip(1);
    let cmd = args.next().unwrap_or_default();
    let rest: Vec<String> = args.collect();
    match cmd.as_str() {
        "scan" => scan(&rest),
        "scrape" => scrape(&rest),
        "list" => list(&rest),
        "-h" | "--help" | "" => {
            println!("{HELP}");
            Ok(())
        }
        other => bail!("unknown command {other:?}\n\n{HELP}"),
    }
}

const HELP: &str = "library scan   --roms DIR   [--catalog PATH] [--prune]\n\
library scrape               [--catalog PATH] [--limit N] [--art DIR] [--sleep MS]\n\
library list                 [--catalog PATH] [--order shelf|name]";

fn opt<'a>(args: &'a [String], key: &str) -> Option<&'a str> {
    args.iter()
        .position(|a| a == key)
        .and_then(|i| args.get(i + 1))
        .map(String::as_str)
}
fn flag(args: &[String], key: &str) -> bool {
    args.iter().any(|a| a == key)
}

fn default_catalog() -> PathBuf {
    let base = std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_default();
    base.join(".local/share/snes-xperience/catalog.db")
}

fn catalog_of(args: &[String]) -> Result<Catalog> {
    let path = opt(args, "--catalog")
        .map(PathBuf::from)
        .unwrap_or_else(default_catalog);
    Catalog::open(&path).with_context(|| format!("opening catalogue {}", path.display()))
}

fn scan(args: &[String]) -> Result<()> {
    let roms = opt(args, "--roms").ok_or_else(|| anyhow!("scan needs --roms DIR"))?;
    let cat = catalog_of(args)?;

    let found = library::scan(Path::new(roms)).context("scanning ROM folder")?;
    let mut added = 0usize;
    for r in &found {
        if cat.upsert_rom(r)? {
            added += 1;
        }
    }
    let pruned = if flag(args, "--prune") {
        let present: Vec<String> = found.iter().map(|r| r.id.sha1.clone()).collect();
        cat.prune_missing(&present)?
    } else {
        0
    };
    let (total, scraped) = cat.counts()?;
    println!(
        "scanned {} file(s): {added} new, {pruned} pruned. catalogue: {total} rom(s), {scraped} scraped.",
        found.len()
    );
    Ok(())
}

fn scrape(args: &[String]) -> Result<()> {
    let Some(creds) = Credentials::from_env() else {
        bail!("no ScreenScraper credentials — export SS_DEVID / SS_DEVPASSWORD");
    };
    let cat = catalog_of(args)?;
    let limit: usize = opt(args, "--limit").map_or(Ok(20), str::parse)?;
    let sleep_ms: u64 = opt(args, "--sleep").map_or(Ok(500), str::parse)?;
    let art_dir = opt(args, "--art").map(PathBuf::from).unwrap_or_else(|| {
        default_catalog()
            .parent()
            .unwrap_or(Path::new("."))
            .join("art")
    });

    let pending = cat.unscraped(limit)?;
    if pending.is_empty() {
        println!("nothing to scrape.");
        return Ok(());
    }
    println!(
        "scraping {} rom(s) as dev '{}'…",
        pending.len(),
        creds.dev_id
    );
    let client = Client::new(creds);

    let mut ok = 0usize;
    for row in &pending {
        let path = Path::new(&row.path);
        let filename = path
            .file_name()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_default();
        let id = match RomId::from_path(path) {
            Ok(id) if id.sha1 == row.sha1 => id,
            Ok(_) => {
                log::warn!("{filename}: file changed on disk, skipping");
                continue;
            }
            Err(e) => {
                log::warn!("{filename}: {e}");
                continue;
            }
        };

        match client.lookup(&id, &filename) {
            Ok(info) => {
                let art = download_art(&client, &art_dir, &row.sha1, &info);
                cat.set_meta(&row.sha1, &info, &art)?;
                ok += 1;
                println!(
                    "  {:<32} {:<24} {}{}{}",
                    trunc(&filename, 32),
                    trunc(info.canonical_name.as_deref().unwrap_or("?"), 24),
                    if art.cover.is_some() { "cover " } else { "" },
                    if art.texture.is_some() {
                        "texture "
                    } else {
                        ""
                    },
                    if art.wheel.is_some() { "wheel" } else { "" },
                );
                if let (Some(t), Some(m)) = (&info.requests_today, &info.max_requests_per_day) {
                    log::debug!("quota {t}/{m}");
                }
            }
            Err(ScrapeError::NotFound) => {
                log::info!("{filename}: not in ScreenScraper");
            }
            Err(ScrapeError::QuotaExhausted) => {
                println!("  quota exhausted — stopping.");
                break;
            }
            Err(e) => log::warn!("{filename}: {e}"),
        }
        std::thread::sleep(Duration::from_millis(sleep_ms));
    }

    let (total, scraped) = cat.counts()?;
    println!("done: {ok} scraped this run. catalogue: {total} rom(s), {scraped} scraped.");
    Ok(())
}

fn list(args: &[String]) -> Result<()> {
    let cat = catalog_of(args)?;
    let order = match opt(args, "--order") {
        Some("name") => Order::Name,
        _ => Order::Shelf,
    };
    let entries = cat.list(order)?;
    println!(
        "{:<34} {:<6} {:<18} {:<8} art",
        "title", "year", "developer", "plays"
    );
    println!("{}", "-".repeat(78));
    for e in &entries {
        let m = e.meta.as_ref();
        println!(
            "{:<34} {:<6} {:<18} {:<8} {}",
            trunc(&e.title(), 34),
            m.and_then(|m| m.year.as_deref()).unwrap_or("-"),
            trunc(m.and_then(|m| m.developer.as_deref()).unwrap_or("-"), 18),
            e.rom.play_count,
            m.map(|m| {
                let mut s = String::new();
                if m.cover_path.is_some() {
                    s.push_str("cover ");
                }
                if m.texture_path.is_some() {
                    s.push_str("texture ");
                }
                if m.wheel_path.is_some() {
                    s.push_str("wheel");
                }
                if s.is_empty() {
                    "-".into()
                } else {
                    s
                }
            })
            .unwrap_or_else(|| "unscraped".into()),
        );
    }
    println!("\n{} rom(s).", entries.len());
    Ok(())
}

fn trunc(s: &str, n: usize) -> String {
    s.chars().take(n).collect()
}
