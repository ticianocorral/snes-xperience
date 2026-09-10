//! The whole thing: selector → game → selector, in one process, no shell glue.
//! Leaving a game (Esc) drops back to the shelf; quitting the shelf (Esc/close)
//! or closing a game window (Cmd-Q / red button) ends the app.
//!
//! Usage:
//!   xperience --core <path/to/snes9x_libretro.{dylib,so,dll}>
//!             [--catalog DB] [--config config.toml] [--save-dir DIR]
//!             [--system-dir DIR] [--order shelf|name] [--runahead N] [--no-scrape]
//!
//! The core path also reads from $XPERIENCE_CORE. Build the catalogue first with
//! `library scan --roms <dir>` (see docs/fase-2.md). With SS_DEVID /
//! SS_DEVPASSWORD set, the shelf scrapes the game you rest on; --no-scrape opts
//! out.

use std::path::{Path, PathBuf};

use anyhow::{anyhow, Context, Result};
use xperience_app::config::Config;
use xperience_app::runner::{run_game, GameExit, GameSpec};
use xperience_app::shelf::{self, Pick, ScrapeSetup, ShelfOpts};
use xperience_domain::{Catalog, Credentials, Order};
use xperience_platform::Platform;

struct Args {
    core: PathBuf,
    catalog: PathBuf,
    config: Option<PathBuf>,
    save_dir: PathBuf,
    system_dir: PathBuf,
    order: Order,
    runahead: Option<u32>,
    no_scrape: bool,
}

fn data_dir() -> PathBuf {
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_default()
        .join(".local/share/snes-xperience")
}

fn parse_args() -> Result<Args> {
    let mut core = std::env::var_os("XPERIENCE_CORE").map(PathBuf::from);
    let mut catalog = None;
    let mut config = None;
    let mut save_dir = None;
    let mut system_dir = None;
    let mut order = Order::Shelf;
    let mut runahead = None;
    let mut no_scrape = false;

    let mut it = std::env::args().skip(1);
    while let Some(a) = it.next() {
        let mut val = || it.next().ok_or_else(|| anyhow!("{a} needs a value"));
        match a.as_str() {
            "--core" => core = Some(val()?.into()),
            "--catalog" => catalog = Some(val()?.into()),
            "--config" => config = Some(val()?.into()),
            "--save-dir" => save_dir = Some(val()?.into()),
            "--system-dir" => system_dir = Some(val()?.into()),
            "--no-scrape" => no_scrape = true,
            "--order" => {
                order = match val()?.as_str() {
                    "name" => Order::Name,
                    "shelf" => Order::Shelf,
                    other => anyhow::bail!("--order wants shelf|name, got {other:?}"),
                }
            }
            "--runahead" => {
                runahead = Some(
                    val()?
                        .parse()
                        .map_err(|_| anyhow!("--runahead wants a number (0 disables)"))?,
                )
            }
            "-h" | "--help" => {
                println!("{HELP}");
                std::process::exit(0);
            }
            other => anyhow::bail!("unexpected argument: {other}"),
        }
    }

    let core = core.ok_or_else(|| anyhow!("no core: pass --core or set $XPERIENCE_CORE"))?;
    let save_dir = save_dir.unwrap_or_else(|| data_dir().join("saves"));
    let system_dir = system_dir.unwrap_or_else(|| save_dir.clone());
    let catalog = catalog.unwrap_or_else(|| data_dir().join("catalog.db"));
    Ok(Args {
        core,
        catalog,
        config,
        save_dir,
        system_dir,
        order,
        runahead,
        no_scrape,
    })
}

const HELP: &str = "xperience --core <lib> [--catalog DB] [--config config.toml]\n\
       [--save-dir DIR] [--system-dir DIR] [--order shelf|name] [--runahead N]\n\
       [--no-scrape]\n\
\n\
Selector → game → selector, one process. Esc in a game returns to the shelf;\n\
Esc / window-close on the shelf, or closing a game window, ends the app.\n\
Build the catalogue first:  library scan --roms <dir>\n\
With SS_DEVID / SS_DEVPASSWORD set, the shelf scrapes the focused game;\n\
--no-scrape turns that off.\n\
Defaults live under ~/.local/share/snes-xperience/ (catalog.db, saves/).";

fn main() -> Result<()> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();
    let args = parse_args()?;

    let cfg = Config::load(args.config.as_deref())?;
    if let Some(p) = &cfg.source {
        log::info!("config: {}", p.display());
    }

    let catalog = Catalog::open(&args.catalog)
        .with_context(|| format!("opening catalogue {}", args.catalog.display()))?;
    let (n_roms, _) = catalog.counts()?;
    if n_roms == 0 {
        eprintln!(
            "catalogue is empty — run:  library scan --roms <dir> --catalog {}",
            args.catalog.display()
        );
        std::process::exit(2);
    }

    let scrape = (!args.no_scrape)
        .then(Credentials::from_env)
        .flatten()
        .map(|creds| ScrapeSetup {
            creds,
            art_dir: args.catalog.parent().unwrap_or(Path::new(".")).join("art"),
        });
    if scrape.is_some() {
        log::info!("on-demand scrape: on");
    }

    let mut plat = Platform::new().map_err(|e| anyhow!(e.to_string()))?;
    // One window for the whole session — shelf and game both draw into it.
    let mut cab = plat
        .create_cabinet("SNES Xperience", 1280, 800)
        .map_err(|e| anyhow!(e.to_string()))?;
    if cfg.fullscreen {
        cab.toggle_fullscreen();
    }
    let shelf_opts = ShelfOpts {
        order: args.order,
        max_frames: None,
        scrape,
    };

    loop {
        let rom = match shelf::run(&mut plat, &mut cab, &catalog, &shelf_opts)? {
            Pick::Quit => break,
            Pick::Play(p) => p,
        };
        let spec = GameSpec {
            core: args.core.clone(),
            rom,
            system_dir: args.system_dir.clone(),
            save_dir: args.save_dir.clone(),
            runahead: args.runahead,
            shot: None,
        };
        match run_game(&mut plat, &mut cab, &spec, &cfg)? {
            GameExit::ToShelf => continue,
            GameExit::Quit => break,
        }
    }

    log::info!("bye");
    Ok(())
}
