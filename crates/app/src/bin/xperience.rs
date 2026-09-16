//! The whole thing: idle → selector → game → idle → …, in one process, no
//! shell glue, no installation. Portable: `roms/`, `core/`, `assets/`,
//! `saves/`, `notes/`, `xperience.cfg` and `library.json` all live in one
//! root (see `xperience_app::dirs`) — next to the executable on
//! Windows/Linux, `~/Documents/SNES Xperience` on macOS — drop ROMs in
//! `roms/` and go.
//!
//! The idle screen (TV off, "Inserir cartucho"/"Configuracoes" in place of
//! the logo) is the app's home: it's what you see at startup, after backing
//! out of the shelf, and after ejecting a game — only closing the window
//! ends the app. Nothing here uses the keyboard (plan revision:
//! mouse/gamepad only) — in a game, the panel's Power button powers off
//! (state, saves, TV to snow) and back on again, and Eject only takes once
//! off, landing back on the idle screen (plan §3.3). "Configuracoes" (idle
//! screen or shelf) opens settings (controls, run-ahead, fullscreen, snes9x
//! core download/update) — see `xperience_app::settings`.
//!
//! Usage:
//!   xperience [--core path/to/snes9x_libretro.{dylib,so,dll}]
//!             [--config xperience.cfg] [--save-dir DIR] [--system-dir DIR]
//!             [--order shelf|name] [--runahead N]
//!
//! No `--core`/`$XPERIENCE_CORE`? Looks for one already downloaded into
//! `core/` (see the settings screen, "Núcleo") — not included in the app
//! itself, non-commercial snes9x license (see THIRD-PARTY-NOTICES.md).

use std::path::{Path, PathBuf};

use anyhow::{anyhow, Context, Result};
use xperience_app::config::Config;
use xperience_app::core_update;
use xperience_app::idle::{self, IdleExit};
use xperience_app::runner::{run_game, GameExit, GameSpec};
use xperience_app::settings;
use xperience_app::shelf::{self, Pick, ShelfOpts};
use xperience_domain::{Catalog, NoIntroDat, Order};
use xperience_platform::{Cabinet, MenuMode, MenuNav, Platform, Screen};

struct Args {
    /// Explicit override; `None` means "look in `core/` at launch, and again
    /// each time a game is picked" (the settings screen may have just
    /// downloaded one).
    core: Option<PathBuf>,
    config: Option<PathBuf>,
    save_dir: PathBuf,
    system_dir: PathBuf,
    notes_dir: PathBuf,
    order: Order,
    runahead: Option<u32>,
    /// Headless: render one settings screen ("main"|"controls") to `--shot`
    /// and exit, instead of starting the shelf (dev/testing).
    debug_settings: Option<String>,
    shot: Option<PathBuf>,
}

fn default_core_path() -> Option<PathBuf> {
    let p = xperience_app::dirs::core_dir().join(core_update::core_file_name());
    p.is_file().then_some(p)
}

fn parse_args() -> Result<Args> {
    let mut core = std::env::var_os("XPERIENCE_CORE").map(PathBuf::from);
    let mut config = None;
    let mut save_dir = None;
    let mut system_dir = None;
    let mut notes_dir = None;
    let mut order = Order::Shelf;
    let mut runahead = None;
    let mut debug_settings = None;
    let mut shot = None;

    let mut it = std::env::args().skip(1);
    while let Some(a) = it.next() {
        let mut val = || it.next().ok_or_else(|| anyhow!("{a} needs a value"));
        match a.as_str() {
            "--core" => core = Some(val()?.into()),
            "--config" => config = Some(val()?.into()),
            "--save-dir" => save_dir = Some(val()?.into()),
            "--system-dir" => system_dir = Some(val()?.into()),
            "--notes-dir" => notes_dir = Some(val()?.into()),
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

    let save_dir = save_dir.unwrap_or_else(xperience_app::dirs::saves_dir);
    let system_dir = system_dir.unwrap_or_else(|| save_dir.clone());
    let notes_dir = notes_dir.unwrap_or_else(xperience_app::dirs::notes_dir);
    Ok(Args {
        core,
        config,
        save_dir,
        system_dir,
        notes_dir,
        order,
        runahead,
        debug_settings,
        shot,
    })
}

const HELP: &str = "xperience [--core <lib>] [--config xperience.cfg]\n\
       [--save-dir DIR] [--system-dir DIR] [--notes-dir DIR]\n\
       [--order shelf|name] [--runahead N]\n\
       [--debug-settings main|controls --shot out.bmp]\n\
\n\
Idle (TV off) → selector → game → idle, one process. Mouse/gamepad only —\n\
no keyboard shortcut for anything but gameplay input (D-pad/buttons). The\n\
idle screen's \"Inserir cartucho\" opens the shelf; it's what you land on at\n\
startup, after backing out of the shelf, and after ejecting a game. In a\n\
game, the panel's Power button powers off (saves, TV to snow) and back on\n\
again; Eject only takes once off, back to idle.\n\
\"Configuracoes\" (idle screen or shelf) opens settings (controls, núcleo\n\
snes9x, run-ahead, fullscreen) — saved straight to xperience.cfg.\n\
Window-close on the idle screen, or closing a game window, ends the\n\
app — no ceremony there.\n\
\n\
Portable: roms/, core/, assets/ (cover/logo art, matched by ROM file name),\n\
saves/, notes/, xperience.cfg, library.json all live in one root — next to\n\
this executable on Windows/Linux, ~/Documents/SNES Xperience on macOS.\n\
Drop ROMs into roms/ and go; no --core/$XPERIENCE_CORE? Use\n\
the settings screen's \"Núcleo\" to download snes9x automatically, or drop\n\
it into core/ by hand (not included — non-commercial license, see\n\
THIRD-PARTY-NOTICES.md). An optional nointro.dat at the root gives games\n\
their canonical No-Intro name.";

fn main() -> Result<()> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();
    let args = parse_args()?;

    for dir in [
        xperience_app::dirs::roms_dir(),
        xperience_app::dirs::core_dir(),
        xperience_app::dirs::assets_dir().join("logo"),
        xperience_app::dirs::assets_dir().join("cover"),
        xperience_app::dirs::assets_dir().join("cartridge"),
        args.save_dir.clone(),
        args.notes_dir.clone(),
    ] {
        let _ = std::fs::create_dir_all(&dir);
    }
    migrate_old_data();

    let mut cfg = Config::load(args.config.as_deref())?;
    if let Some(p) = &cfg.source {
        log::info!("config: {}", p.display());
    }

    let dat = match NoIntroDat::load(&xperience_app::dirs::nointro_dat_path()) {
        Ok(dat) => Some(dat),
        Err(e) => {
            log::info!("no-intro DAT not loaded ({e}) — using internal/file names");
            None
        }
    };
    let catalog = Catalog::open(
        &xperience_app::dirs::roms_dir(),
        &xperience_app::dirs::library_path(),
        dat.as_ref(),
    )
    .with_context(|| "opening the catalog")?;
    log::info!("{} rom(s) in roms/", catalog.counts()?);

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
        fade_in: None,
    };
    let mut idle_static = idle::RESTING_STATIC;
    let mut core_path = args.core.clone().or_else(default_core_path);

    'app: loop {
        match idle::run(&mut plat, &mut cab, idle_static)? {
            IdleExit::Quit => break 'app,
            IdleExit::OpenShelf => shelf_opts.fade_in = Some(idle_static),
            IdleExit::OpenSettings => {
                if settings::run(&mut plat, &mut cab, &mut cfg)? {
                    break 'app;
                }
                // A core download may have just finished.
                core_path = args.core.clone().or_else(default_core_path);
                continue 'app;
            }
        }

        'shelf: loop {
            let (rom, logo, cartridge) =
                match shelf::run(&mut plat, &mut cab, &catalog, &shelf_opts)? {
                    Pick::Quit => break 'app,
                    Pick::Back => {
                        idle_static = idle::RESTING_STATIC;
                        break 'shelf;
                    }
                    Pick::Settings => {
                        let quit = settings::run(&mut plat, &mut cab, &mut cfg)?;
                        if quit {
                            break 'app;
                        }
                        // A core download may have just finished.
                        core_path = args.core.clone().or_else(default_core_path);
                        continue;
                    }
                    Pick::Play {
                        rom,
                        wheel,
                        cartridge,
                    } => (rom, wheel, cartridge),
                };
            shelf_opts.fade_in = None; // consumed

            let Some(core) = &core_path else {
                if no_core_screen(&mut plat, &mut cab)? {
                    break 'app;
                }
                continue;
            };
            let spec = GameSpec {
                core: core.clone(),
                rom,
                system_dir: args.system_dir.clone(),
                save_dir: args.save_dir.clone(),
                notes_dir: args.notes_dir.clone(),
                runahead: args.runahead,
                shot: None,
                logo,
                cartridge,
                shot_off: false,
                debug_note_capture: false,
                debug_shot_pause: false,
                debug_shot_modal: None,
            };
            match run_game(&mut plat, &mut cab, &spec, &cfg)? {
                // The power-off ritual (desligar, snow, wait for eject)
                // already ran inside run_game — the idle screen just eases
                // in over what it left (plan §3.3, "a estante entra por
                // cima" now applies to the idle screen, not the shelf).
                GameExit::Ejected { static_level } => {
                    idle_static = static_level;
                    break 'shelf;
                }
                GameExit::Quit => break 'app,
            }
        }
    }

    log::info!("bye");
    Ok(())
}

/// A game was picked but no core is available yet — points at the settings
/// screen instead of crashing the whole app (which used to happen at
/// startup, before there was anything to download a core *from*). Returns
/// `true` if the whole app should quit.
fn no_core_screen(plat: &mut Platform, cab: &mut Cabinet) -> Result<bool> {
    loop {
        let m = plat.poll_menu(MenuMode::Nav);
        if m.quit {
            return Ok(true);
        }
        if m.nav
            .iter()
            .any(|n| matches!(n, MenuNav::Confirm | MenuNav::Back))
        {
            return Ok(false);
        }
        let render = |d: &mut Screen| {
            d.text(40, 40, 2, (232, 232, 232), "nucleo nao encontrado");
            d.text_wrapped(
                40,
                90,
                700,
                1,
                (150, 150, 158),
                "baixe o nucleo snes9x pelo menu de configuracoes (O na estante) \
                 antes de jogar, ou coloque o arquivo em core/ a mao.",
            );
            d.text(
                40,
                d.size().1 as i32 - 40,
                1,
                (150, 150, 158),
                "enter ou esc: voltar pra estante",
            );
        };
        cab.frame_2d((18, 18, 20), render);
        std::thread::sleep(std::time::Duration::from_millis(16));
    }
}

/// One-time, best-effort copy of real play progress (saves/notebooks) from
/// the pre-portable `~/.local/share/snes-xperience` location, if the new
/// folders are still empty and the old ones exist. The catalog's play-count
/// history is deliberately NOT migrated (fresh start) — but a save state or
/// SRAM file is actual game progress, worth not losing silently just because
/// the app changed where it looks. macOS/Linux only (old installs on native
/// Windows used `%APPDATA%`, not covered here — lower value to chase).
fn migrate_old_data() {
    if let Some(home) = std::env::var_os("HOME") {
        let old_base = PathBuf::from(home).join(".local/share/snes-xperience");
        migrate_pairs([
            (old_base.join("saves"), xperience_app::dirs::saves_dir()),
            (old_base.join("notes"), xperience_app::dirs::notes_dir()),
        ]);
    }
    migrate_macos_bundle_sibling();
}

/// macOS only, and only relevant for the brief window before `dirs::app_root`
/// moved to `~/Documents/SNES Xperience`: the very first 0.4.0 DMG builds
/// created `roms/`/`core/`/`assets/`/`saves/`/`notes/` next to the `.app`
/// (typically inside `/Applications`, not writable/expected for user data on
/// this platform). If that old layout exists next to the running `.app` and
/// the new Documents folders are still empty, copy it over once.
#[cfg(target_os = "macos")]
fn migrate_macos_bundle_sibling() {
    use std::ffi::OsStr;
    let Ok(exe) = std::env::current_exe() else {
        return;
    };
    let dir = exe.parent().unwrap_or_else(|| Path::new("."));
    let old_root = (|| {
        if dir.file_name() != Some(OsStr::new("MacOS")) {
            return None;
        }
        let contents = dir.parent()?;
        if contents.file_name() != Some(OsStr::new("Contents")) {
            return None;
        }
        let bundle = contents.parent()?;
        if bundle.extension() != Some(OsStr::new("app")) {
            return None;
        }
        bundle.parent().map(Path::to_path_buf)
    })();
    let Some(old_root) = old_root else {
        return; // dev build, not a packaged .app — nothing to migrate
    };
    migrate_pairs([
        (old_root.join("roms"), xperience_app::dirs::roms_dir()),
        (old_root.join("core"), xperience_app::dirs::core_dir()),
        (old_root.join("assets"), xperience_app::dirs::assets_dir()),
        (old_root.join("saves"), xperience_app::dirs::saves_dir()),
        (old_root.join("notes"), xperience_app::dirs::notes_dir()),
    ]);
}

#[cfg(not(target_os = "macos"))]
fn migrate_macos_bundle_sibling() {}

fn migrate_pairs<const N: usize>(pairs: [(PathBuf, PathBuf); N]) {
    for (old_dir, new_dir) in pairs {
        if !old_dir.is_dir() {
            continue;
        }
        let new_has_data = std::fs::read_dir(&new_dir)
            .map(|mut it| it.next().is_some())
            .unwrap_or(false);
        if new_has_data {
            continue;
        }
        match copy_dir_all(&old_dir, &new_dir) {
            Ok(()) => log::info!("migrated {} -> {}", old_dir.display(), new_dir.display()),
            Err(e) => log::warn!("migrating {}: {e}", old_dir.display()),
        }
    }
}

fn copy_dir_all(src: &Path, dst: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(dst)?;
    for entry in std::fs::read_dir(src)? {
        let entry = entry?;
        let dst_path = dst.join(entry.file_name());
        if entry.file_type()?.is_dir() {
            copy_dir_all(&entry.path(), &dst_path)?;
        } else {
            std::fs::copy(entry.path(), &dst_path)?;
        }
    }
    Ok(())
}
