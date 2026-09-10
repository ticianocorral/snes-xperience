//! Phase 0 proof #1: a libretro core loads and runs, with video, sound and a
//! pad — no bezel, no selector. Presentation is fixed: RF NTSC + CRT-tube warp.
//!
//! Usage:
//!   emu-run --core <path/to/snes9x_libretro.{dylib,so,dll}> --rom <game.sfc>
//!           [--system-dir DIR] [--save-dir DIR]
//!
//! The core path also reads from $XPERIENCE_CORE. See docs/fase-0.md for where
//! to get the core.

use std::path::PathBuf;
use std::time::{Duration, Instant};

use anyhow::{anyhow, bail, Context, Result};
use xperience_emulation::{Button, Core, PixelFormat as EmuFormat};
use xperience_ntsc::{NtscFilter, Preset};
use xperience_platform::{FrameRef, PixelFormat as PlatFormat, Platform, UiEvent};

struct Args {
    core: PathBuf,
    rom: PathBuf,
    system_dir: PathBuf,
    save_dir: PathBuf,
    /// Headless self-check: run N frames, save the composited window, exit.
    shot: Option<PathBuf>,
    shot_frame: u32,
}

fn parse_args() -> Result<Args> {
    let mut core = std::env::var_os("XPERIENCE_CORE").map(PathBuf::from);
    let mut rom = None;
    let mut system_dir = None;
    let mut save_dir = None;
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
        shot,
        shot_frame,
    })
}

const HELP: &str = "emu-run --core <lib> --rom <game.sfc> [--system-dir D] [--save-dir D]\n\
       [--shot out.bmp [--shot-frame N]]   headless: run N frames, dump one, exit\n\
\n\
Presentation is fixed: RF NTSC + CRT-tube warp (knobs are consts in the source).\n\
\n\
keys: arrows=dpad  Z=B X=A A=Y S=X Q=L W=R  Enter=Start RShift=Select\n\
      F=fullscreen  Backspace=reset  P=pause  Esc=quit";

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

fn main() -> Result<()> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();
    let args = parse_args()?;

    // --- load + identify -------------------------------------------------
    let mut core =
        Core::load(&args.core).with_context(|| format!("loading core {}", args.core.display()))?;
    log::info!("core: {} {}", core.system_name(), core.system_version());
    core.set_directories(&args.system_dir, &args.save_dir);
    core.init();

    let rom_bytes =
        std::fs::read(&args.rom).with_context(|| format!("reading ROM {}", args.rom.display()))?;
    match xperience_domain::RomId::from_bytes(&rom_bytes) {
        Ok(id) => log::info!(
            "rom: {} bytes (+{} header), crc32={} sha1={} name={:?} {:?}",
            id.rom_len,
            id.header_len,
            id.crc32,
            id.sha1,
            id.internal_name,
            id.mapper
        ),
        Err(e) => log::warn!("rom id failed (continuing): {e}"),
    }

    core.load_game(&args.rom, &rom_bytes)
        .context("core rejected the ROM")?;

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
    let mut video = platform
        .create_window("SNES Xperience — Phase 0", win_w, win_h)
        .map_err(|e| anyhow!(e.to_string()))?;
    let audio = platform
        .open_audio(av.sample_rate.round().max(8000.0) as u32)
        .map_err(|e| anyhow!(e.to_string()))?;
    let mut input = platform.new_input();

    let mut paused = false;
    let frame_time = Duration::from_secs_f64(1.0 / av.fps.max(1.0));
    let mut next = Instant::now();
    let mut frames: u32 = 0;
    let mut last_dims = (0u32, 0u32);
    log::info!("running: rf ntsc + crt tube");

    'run: loop {
        for ev in platform.poll(&mut input) {
            match ev {
                UiEvent::Quit => break 'run,
                UiEvent::ToggleFullscreen => video.toggle_fullscreen(),
                UiEvent::Reset => core.reset(),
                UiEvent::TogglePause => {
                    paused = !paused;
                    log::info!("{}", if paused { "paused" } else { "resumed" });
                }
            }
        }

        if !paused {
            for (rb, pb) in PAD {
                core.set_button(0, rb, input.held(pb));
            }
            core.run();
            frames += 1;
            audio.queue(core.audio());
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
        }

        next += frame_time;
        let now = Instant::now();
        if next > now {
            std::thread::sleep(next - now);
        } else {
            // Fell behind; resync so we don't spiral.
            next = now;
        }
    }

    log::info!("bye");
    Ok(())
}
