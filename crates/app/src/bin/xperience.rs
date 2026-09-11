//! The whole thing: selector → game → selector, in one process, no shell glue.
//! In a game, Esc powers off (state, saves, TV to snow, cartridge stays
//! seated) and E ejects once off, opening the shelf; Esc/close on the shelf,
//! or closing a game window, ends the app — no ceremony there (plan §3.3).
//! `O` on the shelf opens settings (controls, ScreenScraper, run-ahead,
//! fullscreen) — see `xperience_app::settings`.
//!
//! Usage:
//!   xperience --core <path/to/snes9x_libretro.{dylib,so,dll}>
//!             [--catalog DB] [--config config.toml] [--save-dir DIR]
//!             [--system-dir DIR] [--order shelf|name] [--runahead N] [--no-scrape]
//!
//! The core path also reads from $XPERIENCE_CORE, or — for a packaged launch
//! with no arguments at all — a core dropped into the data dir's `core/`
//! folder (see --help). Build the catalogue first with
//! `library scan --roms <dir>` (see docs/fase-2.md). ScreenScraper credentials
//! come from the settings screen (saved to config.toml) or, as a fallback,
//! $SS_DEVID/$SS_DEVPASSWORD; --no-scrape opts out either way.

use std::path::{Path, PathBuf};

use anyhow::{anyhow, Context, Result};
use xperience_app::config::Config;
use xperience_app::runner::{run_game, GameExit, GameSpec};
use xperience_app::settings;
use xperience_app::shelf::{self, Pick, ScrapeSetup, ShelfOpts};
use xperience_domain::{Catalog, Order};
use xperience_platform::Platform;

struct Args {
    core: PathBuf,
    catalog: PathBuf,
    config: Option<PathBuf>,
    save_dir: PathBuf,
    system_dir: PathBuf,
    notes_dir: PathBuf,
    order: Order,
    runahead: Option<u32>,
    no_scrape: bool,
    /// Headless: render one settings screen ("main"|"controls"|"screenscraper")
    /// to `--shot` and exit, instead of starting the shelf (dev/testing).
    debug_settings: Option<String>,
    shot: Option<PathBuf>,
}

fn data_dir() -> PathBuf {
    xperience_app::dirs::data_dir()
}

/// The core's file name for the platform this binary was built for — never
/// distributed with the app (non-commercial snes9x license), but a launch
/// with no `--core`/`$XPERIENCE_CORE` (a double-clicked .app/.exe/AppImage
/// has neither) still needs somewhere to look.
fn core_file_name() -> &'static str {
    if cfg!(target_os = "macos") {
        "snes9x_libretro.dylib"
    } else if cfg!(target_os = "windows") {
        "snes9x_libretro.dll"
    } else {
        "snes9x_libretro.so"
    }
}

fn default_core_path() -> Option<PathBuf> {
    let p = data_dir().join("core").join(core_file_name());
    p.is_file().then_some(p)
}

fn parse_args() -> Result<Args> {
    let mut core = std::env::var_os("XPERIENCE_CORE").map(PathBuf::from);
    let mut catalog = None;
    let mut config = None;
    let mut save_dir = None;
    let mut system_dir = None;
    let mut notes_dir = None;
    let mut order = Order::Shelf;
    let mut runahead = None;
    let mut no_scrape = false;
    let mut debug_settings = None;
    let mut shot = None;

    let mut it = std::env::args().skip(1);
    while let Some(a) = it.next() {
        let mut val = || it.next().ok_or_else(|| anyhow!("{a} needs a value"));
        match a.as_str() {
            "--core" => core = Some(val()?.into()),
            "--catalog" => catalog = Some(val()?.into()),
            "--config" => config = Some(val()?.into()),
            "--save-dir" => save_dir = Some(val()?.into()),
            "--system-dir" => system_dir = Some(val()?.into()),
            "--notes-dir" => notes_dir = Some(val()?.into()),
            "--no-scrape" => no_scrape = true,
            "--debug-settings" => debug_settings = Some(val()?),
            "--shot" => shot = Some(val()?.into()),
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

    let core = core.or_else(default_core_path).ok_or_else(|| {
        anyhow!(
            "no core: pass --core, set $XPERIENCE_CORE, or drop {} into {}",
            core_file_name(),
            data_dir().join("core").display()
        )
    })?;
    let save_dir = save_dir.unwrap_or_else(|| data_dir().join("saves"));
    let system_dir = system_dir.unwrap_or_else(|| save_dir.clone());
    let notes_dir = notes_dir.unwrap_or_else(|| data_dir().join("notes"));
    let catalog = catalog.unwrap_or_else(|| data_dir().join("catalog.db"));
    Ok(Args {
        core,
        catalog,
        config,
        save_dir,
        system_dir,
        notes_dir,
        order,
        runahead,
        no_scrape,
        debug_settings,
        shot,
    })
}

const HELP: &str = "xperience --core <lib> [--catalog DB] [--config config.toml]\n\
       [--save-dir DIR] [--system-dir DIR] [--notes-dir DIR]\n\
       [--order shelf|name] [--runahead N] [--no-scrape]\n\
       [--debug-settings main|controls|screenscraper --shot out.bmp]\n\
\n\
Selector → game → selector, one process. In a game: Esc powers off (saves,\n\
TV to snow, cartridge stays put), E ejects once off, opening the shelf.\n\
O on the shelf opens settings (controls, ScreenScraper, run-ahead,\n\
fullscreen) — saved straight to config.toml.\n\
Esc / window-close on the shelf, or closing a game window, ends the app —\n\
no ceremony there.\n\
Build the catalogue first:  library scan --roms <dir>\n\
ScreenScraper credentials come from the settings screen or, as a fallback,\n\
SS_DEVID / SS_DEVPASSWORD; --no-scrape turns scraping off either way.\n\
Defaults live under the data dir (catalog.db, saves/, notes/, core/) —\n\
~/.local/share/snes-xperience on macOS/Linux, %APPDATA%\\snes-xperience on a\n\
native Windows launch with no $HOME. No --core/$XPERIENCE_CORE? Drop the\n\
snes9x core into <data dir>/core/ (snes9x_libretro.dylib/.so/.dll) — not\n\
included, non-commercial license (see THIRD-PARTY-NOTICES.md).";

fn main() -> Result<()> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();
    let args = parse_args()?;

    let mut cfg = Config::load(args.config.as_deref())?;
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

    let build_scrape = |cfg: &Config| -> Option<ScrapeSetup> {
        if args.no_scrape {
            return None;
        }
        cfg.resolve_screenscraper().map(|creds| ScrapeSetup {
            creds,
            art_dir: args.catalog.parent().unwrap_or(Path::new(".")).join("art"),
        })
    };
    let scrape = build_scrape(&cfg);
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

    // Headless self-check: render one settings screen and exit.
    if let (Some(screen), Some(path)) = (&args.debug_settings, &args.shot) {
        settings::capture_preview(&mut cab, &cfg, screen, path)?;
        log::info!("wrote {} (settings preview: {screen})", path.display());
        return Ok(());
    }

    let mut shelf_opts = ShelfOpts {
        order: args.order,
        max_frames: None,
        shot: None,
        scrape,
        fade_in: None,
    };

    loop {
        let (rom, cartridge_label, logo) =
            match shelf::run(&mut plat, &mut cab, &catalog, &shelf_opts)? {
                Pick::Quit => break,
                Pick::Settings => {
                    let quit = settings::run(&mut plat, &mut cab, &mut cfg)?;
                    if quit {
                        break;
                    }
                    // Bindings/ScreenScraper/run-ahead may have changed.
                    shelf_opts.scrape = build_scrape(&cfg);
                    continue;
                }
                Pick::Play {
                    rom,
                    texture,
                    wheel,
                } => (rom, texture, wheel),
            };
        shelf_opts.fade_in = None; // consumed
        let spec = GameSpec {
            core: args.core.clone(),
            rom,
            system_dir: args.system_dir.clone(),
            save_dir: args.save_dir.clone(),
            notes_dir: args.notes_dir.clone(),
            runahead: args.runahead,
            shot: None,
            cartridge_label,
            logo,
            shot_off: false,
            debug_note_capture: false,
            debug_shot_pause: false,
        };
        match run_game(&mut plat, &mut cab, &spec, &cfg)? {
            // The power-off ritual (desligar, snow, wait for eject) already
            // ran inside run_game; the shelf just eases in over what it left.
            GameExit::ToShelf { static_level } => {
                shelf_opts.fade_in = Some(static_level);
            }
            GameExit::Quit => break,
        }
    }

    log::info!("bye");
    Ok(())
}
