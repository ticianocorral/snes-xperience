//! The emulator run-loop, factored out of the `emu-run` binary so the unified
//! `xperience` binary can call it between selector visits. Presentation is fixed:
//! RF NTSC + CRT-tube warp (see docs/fase-0.md).

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use anyhow::{anyhow, Context, Result};
use xperience_emulation::{Button, Core, Frame as EmuFrame, PixelFormat as EmuFormat};
use xperience_ntsc::{NtscFilter, Preset};
use xperience_platform::{
    Cabinet, FrameRef, KeyMap, PixelFormat as PlatFormat, Platform, UiEvent, MAX_PORTS,
};

use crate::config::Config;

/// How often to flush battery SRAM to disk while playing (frames ≈ 10 s).
const SRAM_FLUSH_FRAMES: u32 = 600;
/// Save-state slots (keys 0..9 of the ROM hash).
const SLOTS: u8 = 10;
/// Emulated frames per shown frame while fast-forward is held.
const FF_SPEED: u32 = 8;
/// The dim, near-still hiss the screen idles at once the console is off
/// (plan §3.3) — also what the next screen fades in from.
const OFF_STATIC_LEVEL: f32 = 0.12;

/// Why the run-loop returned.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum GameExit {
    /// Ejected — the caller should show the selector again, if it has one.
    /// Carries the signal-off static level the screen settled on, so the next
    /// screen can fade in over it instead of a fresh burst (plan §3.3, "a
    /// estante entra por cima").
    ToShelf { static_level: f32 },
    /// Window close / Cmd-Q / headless self-check done — tear the app down.
    Quit,
}

/// Everything [`run_game`] needs for one session.
pub struct GameSpec {
    pub core: PathBuf,
    pub rom: PathBuf,
    pub system_dir: PathBuf,
    pub save_dir: PathBuf,
    /// Where per-ROM notebooks live: `<hash>.md` plus a `<hash>/` folder of
    /// captured screenshots alongside it (plan §3.4).
    pub notes_dir: PathBuf,
    /// Speculative frames past the shown one; `None` = take the config value.
    pub runahead: Option<u32>,
    /// Headless self-check: `(path, frame)` — run to `frame`, dump a BMP, exit.
    pub shot: Option<(PathBuf, u32)>,
    /// Cartridge label art (ScreenScraper `texture`) for the slot on the
    /// cabinet. `None` shows the ROM's name instead (plan §3.2/§4.3).
    pub cartridge_label: Option<PathBuf>,
    /// Logo art (ScreenScraper `wheel`) for the top of the side panel.
    /// `None` shows the ROM's name instead (plan §3.2, item 1).
    pub logo: Option<PathBuf>,
    /// Headless self-check: skip straight to the idle "console off" screen
    /// (signal-off snow + cartridge still in the slot) and save `shot` there,
    /// instead of running the game to `shot`'s frame count.
    pub shot_off: bool,
    /// Headless self-check: force one `NoteCapture` at `shot`'s frame (or
    /// frame 1 without one), exactly like a live `N` press — so `--shot` can
    /// prove out the panel's notebook block without a window.
    pub debug_note_capture: bool,
    /// Headless self-check: skip straight to the pause book (plan §3.2/§3.4)
    /// instead of gameplay, so `--shot` can prove it out against whatever
    /// notes already exist on disk for this ROM.
    pub debug_shot_pause: bool,
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

/// The current key bound to a bindable [`UiEvent`] token, for the panel's
/// command legend. `"?"` if somehow unbound (shouldn't happen — every
/// bindable event has a default).
fn key_for(keymap: &KeyMap, token: &str) -> String {
    keymap
        .describe()
        .into_iter()
        .find(|(t, _)| t == token)
        .map(|(_, k)| k)
        .unwrap_or_else(|| "?".to_string())
}

/// Write battery SRAM to `path` if it changed since the last flush.
fn flush_sram(path: &Option<PathBuf>, last: &mut Option<Vec<u8>>, cur: Option<Vec<u8>>) {
    if let (Some(p), Some(cur)) = (path, cur) {
        if last.as_ref() != Some(&cur) {
            match fs::write(p, &cur) {
                Ok(_) => log::info!("SRAM flushed -> {}", p.display()),
                Err(e) => log::warn!("SRAM flush failed: {e}"),
            }
            *last = Some(cur);
        }
    }
}

/// Desligar (plan §3.3): a short burst of RF snow through the tube with a
/// decaying buzz, settling to a dim near-still hiss — never a full-screen
/// flash, and the noise cuts rather than lingers.
fn power_off_burst(plat: &Platform, cab: &mut Cabinet) -> f32 {
    const RATE: u32 = 22_050;
    const SPAN: Duration = Duration::from_millis(650);
    let audio = plat.open_audio(RATE).ok();
    let frame = Duration::from_millis(16);
    let mut rng: u32 = 0x1234_5678;
    let start = Instant::now();

    while start.elapsed() < SPAN {
        let t = (start.elapsed().as_secs_f32() / SPAN.as_secs_f32()).min(1.0);
        // Strong for the first half, then settle toward the idle-off hiss.
        let level = if t < 0.5 {
            1.0 - 0.5 * t
        } else {
            (0.9 - t).max(OFF_STATIC_LEVEL)
        };
        cab.present_static(level);

        if let Some(a) = &audio {
            let n = (RATE / 60) as usize;
            let amp = ((1.0 - t) * 8000.0) as i32;
            let mut buf = Vec::with_capacity(n * 2);
            for _ in 0..n {
                rng ^= rng << 13;
                rng ^= rng >> 17;
                rng ^= rng << 5;
                let s = (((rng >> 8) & 0xFFFF) as i32 - 0x8000) * amp / 0x8000;
                let v = s.clamp(-32000, 32000) as i16;
                buf.push(v);
                buf.push(v);
            }
            a.queue(&buf);
        }
        std::thread::sleep(frame);
    }
    if let Some(a) = &audio {
        a.clear(); // buzz cut, not fade-out tail
    }
    OFF_STATIC_LEVEL
}

/// Ejetar with the console still on: the lock resists — a short mechanical
/// thump, nothing else (plan §3.3, "a alavanca resiste, com um clunk seco").
fn eject_clunk(plat: &Platform) {
    const RATE: u32 = 22_050;
    let Some(audio) = plat.open_audio(RATE).ok() else {
        return;
    };
    let n = (RATE as f32 * 0.09) as usize;
    let mut buf = Vec::with_capacity(n * 2);
    let mut phase = 0f32;
    for i in 0..n {
        let env = 1.0 - i as f32 / n as f32;
        phase += 90.0 / RATE as f32;
        let s = (phase * std::f32::consts::TAU).sin() * env * env;
        let v = (s * 12000.0) as i16;
        buf.push(v);
        buf.push(v);
    }
    audio.queue(&buf);
    std::thread::sleep(Duration::from_millis(100));
}

/// Where a ROM's cheat toggle state lives, keyed by hash like save states.
fn cheat_state_path(hash: &Option<String>, dir: &Path) -> Option<PathBuf> {
    hash.as_ref().map(|h| dir.join(format!("{h}.cheats")))
}

/// One `0`/`1` per line, in the curated list's order. Missing/short/garbled
/// files just mean "start with everything off" — nothing to migrate.
fn load_cheat_state(path: &Option<PathBuf>, len: usize) -> Vec<bool> {
    let mut state = vec![false; len];
    if let Some(text) = path.as_ref().and_then(|p| fs::read_to_string(p).ok()) {
        for (slot, line) in state.iter_mut().zip(text.lines()) {
            *slot = line.trim() == "1";
        }
    }
    state
}

fn save_cheat_state(path: &Option<PathBuf>, state: &[bool]) {
    if let Some(p) = path {
        let text: String = state
            .iter()
            .map(|&on| if on { "1\n" } else { "0\n" })
            .collect();
        if let Err(e) = fs::write(p, text) {
            log::warn!("cheat state flush failed: {e}");
        }
    }
}

/// `(description, on)` pairs for the panel — cheap enough to rebuild on every
/// toggle/navigate, there are only ever a handful.
fn cheat_rows(defs: &[xperience_domain::CheatDef], state: &[bool]) -> Vec<(String, bool)> {
    defs.iter()
        .zip(state)
        .map(|(d, &on)| (d.desc.to_string(), on))
        .collect()
}

/// Decode scraped art (cartridge label, panel logo, …) small enough for its
/// slot, keeping alpha for transparent logos.
fn decode_art(path: &Path, max: u32) -> Result<(u32, u32, Vec<u8>)> {
    let img = image::open(path)?.thumbnail(max, max).to_rgba8();
    let (w, h) = img.dimensions();
    Ok((w, h, img.into_raw()))
}

/// Seconds since the epoch, for screenshot filenames.
fn now_stamp() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// Decode a core frame's raw pixels into plain RGB8 — the un-warped, un-NTSC'd
/// picture, not what's on screen, so a captured password stays legible on the
/// page (plan §3.4: "é como se fazia no papel").
fn frame_to_rgb8(frame: &EmuFrame) -> image::RgbImage {
    let (w, h) = (frame.width as usize, frame.height as usize);
    let mut img = image::RgbImage::new(frame.width, frame.height);
    for y in 0..h {
        let row = &frame.pixels[y * frame.pitch..];
        for x in 0..w {
            let rgb = match frame.format {
                EmuFormat::Rgb565 => {
                    let px = u16::from_le_bytes([row[x * 2], row[x * 2 + 1]]);
                    let (r5, g6, b5) = ((px >> 11) & 0x1F, (px >> 5) & 0x3F, px & 0x1F);
                    [
                        ((r5 << 3) | (r5 >> 2)) as u8,
                        ((g6 << 2) | (g6 >> 4)) as u8,
                        ((b5 << 3) | (b5 >> 2)) as u8,
                    ]
                }
                EmuFormat::Rgb1555 => {
                    let px = u16::from_le_bytes([row[x * 2], row[x * 2 + 1]]);
                    let (r5, g5, b5) = ((px >> 10) & 0x1F, (px >> 5) & 0x1F, px & 0x1F);
                    [
                        ((r5 << 3) | (r5 >> 2)) as u8,
                        ((g5 << 3) | (g5 >> 2)) as u8,
                        ((b5 << 3) | (b5 >> 2)) as u8,
                    ]
                }
                // XRGB8888, little-endian bytes: B, G, R, X.
                EmuFormat::Xrgb8888 => {
                    let o = x * 4;
                    [row[o + 2], row[o + 1], row[o]]
                }
            };
            img.put_pixel(x as u32, y as u32, image::Rgb(rgb));
        }
    }
    img
}

/// Screenshot straight into this ROM's notebook (plan §3.4): saves the frame
/// next to a per-hash markdown file and appends an image reference — the
/// file is plain text, readable outside the app, captures in a folder beside
/// it. Indexed by hash so a rename or a re-dump doesn't orphan the notes.
fn append_note_image(notes_dir: &Path, hash: &str, frame: &EmuFrame) -> Result<()> {
    let img_dir = notes_dir.join(hash);
    fs::create_dir_all(&img_dir).with_context(|| format!("creating {}", img_dir.display()))?;
    // now_stamp() is second-resolution — bump past any collision from two
    // captures inside the same second rather than silently overwriting one.
    let mut stamp = now_stamp();
    while img_dir.join(format!("{stamp}.png")).exists() {
        stamp += 1;
    }
    let name = format!("{stamp}.png");
    frame_to_rgb8(frame)
        .save(img_dir.join(&name))
        .with_context(|| format!("saving note image {name}"))?;

    let md_path = notes_dir.join(format!("{hash}.md"));
    let is_new = !md_path.exists();
    let mut f = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&md_path)
        .with_context(|| format!("opening {}", md_path.display()))?;
    if is_new {
        writeln!(f, "# Anotacoes\n")?;
    }
    writeln!(f, "![captura]({hash}/{name})\n")?;
    Ok(())
}

/// Recompute the panel's notebook block from what's actually on disk: how
/// many pages this ROM has, and the most recent one as a thumbnail. Call at
/// game start and after every capture — cheap, there's only ever a handful.
fn refresh_notes(cab: &mut Cabinet, notes_dir: &Path, hash: &Option<String>) {
    let Some(hash) = hash else {
        cab.set_notes(0, None);
        return;
    };
    let count = count_note_images(notes_dir, hash);
    match latest_note_image_path(notes_dir, hash).map(|p| decode_art(&p, 200)) {
        Some(Ok((w, h, rgba))) => cab.set_notes(count, Some((w, h, &rgba))),
        _ => cab.set_notes(count, None),
    }
}

/// How many captures a ROM's notebook holds — a count of image files, not a
/// markdown parse, so it stays right even before free-text writing exists.
fn count_note_images(notes_dir: &Path, hash: &str) -> usize {
    fs::read_dir(notes_dir.join(hash))
        .map(|rd| rd.filter_map(|e| e.ok()).count())
        .unwrap_or(0)
}

/// The most recently captured page, by filename — capture names are
/// timestamps, so the last one sorted is the newest.
fn latest_note_image_path(notes_dir: &Path, hash: &str) -> Option<PathBuf> {
    let dir = notes_dir.join(hash);
    let mut names: Vec<std::ffi::OsString> = fs::read_dir(&dir)
        .map(|rd| rd.filter_map(|e| e.ok()).map(|e| e.file_name()).collect())
        .unwrap_or_default();
    names.sort();
    names.last().map(|n| dir.join(n))
}

/// Everything the pause book needs to show (plan §3.2/§3.4, read-only for
/// now): how many pages this ROM's notebook has, and the most recent one
/// decoded big enough for the right-hand page.
fn load_pause_note(
    notes_dir: &Path,
    hash: &Option<String>,
) -> (usize, Option<(u32, u32, Vec<u8>)>) {
    let Some(hash) = hash else {
        return (0, None);
    };
    let captures = count_note_images(notes_dir, hash);
    let thumb = latest_note_image_path(notes_dir, hash).and_then(|p| decode_art(&p, 900).ok());
    (captures, thumb)
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
    let (rom_hash, internal_name) = match xperience_domain::RomId::from_bytes(&rom_bytes) {
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
            (Some(id.sha1), id.internal_name)
        }
        Err(e) => {
            log::warn!("rom id failed (no SRAM/state persistence): {e}");
            (None, None)
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

    // --- cartridge in the slot (plan §3.2/§4.3) --------------------------
    let title = spec
        .rom
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| "???".to_string());
    if let Some(label_path) = &spec.cartridge_label {
        match decode_art(label_path, 300) {
            Ok((w, h, rgba)) => cab.set_cartridge(Some((w, h, &rgba)), &title),
            Err(e) => {
                log::warn!("cartridge label {}: {e}", label_path.display());
                cab.set_cartridge(None, &title);
            }
        }
    } else {
        cab.set_cartridge(None, &title);
    }

    // --- side panel: logo, command legend, session timer (plan §3.2) ------
    // "Desligar" is Esc unconditionally — it's the one binding `KeyMap` keeps
    // fixed (see `UiEvent::token`), so it never shows up in `describe()`.
    let commands = vec![
        ("Desligar".to_string(), "Esc".to_string()),
        ("Ejetar".to_string(), key_for(&cfg.keymap, "eject")),
        ("Reset".to_string(), key_for(&cfg.keymap, "reset")),
    ];
    if let Some(logo_path) = &spec.logo {
        match decode_art(logo_path, 640) {
            Ok((w, h, rgba)) => cab.set_panel(Some((w, h, &rgba)), &title, &commands),
            Err(e) => {
                log::warn!("logo {}: {e}", logo_path.display());
                cab.set_panel(None, &title, &commands);
            }
        }
    } else {
        cab.set_panel(None, &title, &commands);
    }

    // --- cheats: a curated slice of libretro-database codes (plan §4.4) ---
    // Matched by the cartridge header title, not the file — see
    // `xperience_domain::cheats`. Empty for anything we haven't picked yet.
    let cheat_defs = xperience_domain::cheats_for_title(internal_name.as_deref().unwrap_or(""));
    let cheat_path = cheat_state_path(&rom_hash, &spec.save_dir);
    let mut cheat_state = load_cheat_state(&cheat_path, cheat_defs.len());
    let mut cheat_sel: usize = 0;
    if !cheat_defs.is_empty() {
        core.cheat_reset();
        for (i, (def, &on)) in cheat_defs.iter().zip(&cheat_state).enumerate() {
            core.cheat_set(i as u32, on, def.code);
        }
    }
    cab.set_cheats(&cheat_rows(cheat_defs, &cheat_state), cheat_sel);

    // --- notes: the notebook block, empty until the first capture (§3.4) --
    fs::create_dir_all(&spec.notes_dir).ok();
    refresh_notes(cab, &spec.notes_dir, &rom_hash);

    let session_start = Instant::now();

    // Headless self-check: skip straight to the idle "console off" screen.
    if spec.shot_off {
        if let Some((path, _)) = &spec.shot {
            cab.capture_static_bmp(OFF_STATIC_LEVEL, path)
                .map_err(|e| anyhow!(e.to_string()))?;
            log::info!("wrote {} (idle-off preview)", path.display());
        }
        return Ok(GameExit::Quit);
    }

    // Headless self-check: skip straight to the pause book, against whatever
    // notes already exist on disk for this ROM (build them with a separate
    // --debug-note-capture run first).
    if spec.debug_shot_pause {
        if let Some((path, _)) = &spec.shot {
            let (captures, thumb) = load_pause_note(&spec.notes_dir, &rom_hash);
            cab.set_pause_note(
                &title,
                captures,
                thumb.as_ref().map(|(w, h, d)| (*w, *h, d.as_slice())),
            );
            cab.capture_pause_bmp(path)
                .map_err(|e| anyhow!(e.to_string()))?;
            log::info!("wrote {} (pause book preview)", path.display());
        }
        return Ok(GameExit::Quit);
    }

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
    let mut note_request = false;
    // Debug capture lines up with --shot-frame (default: the very first
    // frame) so the saved page actually shows whatever --shot is inspecting,
    // not just a black boot frame.
    let debug_note_frame = spec.shot.as_ref().map_or(1, |(_, f)| *f).max(1);
    // The console: on while playing; off after Esc, idling on snow with the
    // cartridge still seated until Eject (plan §3.3).
    let mut powered = true;
    let mut static_level = OFF_STATIC_LEVEL;
    log::info!("running: rf ntsc + crt tube, run-ahead {runahead}, slot {slot}");

    let exit = 'run: loop {
        let mut step_once = false;
        for ev in plat.poll(&mut input, &cfg.keymap) {
            match ev {
                UiEvent::Quit => {
                    if powered {
                        // Desligar (plan §3.3): flush the cart, then the
                        // signal-off ritual — cartridge stays seated; only
                        // Eject moves on from here. A second Esc while
                        // already off does nothing on purpose.
                        flush_sram(&sram_path, &mut last_sram, core.sram());
                        cab.set_session_time(session_start.elapsed());
                        static_level = power_off_burst(plat, cab);
                        powered = false;
                        log::info!("power off — eject to leave");
                    }
                }
                UiEvent::Eject => {
                    if powered {
                        eject_clunk(plat); // lock resists while it's still on
                    } else {
                        cab.clear_cartridge();
                        break 'run GameExit::ToShelf { static_level };
                    }
                }
                UiEvent::CloseRequested => break 'run GameExit::Quit,
                UiEvent::FrameStep => step_once = true,
                // Held state; platform surfaces it via input.fast_forward().
                UiEvent::FastForward => {}
                UiEvent::ToggleFullscreen => cab.toggle_fullscreen(),
                UiEvent::Reset => {
                    if powered {
                        core.reset();
                    }
                }
                UiEvent::TogglePause if powered => {
                    paused = !paused;
                    if paused {
                        // Load once on the way in; present_pause just
                        // redraws it every frame (plan §3.2/§3.4).
                        let (captures, thumb) = load_pause_note(&spec.notes_dir, &rom_hash);
                        cab.set_pause_note(
                            &title,
                            captures,
                            thumb.as_ref().map(|(w, h, d)| (*w, *h, d.as_slice())),
                        );
                    }
                    log::info!("{}", if paused { "paused" } else { "resumed" });
                }
                UiEvent::NextSlot if powered => {
                    slot = (slot + 1) % SLOTS;
                    log::info!("slot {slot}");
                }
                UiEvent::PrevSlot if powered => {
                    slot = (slot + SLOTS - 1) % SLOTS;
                    log::info!("slot {slot}");
                }
                UiEvent::SaveState if powered => {
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
                UiEvent::LoadState if powered => {
                    match state_file(&rom_hash, &spec.save_dir, slot).map(|p| fs::read(&p)) {
                        Some(Ok(s)) if core.load_state(&s) => log::info!("slot {slot}: loaded"),
                        Some(Ok(_)) => log::warn!("slot {slot}: core rejected the state"),
                        Some(Err(_)) => log::warn!("slot {slot}: empty"),
                        None => log::warn!("no state slot (unidentified ROM?)"),
                    }
                }
                UiEvent::Screenshot if powered => {
                    let p = spec.save_dir.join(format!("shot-{}.bmp", now_stamp()));
                    // capture happens after present, below; stash the request.
                    shot_request = Some(p);
                }
                UiEvent::CheatNext if powered && !cheat_defs.is_empty() => {
                    cheat_sel = (cheat_sel + 1) % cheat_defs.len();
                    cab.set_cheats(&cheat_rows(cheat_defs, &cheat_state), cheat_sel);
                }
                UiEvent::CheatPrev if powered && !cheat_defs.is_empty() => {
                    cheat_sel = (cheat_sel + cheat_defs.len() - 1) % cheat_defs.len();
                    cab.set_cheats(&cheat_rows(cheat_defs, &cheat_state), cheat_sel);
                }
                UiEvent::CheatToggle if powered && !cheat_defs.is_empty() => {
                    let on = !cheat_state[cheat_sel];
                    cheat_state[cheat_sel] = on;
                    core.cheat_set(cheat_sel as u32, on, cheat_defs[cheat_sel].code);
                    cab.set_cheats(&cheat_rows(cheat_defs, &cheat_state), cheat_sel);
                    save_cheat_state(&cheat_path, &cheat_state);
                    log::info!(
                        "cheat {:?}: {}",
                        cheat_defs[cheat_sel].desc,
                        if on { "on" } else { "off" }
                    );
                }
                UiEvent::NoteCapture if powered => {
                    // capture happens after present, below, same as Screenshot.
                    note_request = true;
                }
                // The rest only make sense with the console on; ignored off
                // (or, for the cheat trio, with nothing curated to toggle).
                UiEvent::TogglePause
                | UiEvent::NextSlot
                | UiEvent::PrevSlot
                | UiEvent::SaveState
                | UiEvent::LoadState
                | UiEvent::Screenshot
                | UiEvent::CheatNext
                | UiEvent::CheatPrev
                | UiEvent::CheatToggle
                | UiEvent::NoteCapture => {}
            }
        }

        if !powered {
            cab.set_session_time(session_start.elapsed());
            cab.present_static(OFF_STATIC_LEVEL);
            next += frame_time;
            let now = Instant::now();
            if next > now {
                std::thread::sleep(next - now);
            } else {
                next = now;
            }
            continue;
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
            if spec.debug_note_capture && frames == debug_note_frame {
                note_request = true;
            }
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
                cab.set_session_time(session_start.elapsed());
                cab.present_frame(&fref, aspect);

                if let Some(path) = shot_request.take() {
                    match cab.capture_bmp(&fref, aspect, &path) {
                        Ok(_) => log::info!("screenshot -> {}", path.display()),
                        Err(e) => log::warn!("screenshot failed: {e}"),
                    }
                }

                if note_request {
                    note_request = false;
                    match &rom_hash {
                        Some(h) => match append_note_image(&spec.notes_dir, h, &frame) {
                            Ok(_) => {
                                refresh_notes(cab, &spec.notes_dir, &rom_hash);
                                log::info!("note: captured");
                            }
                            Err(e) => log::warn!("note capture failed: {e}"),
                        },
                        None => log::warn!("note capture skipped (unidentified ROM)"),
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
                flush_sram(&sram_path, &mut last_sram, core.sram());
            }
        } else {
            // Paused, not stepping: the book, not a frozen game frame.
            cab.present_pause();
            if let Some(path) = shot_request.take() {
                match cab.capture_pause_bmp(&path) {
                    Ok(_) => log::info!("screenshot -> {}", path.display()),
                    Err(e) => log::warn!("screenshot failed: {e}"),
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
    flush_sram(&sram_path, &mut last_sram, core.sram());

    log::info!("game loop done: {exit:?}");
    Ok(exit)
}

#[cfg(test)]
mod tests {
    use super::{append_note_image, frame_to_rgb8, load_cheat_state, save_cheat_state, EmuFrame};
    use xperience_emulation::PixelFormat as EmuFormat;

    fn scratch_dir(name: &str) -> std::path::PathBuf {
        let dir =
            std::env::temp_dir().join(format!("xperience-test-{name}-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn cheat_state_round_trips_through_disk() {
        let dir = scratch_dir("cheats");
        let path = Some(dir.join("test.cheats"));

        save_cheat_state(&path, &[true, false, true]);
        assert_eq!(load_cheat_state(&path, 3), vec![true, false, true]);

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn missing_file_defaults_everything_off() {
        let path = Some(std::env::temp_dir().join("xperience-cheat-test-missing.cheats"));
        assert_eq!(load_cheat_state(&path, 2), vec![false, false]);
    }

    #[test]
    fn frame_to_rgb8_decodes_rgb565_bit_layout() {
        // Pure red, green, blue, white — 5-6-5 packed little-endian.
        let px: [u16; 4] = [0xF800, 0x07E0, 0x001F, 0xFFFF];
        let mut pixels = Vec::with_capacity(8);
        for p in px {
            pixels.extend_from_slice(&p.to_le_bytes());
        }
        let frame = EmuFrame {
            width: 4,
            height: 1,
            pitch: 8,
            format: EmuFormat::Rgb565,
            pixels,
        };
        let img = frame_to_rgb8(&frame);
        assert_eq!(img.get_pixel(0, 0).0, [255, 0, 0]);
        assert_eq!(img.get_pixel(1, 0).0, [0, 255, 0]);
        assert_eq!(img.get_pixel(2, 0).0, [0, 0, 255]);
        assert_eq!(img.get_pixel(3, 0).0, [255, 255, 255]);
    }

    #[test]
    fn note_capture_appends_markdown_and_saves_a_png() {
        let dir = scratch_dir("notes");
        let frame = EmuFrame {
            width: 2,
            height: 2,
            pitch: 4,
            format: EmuFormat::Rgb565,
            pixels: vec![0u8; 4 * 2],
        };

        append_note_image(&dir, "deadbeef", &frame).unwrap();
        append_note_image(&dir, "deadbeef", &frame).unwrap();

        let md = std::fs::read_to_string(dir.join("deadbeef.md")).unwrap();
        assert_eq!(md.matches("![captura]").count(), 2);
        let images: Vec<_> = std::fs::read_dir(dir.join("deadbeef"))
            .unwrap()
            .filter_map(|e| e.ok())
            .collect();
        assert_eq!(images.len(), 2);

        std::fs::remove_dir_all(&dir).ok();
    }
}
