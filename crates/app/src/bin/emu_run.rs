//! Phase 0 proof #1: a libretro core loads and runs, with video, sound, a pad
//! and the three scaling modes — no bezel, no selector.
//!
//! Usage:
//!   emu-run --core <path/to/snes9x_libretro.{dylib,so,dll}> --rom <game.sfc>
//!           [--system-dir DIR] [--save-dir DIR] [--scale pixel|sharp|crt]
//!
//! The core path also reads from $XPERIENCE_CORE. See docs/fase-0.md for where
//! to get the core.

use std::path::PathBuf;
use std::time::{Duration, Instant};

use anyhow::{anyhow, bail, Context, Result};
use xperience_emulation::{Button, Core, PixelFormat as EmuFormat};
use xperience_platform::{FrameRef, PixelFormat as PlatFormat, Platform, ScaleMode, UiEvent};

struct Args {
    core: PathBuf,
    rom: PathBuf,
    system_dir: PathBuf,
    save_dir: PathBuf,
    scale: ScaleMode,
    /// Headless self-check: run N frames, dump the last one as a PPM, exit.
    shot: Option<PathBuf>,
    shot_frame: u32,
}

fn parse_args() -> Result<Args> {
    let mut core = std::env::var_os("XPERIENCE_CORE").map(PathBuf::from);
    let mut rom = None;
    let mut system_dir = None;
    let mut save_dir = None;
    let mut scale = ScaleMode::SharpBilinear;
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
            "--scale" => {
                scale = match it.next().as_deref() {
                    Some("pixel") => ScaleMode::PixelPerfect,
                    Some("sharp") => ScaleMode::SharpBilinear,
                    Some("crt") => ScaleMode::Crt,
                    other => bail!("--scale wants pixel|sharp|crt, got {other:?}"),
                }
            }
            "--shot" => {
                shot = Some(
                    it.next()
                        .ok_or_else(|| anyhow!("--shot needs a path"))?
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
        scale,
        shot,
        shot_frame,
    })
}

const HELP: &str = "emu-run --core <lib> --rom <game.sfc> [--system-dir D] [--save-dir D] [--scale pixel|sharp|crt]\n\
       [--shot out.ppm [--shot-frame N]]   headless: run N frames, dump one, exit\n\
\n\
keys: arrows=dpad  Z=B X=A A=Y S=X Q=L W=R  Enter=Start RShift=Select\n\
      Tab=cycle scale  F=fullscreen  Backspace=reset  P=pause  Esc=quit";

/// Expand a core frame to 8-bit RGB and write it as a binary PPM (P6). Enough
/// to eyeball that the video pipeline produces real pixels; not a real encoder.
fn write_ppm(path: &std::path::Path, frame: &xperience_emulation::Frame) -> Result<()> {
    let (w, h) = (frame.width as usize, frame.height as usize);
    let bpp = frame.format.bytes_per_pixel();
    let mut out = Vec::with_capacity(w * h * 3 + 32);
    out.extend_from_slice(format!("P6\n{w} {h}\n255\n").as_bytes());
    for y in 0..h {
        let row = &frame.pixels[y * frame.pitch..y * frame.pitch + w * bpp];
        for x in 0..w {
            let px = &row[x * bpp..x * bpp + bpp];
            let (r, g, b) = match frame.format {
                EmuFormat::Rgb565 => {
                    let v = u16::from_le_bytes([px[0], px[1]]);
                    let r5 = (v >> 11) & 0x1f;
                    let g6 = (v >> 5) & 0x3f;
                    let b5 = v & 0x1f;
                    (
                        ((r5 << 3) | (r5 >> 2)) as u8,
                        ((g6 << 2) | (g6 >> 4)) as u8,
                        ((b5 << 3) | (b5 >> 2)) as u8,
                    )
                }
                EmuFormat::Rgb1555 => {
                    let v = u16::from_le_bytes([px[0], px[1]]);
                    let r5 = (v >> 10) & 0x1f;
                    let g5 = (v >> 5) & 0x1f;
                    let b5 = v & 0x1f;
                    (
                        ((r5 << 3) | (r5 >> 2)) as u8,
                        ((g5 << 3) | (g5 >> 2)) as u8,
                        ((b5 << 3) | (b5 >> 2)) as u8,
                    )
                }
                EmuFormat::Xrgb8888 => (px[2], px[1], px[0]),
            };
            out.push(r);
            out.push(g);
            out.push(b);
        }
    }
    std::fs::write(path, out).with_context(|| format!("writing {}", path.display()))?;
    Ok(())
}

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

    let mut scale = args.scale;
    let mut paused = false;
    let frame_time = Duration::from_secs_f64(1.0 / av.fps.max(1.0));
    let mut next = Instant::now();
    let mut frames: u32 = 0;
    log::info!("running. scale = {}", scale.label());

    'run: loop {
        for ev in platform.poll(&mut input) {
            match ev {
                UiEvent::Quit => break 'run,
                UiEvent::ToggleFullscreen => video.toggle_fullscreen(),
                UiEvent::CycleScaleMode => {
                    scale = scale.next();
                    log::info!("scale = {}", scale.label());
                }
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
                let pf = map_format(frame.format);
                video.present(
                    &FrameRef {
                        width: frame.width,
                        height: frame.height,
                        pitch: frame.pitch,
                        format: pf,
                        pixels: &frame.pixels,
                    },
                    core.av_info().aspect_ratio,
                    scale,
                );

                if let Some(path) = &args.shot {
                    if frames >= args.shot_frame {
                        write_ppm(path, &frame)?;
                        log::info!(
                            "wrote {} ({}x{}, {:?}) after {} frames",
                            path.display(),
                            frame.width,
                            frame.height,
                            frame.format,
                            frames
                        );
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
