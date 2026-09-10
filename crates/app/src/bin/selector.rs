//! Phase 2 selector: a scrollable shelf of covers with a details panel,
//! gamepad-first navigation and type-to-search. On confirm it prints the chosen
//! ROM path to stdout and exits 0; on cancel it exits 1. The shelf itself lives
//! in `xperience_app::shelf`, shared with the unified `xperience` binary.
//!
//! With SS_DEVID / SS_DEVPASSWORD in the environment, the game you rest on is
//! scraped on the spot (disable with --no-scrape).

use std::path::{Path, PathBuf};

use anyhow::{anyhow, Context, Result};
use xperience_app::shelf::{self, Pick, ScrapeSetup, ShelfOpts};
use xperience_domain::{Catalog, Credentials, Order};
use xperience_platform::Platform;

fn default_catalog() -> PathBuf {
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_default()
        .join(".local/share/snes-xperience/catalog.db")
}

fn main() -> Result<()> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("warn")).init();

    let args: Vec<String> = std::env::args().skip(1).collect();
    let catalog_path = arg(&args, "--catalog")
        .map(PathBuf::from)
        .unwrap_or_else(default_catalog);
    let order = match arg(&args, "--order") {
        Some("name") => Order::Name,
        _ => Order::Shelf,
    };
    // Headless smoke test: run N frames then exit 0 without picking anything.
    let max_frames: Option<u64> = arg(&args, "--frames").and_then(|s| s.parse().ok());
    let no_scrape = args.iter().any(|a| a == "--no-scrape");

    let catalog = Catalog::open(&catalog_path)
        .with_context(|| format!("opening catalogue {}", catalog_path.display()))?;

    let scrape = (!no_scrape)
        .then(Credentials::from_env)
        .flatten()
        .map(|creds| ScrapeSetup {
            creds,
            art_dir: art_dir_for(&catalog_path),
        });

    let mut plat = Platform::new().map_err(|e| anyhow!(e.to_string()))?;
    let opts = ShelfOpts {
        order,
        max_frames,
        scrape,
    };
    match shelf::run(&mut plat, &catalog, &opts)? {
        Pick::Play(path) => {
            println!("{}", path.display());
            Ok(())
        }
        // The `--frames` smoke test ends here too; that's a clean exit, not a cancel.
        Pick::Quit if max_frames.is_some() => Ok(()),
        Pick::Quit => std::process::exit(1),
    }
}

/// `<catalog dir>/art`, matching the `library` CLI's default.
fn art_dir_for(catalog_path: &Path) -> PathBuf {
    catalog_path.parent().unwrap_or(Path::new(".")).join("art")
}

fn arg<'a>(args: &'a [String], key: &str) -> Option<&'a str> {
    args.iter()
        .position(|a| a == key)
        .and_then(|i| args.get(i + 1))
        .map(String::as_str)
}
