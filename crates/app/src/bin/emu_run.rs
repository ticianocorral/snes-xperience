//! The bare emulator: a libretro core loads and runs, with video, sound, a pad,
//! save states, battery SRAM and run-ahead — no bezel, no selector.
//! Presentation is fixed: RF NTSC + CRT-tube warp.
//!
//! Usage:
//!   emu-run --core <path/to/snes9x_libretro.{dylib,so,dll}> --rom <game.sfc>
//!           [--system-dir DIR] [--save-dir DIR] [--runahead N]
//!
//! The core path also reads from $XPERIENCE_CORE. See docs/fase-0.md for where
//! to get the core, docs/fase-1.md for save states / SRAM / run-ahead.

use std::fs;
use std::path::PathBuf;
use std::time::{Duration, Instant};

use anyhow::{anyhow, bail, Context, Result};
use xperience_app::config::Config;
use xperience_emulation::{Button, Core, PixelFormat as EmuFormat};
use xperience_ntsc::{NtscFilter, Preset};
use xperience_platform::{FrameRef, PixelFormat as PlatFormat, Platform, UiEvent, MAX_PORTS};

/// How often to flush battery SRAM to disk while playing (frames ≈ 10 s).
const SRAM_FLUSH_FRAMES: u32 = 600;
/// Save-state slots (keys 0..9 of the ROM hash).
const SLOTS: u8 = 10;
/// Emulated frames per shown frame while fast-forward is held.
const FF_SPEED: u32 = 8;

struct Args {
    core: PathBuf,
    rom: PathBuf,
    system_dir: PathBuf,
    save_dir: PathBuf,
    config: Option<PathBuf>,
    /// Speculative frames past the shown one; `None` = take the config value.
    runahead: Option<u32>,
    /// Headless self-check: run N frames, save the composited window, exit.
    shot: Option<PathBuf>,
    shot_frame: u32,
}

fn parse_args() -> Result<Args> {
    let mut core = std::env::var_os("XPERIENCE_CORE").map(PathBuf::from);
    let mut rom = None;
    let mut system_dir = None;
    let mut save_dir = None;
    let mut config = None;
    let mut runahead = None;
    let mut shot = None;
    let mut shot_frame = 180u32;

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
    Ok(Args {
        core,
        rom,
        system_dir,
        save_dir,
        config,
        runahead,
        shot,
        shot_frame,
    })
}

const HELP: &str = "emu-run --core <lib> --rom <game.sfc> [--system-dir D] [--save-dir D]\n\
       [--config config.toml] [--runahead N]\n\
       [--shot out.bmp [--shot-frame N]]   headless: run N frames, dump one, exit\n\
\n\
Presentation is fixed: RF NTSC + CRT-tube warp (knobs are consts in the source).\n\
Battery SRAM and 10 save-state slots live next to --save-dir, keyed by ROM hash.\n\
Player 1 = keyboard or gamepad 1; player 2 = gamepad 2.\n\
Keyboard binds and the run-ahead default come from config.toml (see docs/fase-1).\n\
\n\
default keys: arrows=dpad  Z=B X=A A=Y S=X Q=L W=R  Enter=Start RShift=Select\n\
      F2=save  F4=load  ] / [ =slot  Tab=fast-forward  \\=frame-step (paused)\n\
      F12=screenshot  F=fullscreen  Backspace=reset  P=pause  Esc=quit";

fn map_format(f: EmuFormat) -> PlatFormat {
    match f {
        EmuFormat::Rgb1555 => PlatFormat::Rgb1555,
        EmuFormat::Xrgb8888 => PlatFormat::Xrgb8888,
        EmuFormat::Rgb565 => PlatFormat::Rgb565,
    }
}

const PAD: [(Button, xperience_platform::PadButton); 12] = {
    use xperience_platform::PadButton as P;
    [
        (Button::B, P::B),
        (Button::Y, P::Y),
        (Button::Select, P::Select),
        (Button::Start, P::Start),
        (Button::Up, P::Up),
        (Button::Down, P::Down),
        (Button::Left, P::Left),
        (Button::Right, P::Right),
        (Button::A, P::A),
        (Button::X, P::X),
        (Button::L, P::L),
        (Button::R, P::R),
    ]
};

fn state_file(hash: &Option<String>, dir: &std::path::Path, slot: u8) -> Option<PathBuf> {
    hash.as_ref().map(|h| dir.join(format!("{h}.state{slot}")))
}

/// Seconds since the epoch, for screenshot filenames.
fn now_stamp() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

fn main() -> Result<()> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();
    let args = parse_args()?;

    let cfg = Config::load(args.config.as_deref())?;
    if let Some(p) = &cfg.source {
        log::info!("config: {}", p.display());
    }
    let runahead_cfg = args.runahead.unwrap_or(cfg.runahead);

    // --- load + identify -------------------------------------------------
    let mut core =
        Core::load(&args.core).with_context(|| format!("loading core {}", args.core.display()))?;
    log::info!("core: {} {}", core.system_name(), core.system_version());
    core.set_directories(&args.system_dir, &args.save_dir);
    core.init();

    let rom_bytes =
        std::fs::read(&args.rom).with_context(|| format!("reading ROM {}", args.rom.display()))?;
    let rom_hash = match xperience_domain::RomId::from_bytes(&rom_bytes) {
        Ok(id) => {
            log::info!(
                "rom: {} bytes (+{} header), crc32={} sha1={} name={:?} {:?}",
                id.rom_len,
                id.header_len,
                id.crc32,
                id.sha1,
                id.internal_name,
                id.mapper
            );
            Some(id.sha1)
        }
        Err(e) => {
            log::warn!("rom id failed (no SRAM/state persistence): {e}");
            None
        }
    };

    core.load_game(&args.rom, &rom_bytes)
        .context("core rejected the ROM")?;

    // Per-game persistence files next to --save-dir, keyed by ROM hash.
    fs::create_dir_all(&args.save_dir).ok();
    let sram_path = rom_hash
        .as_ref()
        .map(|h| args.save_dir.join(format!("{h}.srm")));
    let mut slot: u8 = 0;

    if let Some(p) = &sram_path {
        if let Ok(bytes) = fs::read(p) {
            let n = core.load_sram(&bytes);
            log::info!("SRAM: loaded {n} bytes from {}", p.display());
        }
    }

    // We do our own NTSC (vendored blargg snes_ntsc, RF preset) on the raw
    // frame, so keep the core's built-in filter off.
    core.set_variable("snes9x_blargg", "disabled");
    let mut ntsc = NtscFilter::new(Preset::Rf);

    let av = core.av_info();
    log::info!(
        "av: {}x{} (max {}x{}) aspect={:.3} fps={:.3} sr={:.0}",
        av.base_width,
        av.base_height,
        av.max_width,
        av.max_height,
        av.aspect_ratio,
        av.fps,
        av.sample_rate
    );

    // --- platform ------------------------------------------------------
    let mut platform = Platform::new().map_err(|e| anyhow!(e.to_string()))?;
    let win_h = 672u32;
    let win_w = ((win_h as f32)
        * if av.aspect_ratio > 0.0 {
            av.aspect_ratio
        } else {
            4.0 / 3.0
        })
    .round() as u32;
    let title = args
        .rom
        .file_stem()
        .map(|s| format!("SNES Xperience — {}", s.to_string_lossy()))
        .unwrap_or_else(|| "SNES Xperience".to_string());
    let mut video = platform
        .create_window(&title, win_w, win_h)
        .map_err(|e| anyhow!(e.to_string()))?;
    if cfg.fullscreen {
        video.toggle_fullscreen();
    }
    let audio = platform
        .open_audio(av.sample_rate.round().max(8000.0) as u32)
        .map_err(|e| anyhow!(e.to_string()))?;
    let mut input = platform.new_input();

    let mut paused = false;
    let frame_time = Duration::from_secs_f64(1.0 / av.fps.max(1.0));
    let mut next = Instant::now();
    let mut frames: u32 = 0;
    let mut last_dims = (0u32, 0u32);
    // Don't let the audio queue run more than ~0.15 s ahead (latency creep).
    let audio_cap = (av.sample_rate / 6.0) as usize;

    // Run-ahead: only if the core actually serializes.
    let mut runahead = runahead_cfg;
    if runahead > 0 && core.save_state().is_none() {
        log::warn!("core has no save state — run-ahead disabled");
        runahead = 0;
    }
    let mut spec_state: Vec<u8> = Vec::new();
    let mut last_sram = core.sram();
    let mut shot_request: Option<PathBuf> = None;
    log::info!("running: rf ntsc + crt tube, run-ahead {runahead}, slot {slot}");

    'run: loop {
        let mut step_once = false;
        for ev in platform.poll(&mut input, &cfg.keymap) {
            match ev {
                UiEvent::Quit => break 'run,
                UiEvent::FrameStep => step_once = true,
                // Held state; platform surfaces it via input.fast_forward().
                UiEvent::FastForward => {}
                UiEvent::ToggleFullscreen => video.toggle_fullscreen(),
                UiEvent::Reset => core.reset(),
                UiEvent::TogglePause => {
                    paused = !paused;
                    log::info!("{}", if paused { "paused" } else { "resumed" });
                }
                UiEvent::NextSlot => {
                    slot = (slot + 1) % SLOTS;
                    log::info!("slot {slot}");
                }
                UiEvent::PrevSlot => {
                    slot = (slot + SLOTS - 1) % SLOTS;
                    log::info!("slot {slot}");
                }
                UiEvent::SaveState => {
                    match (
                        state_file(&rom_hash, &args.save_dir, slot),
                        core.save_state(),
                    ) {
                        (Some(p), Some(s)) => match fs::write(&p, &s) {
                            Ok(_) => log::info!("slot {slot}: saved ({} KiB)", s.len() / 1024),
                            Err(e) => log::warn!("slot {slot}: save failed: {e}"),
                        },
                        _ => log::warn!("no state slot (unidentified ROM?)"),
                    }
                }
                UiEvent::LoadState => {
                    match state_file(&rom_hash, &args.save_dir, slot).map(|p| fs::read(&p)) {
                        Some(Ok(s)) if core.load_state(&s) => log::info!("slot {slot}: loaded"),
                        Some(Ok(_)) => log::warn!("slot {slot}: core rejected the state"),
                        Some(Err(_)) => log::warn!("slot {slot}: empty"),
                        None => log::warn!("no state slot (unidentified ROM?)"),
                    }
                }
                UiEvent::Screenshot => {
                    let p = args.save_dir.join(format!("shot-{}.bmp", now_stamp()));
                    // capture happens after present, below; stash the request.
                    shot_request = Some(p);
                }
            }
        }

        let ff = input.fast_forward() && !paused;
        if !paused || step_once {
            for port in 0..MAX_PORTS {
                for (rb, pb) in PAD {
                    core.set_button(port, rb, input.held(port, pb));
                }
            }
            core.run();
            frames += 1;
            if audio.queued_frames() < audio_cap {
                audio.queue(core.audio());
            }

            // Fast-forward: extra emulated frames with no audio, no run-ahead.
            if ff {
                for _ in 1..FF_SPEED {
                    core.run();
                    frames += 1;
                }
            }

            // Speculative frames past the shown one; their audio is discarded.
            let speculated =
                runahead > 0 && !ff && !step_once && core.save_state_into(&mut spec_state);
            if speculated {
                for _ in 0..runahead {
                    core.run();
                }
            }

            if let Some(frame) = core.take_frame() {
                let dims = (frame.width, frame.height);
                if dims != last_dims {
                    log::info!(
                        "core framebuffer: {}x{} ({:?})",
                        dims.0,
                        dims.1,
                        frame.format
                    );
                    last_dims = dims;
                }
                // RF NTSC on RGB565 frames; anything else passes straight through.
                let fref = if frame.format == EmuFormat::Rgb565 {
                    let (out, ow, oh) =
                        ntsc.process(&frame.pixels, frame.width, frame.height, frame.pitch);
                    let bytes = unsafe {
                        std::slice::from_raw_parts(out.as_ptr() as *const u8, out.len() * 2)
                    };
                    FrameRef {
                        width: ow,
                        height: oh,
                        pitch: ow as usize * 2,
                        format: PlatFormat::Rgb565,
                        pixels: bytes,
                    }
                } else {
                    FrameRef {
                        width: frame.width,
                        height: frame.height,
                        pitch: frame.pitch,
                        format: map_format(frame.format),
                        pixels: &frame.pixels,
                    }
                };
                let aspect = core.av_info().aspect_ratio;
                video.present(&fref, aspect);

                if let Some(path) = shot_request.take() {
                    match video.capture_bmp(&fref, aspect, &path) {
                        Ok(_) => log::info!("screenshot -> {}", path.display()),
                        Err(e) => log::warn!("screenshot failed: {e}"),
                    }
                }

                if let Some(path) = &args.shot {
                    if frames >= args.shot_frame {
                        video
                            .capture_bmp(&fref, aspect, path)
                            .map_err(|e| anyhow!(e.to_string()))?;
                        log::info!("wrote {} after {} frames", path.display(), frames);
                        break 'run;
                    }
                }
            }

            // Rewind past the speculative frames to the real state.
            if speculated {
                core.load_state(&spec_state);
            }

            // Periodically flush battery SRAM if it changed.
            if frames.is_multiple_of(SRAM_FLUSH_FRAMES) {
                if let (Some(p), Some(cur)) = (&sram_path, core.sram()) {
                    if last_sram.as_ref() != Some(&cur) {
                        let _ = fs::write(p, &cur);
                        last_sram = Some(cur);
                    }
                }
            }
        }

        if ff {
            // Run flat out; don't accumulate a pacing debt.
            next = Instant::now();
        } else {
            next += frame_time;
            let now = Instant::now();
            if next > now {
                std::thread::sleep(next - now);
            } else {
                // Fell behind; resync so we don't spiral.
                next = now;
            }
        }
    }

    // Final SRAM flush on the way out.
    if let (Some(p), Some(cur)) = (&sram_path, core.sram()) {
        if last_sram.as_ref() != Some(&cur) {
            match fs::write(p, &cur) {
                Ok(_) => log::info!("SRAM flushed -> {}", p.display()),
                Err(e) => log::warn!("SRAM flush failed: {e}"),
            }
        }
    }

    log::info!("bye");
    Ok(())
}
