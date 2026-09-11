//! The bare emulator: a libretro core loads and runs, with video, sound, a pad,
//! save states, battery SRAM and run-ahead — no bezel, no selector.
//! Presentation is fixed: RF NTSC + CRT-tube warp.
//!
//! Usage:
//!   emu-run --core <path/to/snes9x_libretro.{dylib,so,dll}> --rom <game.sfc>
//!           [--system-dir DIR] [--save-dir DIR] [--runahead N]
//!
//! The core path also reads from $XPERIENCE_CORE. See docs/fase-0.md for where
//! to get the core, docs/fase-1.md for save states / SRAM / run-ahead. The
//! run-loop itself lives in `xperience_app::runner`, shared with `xperience`.

use std::path::PathBuf;

use anyhow::{anyhow, bail, Result};
use xperience_app::config::Config;
use xperience_app::runner::{run_game, GameSpec};

struct Args {
    core: PathBuf,
    rom: PathBuf,
    system_dir: PathBuf,
    save_dir: PathBuf,
    /// Per-ROM notebooks (plan §3.4); defaults to `<save-dir>/notes`.
    notes_dir: PathBuf,
    config: Option<PathBuf>,
    /// Speculative frames past the shown one; `None` = take the config value.
    runahead: Option<u32>,
    /// Headless self-check: run N frames, save the composited window, exit.
    shot: Option<PathBuf>,
    shot_frame: u32,
    /// Cartridge label art for the slot on the cabinet (dev/testing).
    cartridge_label: Option<PathBuf>,
    /// Logo art for the side panel (dev/testing).
    logo: Option<PathBuf>,
    /// Headless: preview the idle "console off" screen instead of gameplay.
    shot_off: bool,
    /// Headless: force one note capture on the first frame (dev/testing).
    debug_note_capture: bool,
}

fn parse_args() -> Result<Args> {
    let mut core = std::env::var_os("XPERIENCE_CORE").map(PathBuf::from);
    let mut rom = None;
    let mut system_dir = None;
    let mut save_dir = None;
    let mut notes_dir = None;
    let mut config = None;
    let mut runahead = None;
    let mut shot = None;
    let mut shot_frame = 180u32;
    let mut cartridge_label = None;
    let mut logo = None;
    let mut shot_off = false;
    let mut debug_note_capture = false;

    let mut it = std::env::args().skip(1);
    while let Some(a) = it.next() {
        match a.as_str() {
            "--core" => {
                core = Some(
                    it.next()
                        .ok_or_else(|| anyhow!("--core needs a value"))?
                        .into(),
                )
            }
            "--rom" => {
                rom = Some(
                    it.next()
                        .ok_or_else(|| anyhow!("--rom needs a value"))?
                        .into(),
                )
            }
            "--system-dir" => {
                system_dir = Some(
                    it.next()
                        .ok_or_else(|| anyhow!("--system-dir needs a value"))?
                        .into(),
                )
            }
            "--save-dir" => {
                save_dir = Some(
                    it.next()
                        .ok_or_else(|| anyhow!("--save-dir needs a value"))?
                        .into(),
                )
            }
            "--config" => {
                config = Some(
                    it.next()
                        .ok_or_else(|| anyhow!("--config needs a path"))?
                        .into(),
                )
            }
            "--runahead" => {
                runahead = Some(
                    it.next()
                        .ok_or_else(|| anyhow!("--runahead needs a number"))?
                        .parse()
                        .map_err(|_| anyhow!("--runahead wants a number (0 disables)"))?,
                )
            }
            "--shot" => {
                shot = Some(
                    it.next()
                        .ok_or_else(|| anyhow!("--shot needs a path (.bmp)"))?
                        .into(),
                )
            }
            "--shot-frame" => {
                shot_frame = it
                    .next()
                    .ok_or_else(|| anyhow!("--shot-frame needs a number"))?
                    .parse()
                    .map_err(|_| anyhow!("--shot-frame wants a number"))?
            }
            "--cartridge-label" => {
                cartridge_label = Some(
                    it.next()
                        .ok_or_else(|| anyhow!("--cartridge-label needs a path (image)"))?
                        .into(),
                )
            }
            "--logo" => {
                logo = Some(
                    it.next()
                        .ok_or_else(|| anyhow!("--logo needs a path (image)"))?
                        .into(),
                )
            }
            "--shot-off" => shot_off = true,
            "--notes-dir" => {
                notes_dir = Some(
                    it.next()
                        .ok_or_else(|| anyhow!("--notes-dir needs a path"))?
                        .into(),
                )
            }
            "--debug-note-capture" => debug_note_capture = true,
            "-h" | "--help" => {
                println!("{}", HELP);
                std::process::exit(0);
            }
            other => bail!("unexpected argument: {other}"),
        }
    }

    let core = core.ok_or_else(|| anyhow!("no core: pass --core or set $XPERIENCE_CORE"))?;
    let rom = rom.ok_or_else(|| anyhow!("no ROM: pass --rom <file>"))?;
    let save_dir = save_dir.unwrap_or_else(|| PathBuf::from("."));
    let system_dir = system_dir.unwrap_or_else(|| save_dir.clone());
    let notes_dir = notes_dir.unwrap_or_else(|| save_dir.join("notes"));
    Ok(Args {
        core,
        rom,
        system_dir,
        save_dir,
        notes_dir,
        config,
        runahead,
        shot,
        shot_frame,
        cartridge_label,
        logo,
        shot_off,
        debug_note_capture,
    })
}

const HELP: &str = "emu-run --core <lib> --rom <game.sfc> [--system-dir D] [--save-dir D]\n\
       [--config config.toml] [--runahead N]\n\
       [--shot out.bmp [--shot-frame N]]   headless: run N frames, dump one, exit\n\
       [--cartridge-label img.png]         show a label in the cabinet's slot\n\
       [--logo img.png]                    show a logo atop the side panel\n\
       [--shot-off]                        with --shot, preview the idle off screen\n\
       [--notes-dir DIR]                   per-ROM notebooks (default: <save-dir>/notes)\n\
       [--debug-note-capture]              force one note capture at --shot-frame (dev/testing)\n\
\n\
Presentation is fixed: RF NTSC + CRT-tube warp (knobs are consts in the source).\n\
Battery SRAM and 10 save-state slots live next to --save-dir, keyed by ROM hash.\n\
Player 1 = keyboard or gamepad 1; player 2 = gamepad 2.\n\
Keyboard binds and the run-ahead default come from config.toml (see docs/fase-1).\n\
\n\
default keys: arrows=dpad  Z=B X=A A=Y S=X Q=L W=R  Enter=Start RShift=Select\n\
      F2=save  F4=load  ] / [ =slot  Tab=fast-forward  \\=frame-step (paused)\n\
      F12=screenshot  F=fullscreen  Backspace=reset  P=pause\n\
      Esc=power off (desligar)  E=eject (only once off)\n\
      , / . =cheat cursor  /=toggle cheat (panel, curated games only)\n\
      N=capture into notebook (saved next to --notes-dir, plan §3.4)\n\
      Closing the window always quits, on or off — no ceremony.";

fn main() -> Result<()> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();
    let args = parse_args()?;

    let cfg = Config::load(args.config.as_deref())?;
    if let Some(p) = &cfg.source {
        log::info!("config: {}", p.display());
    }

    let mut platform = xperience_platform::Platform::new().map_err(|e| anyhow!(e.to_string()))?;
    let mut cabinet = platform
        .create_cabinet("SNES Xperience", 1024, 768)
        .map_err(|e| anyhow!(e.to_string()))?;
    if cfg.fullscreen {
        cabinet.toggle_fullscreen();
    }
    let spec = GameSpec {
        core: args.core,
        rom: args.rom,
        system_dir: args.system_dir,
        save_dir: args.save_dir,
        notes_dir: args.notes_dir,
        runahead: args.runahead,
        shot: args.shot.map(|p| (p, args.shot_frame)),
        cartridge_label: args.cartridge_label,
        logo: args.logo,
        shot_off: args.shot_off,
        debug_note_capture: args.debug_note_capture,
    };
    // Standalone: "back" and "close" both just end the process.
    run_game(&mut platform, &mut cabinet, &spec, &cfg)?;
    log::info!("bye");
    Ok(())
}
