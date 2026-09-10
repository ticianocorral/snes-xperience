//! The emulator run-loop, factored out of the `emu-run` binary so the unified
//! `xperience` binary can call it between selector visits. Presentation is fixed:
//! RF NTSC + CRT-tube warp (see docs/fase-0.md).

use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use anyhow::{anyhow, Context, Result};
use xperience_emulation::{Button, Core, PixelFormat as EmuFormat};
use xperience_ntsc::{NtscFilter, Preset};
use xperience_platform::{
    Cabinet, FrameRef, PixelFormat as PlatFormat, Platform, UiEvent, MAX_PORTS,
};

use crate::config::Config;

/// How often to flush battery SRAM to disk while playing (frames ≈ 10 s).
const SRAM_FLUSH_FRAMES: u32 = 600;
/// Save-state slots (keys 0..9 of the ROM hash).
const SLOTS: u8 = 10;
/// Emulated frames per shown frame while fast-forward is held.
const FF_SPEED: u32 = 8;

/// Why the run-loop returned.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GameExit {
    /// Esc / "back" — the caller should show the selector again, if it has one.
    ToShelf,
    /// Window close / Cmd-Q / headless self-check done — tear the app down.
    Quit,
}

/// Everything [`run_game`] needs for one session.
pub struct GameSpec {
    pub core: PathBuf,
    pub rom: PathBuf,
    pub system_dir: PathBuf,
    pub save_dir: PathBuf,
    /// Speculative frames past the shown one; `None` = take the config value.
    pub runahead: Option<u32>,
    /// Headless self-check: `(path, frame)` — run to `frame`, dump a BMP, exit.
    pub shot: Option<(PathBuf, u32)>,
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

fn map_format(f: EmuFormat) -> PlatFormat {
    match f {
        EmuFormat::Rgb1555 => PlatFormat::Rgb1555,
        EmuFormat::Xrgb8888 => PlatFormat::Xrgb8888,
        EmuFormat::Rgb565 => PlatFormat::Rgb565,
    }
}

fn state_file(hash: &Option<String>, dir: &Path, slot: u8) -> Option<PathBuf> {
    hash.as_ref().map(|h| dir.join(format!("{h}.state{slot}")))
}

/// Seconds since the epoch, for screenshot filenames.
fn now_stamp() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// Load the core + ROM and run until the player leaves, drawing into `cab` (the
/// one persistent window). `plat` and `cab` both outlive the call so `xperience`
/// can reuse them for the next screen.
pub fn run_game(
    plat: &mut Platform,
    cab: &mut Cabinet,
    spec: &GameSpec,
    cfg: &Config,
) -> Result<GameExit> {
    let runahead_cfg = spec.runahead.unwrap_or(cfg.runahead);

    // --- load + identify -------------------------------------------------
    let mut core =
        Core::load(&spec.core).with_context(|| format!("loading core {}", spec.core.display()))?;
    log::info!("core: {} {}", core.system_name(), core.system_version());
    core.set_directories(&spec.system_dir, &spec.save_dir);
    core.init();

    let rom_bytes =
        fs::read(&spec.rom).with_context(|| format!("reading ROM {}", spec.rom.display()))?;
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

    core.load_game(&spec.rom, &rom_bytes)
        .context("core rejected the ROM")?;

    // Per-game persistence files next to save_dir, keyed by ROM hash.
    fs::create_dir_all(&spec.save_dir).ok();
    let sram_path = rom_hash
        .as_ref()
        .map(|h| spec.save_dir.join(format!("{h}.srm")));
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

    // --- audio -----------------------------------------------------------
    let audio = plat
        .open_audio(av.sample_rate.round().max(8000.0) as u32)
        .map_err(|e| anyhow!(e.to_string()))?;
    let mut input = plat.new_input();

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

    let exit = 'run: loop {
        let mut step_once = false;
        for ev in plat.poll(&mut input, &cfg.keymap) {
            match ev {
                UiEvent::Quit => break 'run GameExit::ToShelf,
                UiEvent::CloseRequested => break 'run GameExit::Quit,
                UiEvent::FrameStep => step_once = true,
                // Held state; platform surfaces it via input.fast_forward().
                UiEvent::FastForward => {}
                UiEvent::ToggleFullscreen => cab.toggle_fullscreen(),
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
                        state_file(&rom_hash, &spec.save_dir, slot),
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
                    match state_file(&rom_hash, &spec.save_dir, slot).map(|p| fs::read(&p)) {
                        Some(Ok(s)) if core.load_state(&s) => log::info!("slot {slot}: loaded"),
                        Some(Ok(_)) => log::warn!("slot {slot}: core rejected the state"),
                        Some(Err(_)) => log::warn!("slot {slot}: empty"),
                        None => log::warn!("no state slot (unidentified ROM?)"),
                    }
                }
                UiEvent::Screenshot => {
                    let p = spec.save_dir.join(format!("shot-{}.bmp", now_stamp()));
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
                cab.present_frame(&fref, aspect);

                if let Some(path) = shot_request.take() {
                    match cab.capture_bmp(&fref, aspect, &path) {
                        Ok(_) => log::info!("screenshot -> {}", path.display()),
                        Err(e) => log::warn!("screenshot failed: {e}"),
                    }
                }

                if let Some((path, at)) = &spec.shot {
                    if frames >= *at {
                        cab.capture_bmp(&fref, aspect, path)
                            .map_err(|e| anyhow!(e.to_string()))?;
                        log::info!("wrote {} after {} frames", path.display(), frames);
                        break 'run GameExit::Quit;
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
    };

    // Final SRAM flush on the way out (either exit path).
    if let (Some(p), Some(cur)) = (&sram_path, core.sram()) {
        if last_sram.as_ref() != Some(&cur) {
            match fs::write(p, &cur) {
                Ok(_) => log::info!("SRAM flushed -> {}", p.display()),
                Err(e) => log::warn!("SRAM flush failed: {e}"),
            }
        }
    }

    log::info!("game loop done: {exit:?}");
    Ok(exit)
}
