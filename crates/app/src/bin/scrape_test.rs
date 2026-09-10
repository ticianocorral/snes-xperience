//! Phase 0 proof #2: ScreenScraper returns `texture` and `wheel` for a sample
//! of ROMs.
//!
//! Usage:
//!   scrape-test --roms <dir> [--limit 20]
//!
//! Credentials come from the environment (plan §4.2 — the user registers their
//! own):
//!   SS_DEVID, SS_DEVPASSWORD   (mandatory; issued by screenscraper.fr)
//!   SS_SOFTNAME                (optional; defaults to "snes-xperience")
//!   SS_USER, SS_PASSWORD       (optional; the user's own account, bigger quota)

use std::path::{Path, PathBuf};

use anyhow::{anyhow, bail, Result};
use xperience_domain::{Client, Credentials, RomId};

const EXTS: [&str; 7] = ["sfc", "smc", "fig", "swc", "bs", "st", "bin"];

struct Args {
    roms: PathBuf,
    limit: usize,
}

fn parse_args() -> Result<Args> {
    let mut roms = None;
    let mut limit = 20usize;
    let mut it = std::env::args().skip(1);
    while let Some(a) = it.next() {
        match a.as_str() {
            "--roms" => {
                roms = Some(PathBuf::from(
                    it.next().ok_or_else(|| anyhow!("--roms needs a value"))?,
                ))
            }
            "--limit" => {
                limit = it
                    .next()
                    .ok_or_else(|| anyhow!("--limit needs a value"))?
                    .parse()
                    .map_err(|_| anyhow!("--limit wants a number"))?
            }
            "-h" | "--help" => {
                println!("scrape-test --roms <dir> [--limit 20]");
                std::process::exit(0);
            }
            other => bail!("unexpected argument: {other}"),
        }
    }
    Ok(Args {
        roms: roms.ok_or_else(|| anyhow!("pass --roms <dir>"))?,
        limit,
    })
}

fn collect_roms(dir: &Path, limit: usize) -> Result<Vec<PathBuf>> {
    let mut v: Vec<PathBuf> = std::fs::read_dir(dir)?
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| {
            p.is_file()
                && p.extension()
                    .and_then(|e| e.to_str())
                    .map(|e| EXTS.contains(&e.to_ascii_lowercase().as_str()))
                    .unwrap_or(false)
        })
        .collect();
    v.sort();
    v.truncate(limit);
    Ok(v)
}

fn main() -> Result<()> {
    let args = parse_args()?;
    let Some(creds) = Credentials::from_env() else {
        bail!(
            "no ScreenScraper credentials.\n\
             Register at https://www.screenscraper.fr/ and export:\n  \
             SS_DEVID=... SS_DEVPASSWORD=... [SS_SOFTNAME=...] [SS_USER=... SS_PASSWORD=...]"
        );
    };

    let roms = collect_roms(&args.roms, args.limit)?;
    if roms.is_empty() {
        bail!(
            "no ROM files ({}) under {}",
            EXTS.join("/"),
            args.roms.display()
        );
    }
    println!(
        "Checking {} ROM(s) against ScreenScraper as dev '{}'{}\n",
        roms.len(),
        creds.dev_id,
        if creds.user.is_some() {
            " (+ user account)"
        } else {
            ""
        }
    );

    let client = Client::new(creds);
    println!(
        "{:<34} {:<26} {:<8} {:<6}",
        "file", "matched name", "texture", "wheel"
    );
    println!("{}", "-".repeat(78));

    let mut both = 0usize;
    let mut matched = 0usize;
    let mut last_quota: Option<(String, String)> = None;

    for path in &roms {
        let name = path
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();
        let short: String = name.chars().take(33).collect();

        let id = match RomId::from_path(path) {
            Ok(id) => id,
            Err(e) => {
                println!("{short:<34} <hash failed: {e}>");
                continue;
            }
        };

        match client.lookup(&id, &name) {
            Ok(media) => {
                matched += 1;
                if media.has_both() {
                    both += 1;
                }
                if let (Some(t), Some(m)) = (&media.requests_today, &media.max_requests_per_day) {
                    last_quota = Some((t.clone(), m.clone()));
                }
                println!(
                    "{:<34} {:<26} {:<8} {:<6}",
                    short,
                    media
                        .canonical_name
                        .as_deref()
                        .unwrap_or("?")
                        .chars()
                        .take(25)
                        .collect::<String>(),
                    yn(media.texture_url.is_some()),
                    yn(media.wheel_url.is_some()),
                );
            }
            Err(e) => println!("{short:<34} <{e}>"),
        }
    }

    println!(
        "\n{} / {} matched a game; {} / {} returned BOTH texture and wheel.",
        matched,
        roms.len(),
        both,
        roms.len()
    );
    if let Some((t, m)) = last_quota {
        println!("ScreenScraper quota: {t} / {m} requests today.");
    }

    // Phase 0 goal: the pipeline works at all.
    if matched == 0 {
        std::process::exit(1);
    }
    Ok(())
}

fn yn(b: bool) -> &'static str {
    if b {
        "yes"
    } else {
        "-"
    }
}
