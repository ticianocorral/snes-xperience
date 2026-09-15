//! The emulator run-loop, factored out of the `emu-run` binary so the unified
//! `xperience` binary can call it between selector visits. Presentation is fixed:
//! RF NTSC + CRT-tube warp (see docs/fase-0.md).

use std::collections::HashMap;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use anyhow::{anyhow, Context, Result};
use xperience_emulation::{Button, Core, Frame as EmuFrame, PixelFormat as EmuFormat};
use xperience_ntsc::{NtscFilter, Preset};
use xperience_platform::{
    Cabinet, FrameRef, PanelButton, PixelFormat as PlatFormat, Platform, UiEvent, MAX_PORTS,
};

use crate::config::Config;

/// How often to flush battery SRAM to disk while playing (frames ≈ 10 s).
const SRAM_FLUSH_FRAMES: u32 = 600;
/// Save-state slots (keys 0..9 of the ROM hash).
const SLOTS: u8 = 10;
/// Emulated frames per shown frame while fast-forward is held.
const FF_SPEED: u32 = 8;
/// Max characters a pause-book free-text note may hold (plan revision) —
/// enforced live, as it's typed, not just on save.
const NOTE_CHAR_LIMIT: usize = 240;
/// The dim, near-still hiss the screen idles at once the console is off
/// (plan §3.3) — also what the next screen fades in from, and the idle/root
/// screen's resting level (`crate::idle`).
pub(crate) const OFF_STATIC_LEVEL: f32 = 0.12;

/// Why the run-loop returned.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum GameExit {
    /// Ejected — the caller should show the idle/root screen again (not the
    /// shelf directly). Carries the signal-off static level the screen
    /// settled on, so the next screen can fade in over it instead of a fresh
    /// burst (plan §3.3, "a estante entra por cima").
    Ejected { static_level: f32 },
    /// Window close / Cmd-Q / headless self-check done — tear the app down.
    Quit,
}

/// Everything [`run_game`] needs for one session.
pub struct GameSpec {
    pub core: PathBuf,
    pub rom: PathBuf,
    pub system_dir: PathBuf,
    pub save_dir: PathBuf,
    /// Where per-game notebooks live: `<title>/01.png`..`15.png` (fixed
    /// slots, plan revision) plus `<title>/notas.txt` for free text
    /// (plan §3.4).
    pub notes_dir: PathBuf,
    /// Speculative frames past the shown one; `None` = take the config value.
    pub runahead: Option<u32>,
    /// Headless self-check: `(path, frame)` — run to `frame`, dump a BMP, exit.
    pub shot: Option<(PathBuf, u32)>,
    /// Local logo art (`assets/logo/<rom>.*`) for the top of the side panel.
    /// `None` shows the ROM's name instead (plan §3.2, item 1).
    pub logo: Option<PathBuf>,
    /// Local cartridge art (`assets/cartridge/<rom>.*`), shown in the panel
    /// alongside the logo when present (plan revision) — `None` just skips
    /// that block, no fallback needed.
    pub cartridge: Option<PathBuf>,
    /// Headless self-check: skip straight to the console-off signal-off snow
    /// and save `shot` there, instead of running the game to `shot`'s frame
    /// count.
    pub shot_off: bool,
    /// Headless self-check: force one `NoteCapture` at `shot`'s frame (or
    /// frame 1 without one), exactly like clicking "Nota" live — so `--shot`
    /// can prove out the panel's notebook block without a window.
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

/// How long a silent action's button shows "feito!" after firing (plan
/// revision, fixing a real report: clicking Nota gave zero on-screen
/// feedback, so a player clicked it seven times thinking nothing happened —
/// it had, every time). Long enough to register as intentional, short
/// enough to not look stuck.
const FLASH_DURATION: Duration = Duration::from_millis(900);

/// Whether `b`'s flash is still showing — `flash` maps a button to when it
/// last fired, only ever holding entries for buttons that flash at all.
fn flashed(flash: &HashMap<PanelButton, Instant>, b: PanelButton) -> bool {
    flash.get(&b).is_some_and(|t| t.elapsed() < FLASH_DURATION)
}

/// How long Reset's rocker stays "up" after a click before springing back
/// down on its own (plan revision — a momentary switch, not a toggle like
/// Power's). Shorter than `FLASH_DURATION`: a real spring-back reads as
/// snappy, not as a held state to notice.
const RESET_SPRING: Duration = Duration::from_millis(220);

/// Whether Reset's rocker should be drawn up right now — same `flash` map
/// the "feito!" labels use, just a different (shorter) window.
fn reset_pressed(flash: &HashMap<PanelButton, Instant>) -> bool {
    flash
        .get(&PanelButton::Reset)
        .is_some_and(|t| t.elapsed() < RESET_SPRING)
}

/// The side panel's command legend (plan §3.2, item 3; plan revision:
/// mouse/gamepad only, so labels don't carry a key name any more) — some
/// live-refreshed every frame via `Cabinet::set_commands` since their text
/// depends on state (`slot`, `turbo_on`, `flash`), not just the console
/// being on. Power/Eject/Reset aren't in here (plan revision): they're drawn
/// as their own rocker-switch widgets straight off `panel.powered`/
/// `panel.reset_pressed` (see `draw_panel`/`draw_rocker`), not a text label.
fn command_rows(
    slot: u8,
    note_slot: u8,
    turbo_on: bool,
    flash: &HashMap<PanelButton, Instant>,
    all_slots_pinned: bool,
) -> Vec<(PanelButton, String)> {
    let label = |b: PanelButton, base: &str| -> String {
        if flashed(flash, b) {
            format!("{base} (feito!)")
        } else {
            base.to_string()
        }
    };
    vec![
        (PanelButton::Pause, "Pausar".to_string()),
        (
            PanelButton::NoteCapture,
            if all_slots_pinned {
                // Every slot resists overwrite — a click here would have
                // nowhere to land, so say so instead of the normal label
                // (plan revision: "avisar quando 15 fixados").
                "Nota: sem espaco (15 fixados)".to_string()
            } else {
                label(
                    PanelButton::NoteCapture,
                    &format!("Nota (slot {note_slot})"),
                )
            },
        ),
        (PanelButton::NoteSlot, format!("Nota slot: {note_slot}")),
        (
            PanelButton::SaveState,
            label(PanelButton::SaveState, &format!("Salvar (slot {slot})")),
        ),
        (
            PanelButton::LoadState,
            label(PanelButton::LoadState, &format!("Carregar (slot {slot})")),
        ),
        (PanelButton::NextSlot, format!("Slot: {slot}")),
        (
            PanelButton::Turbo,
            format!("Turbo: {}", if turbo_on { "ligado" } else { "desligado" }),
        ),
    ]
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

/// Ligar de novo: the mirror of `power_off_burst` — snow clears from a dim
/// hiss back up to a brief bright burst, then cuts to the game resuming
/// exactly where it was paused.
fn power_on_burst(plat: &Platform, cab: &mut Cabinet) {
    const RATE: u32 = 22_050;
    const SPAN: Duration = Duration::from_millis(450);
    let audio = plat.open_audio(RATE).ok();
    let frame = Duration::from_millis(16);
    let mut rng: u32 = 0x8765_4321;
    let start = Instant::now();

    while start.elapsed() < SPAN {
        let t = (start.elapsed().as_secs_f32() / SPAN.as_secs_f32()).min(1.0);
        let level = OFF_STATIC_LEVEL + (1.0 - OFF_STATIC_LEVEL) * t;
        cab.present_static(level);

        if let Some(a) = &audio {
            let n = (RATE / 60) as usize;
            let amp = (t * 8000.0) as i32;
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
        a.clear();
    }
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

/// Decode scraped art (panel logo, note thumbnail, …) small enough for its
/// slot, keeping alpha for transparent logos.
fn decode_art(path: &Path, max: u32) -> Result<(u32, u32, Vec<u8>)> {
    let img = image::open(path)?.thumbnail(max, max).to_rgba8();
    let (w, h) = img.dimensions();
    Ok((w, h, img.into_raw()))
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

/// Fixed note slots per game (plan revision, replacing an open-ended
/// timestamped list): a screenshot into the notebook always lands in one of
/// these, chosen by the player, same shape as the save-state slots.
const NOTE_SLOTS: u8 = 15;
/// Max characters a slot's caption may hold — short, it's a label, not a
/// page (contrast `NOTE_CHAR_LIMIT` for the free-text notebook page).
const SLOT_NAME_LIMIT: usize = 40;

/// Which of the two things the pause book's one text editor is currently
/// editing (plan revision) — they share all the input-polling plumbing,
/// just write to a different place and have a different character limit.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum NoteEdit {
    None,
    /// Appending a page to the free-text notebook (`notas.txt`).
    Text,
    /// Setting the caption on the current note slot.
    SlotName,
}

impl NoteEdit {
    fn limit(self) -> usize {
        match self {
            NoteEdit::None => 0,
            NoteEdit::Text => NOTE_CHAR_LIMIT,
            NoteEdit::SlotName => SLOT_NAME_LIMIT,
        }
    }

    fn heading(self) -> &'static str {
        match self {
            NoteEdit::None => "",
            NoteEdit::Text => "escrevendo anotacao (clique fora cancela)",
            NoteEdit::SlotName => "nomeando o print (clique fora cancela)",
        }
    }
}

/// One note slot's protection/caption (plan revision) — everything defaults
/// to "untouched" (not pinned, no caption) for a slot nobody's set either on.
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
struct SlotMeta {
    #[serde(default)]
    pinned: bool,
    #[serde(default)]
    label: String,
}

/// `notes_dir/<title>/slots.json`'s shape: which of the 15 slots are pinned
/// or captioned, keyed by slot number. Absent entries mean the default
/// (`SlotMeta::default()`) — most slots never need a real entry.
#[derive(Debug, Default, serde::Serialize, serde::Deserialize)]
struct NotesMeta {
    #[serde(default)]
    slots: std::collections::BTreeMap<u8, SlotMeta>,
}

impl NotesMeta {
    fn slot(&self, slot: u8) -> SlotMeta {
        self.slots.get(&slot).cloned().unwrap_or_default()
    }
}

/// Every one of the 15 slots resists overwrite — `NoteCapture` has nowhere
/// left to redirect to (plan revision: "avisar quando 15 fixados").
fn all_slots_pinned(meta: &NotesMeta) -> bool {
    (1..=NOTE_SLOTS).all(|s| meta.slot(s).pinned)
}

fn notes_meta_path(notes_dir: &Path, title: &str) -> PathBuf {
    note_dir(notes_dir, title).join("slots.json")
}

/// Missing or unreadable is just "nothing pinned or captioned yet", not an
/// error — same tolerant read as the rest of this portable app's sidecars.
fn load_notes_meta(notes_dir: &Path, title: &str) -> NotesMeta {
    fs::read_to_string(notes_meta_path(notes_dir, title))
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

fn save_notes_meta(notes_dir: &Path, title: &str, meta: &NotesMeta) {
    let dir = note_dir(notes_dir, title);
    if fs::create_dir_all(&dir).is_err() {
        return;
    }
    if let Ok(s) = serde_json::to_string_pretty(meta) {
        if let Err(e) = fs::write(notes_meta_path(notes_dir, title), s) {
            log::warn!("saving note slot metadata failed: {e}");
        }
    }
}

/// `notes_dir/<game title>/` — one folder per game, named for a human
/// browsing it rather than the ROM hash (plan revision: readability over
/// rename-proofing, since the player picked this explicitly). Sanitized so
/// a title with a `/` or other path-hostile character can't escape it or
/// fail to create.
fn note_dir(notes_dir: &Path, title: &str) -> PathBuf {
    let safe: String = title
        .trim()
        .chars()
        .map(|c| {
            if c.is_ascii_control()
                || matches!(c, '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|')
            {
                '_'
            } else {
                c
            }
        })
        .collect();
    notes_dir.join(if safe.is_empty() {
        "___".to_string()
    } else {
        safe
    })
}

/// `notes_dir/<title>/01.png` .. `15.png` — 1-indexed to match the slot
/// numbers shown on screen.
fn note_slot_path(notes_dir: &Path, title: &str, slot: u8) -> PathBuf {
    note_dir(notes_dir, title).join(format!("{slot:02}.png"))
}

/// `notes_dir/<title>/notas.txt` — free-text pages (plan revision), separate
/// from the slotted images, one plain-text file per game.
fn note_text_path(notes_dir: &Path, title: &str) -> PathBuf {
    note_dir(notes_dir, title).join("notas.txt")
}

/// Save the current frame into note `slot` (1..=15), overwriting whatever
/// was there before — same "pick a slot, it replaces what's in it" model as
/// save states.
fn save_note_image(notes_dir: &Path, title: &str, slot: u8, frame: &EmuFrame) -> Result<()> {
    let dir = note_dir(notes_dir, title);
    fs::create_dir_all(&dir).with_context(|| format!("creating {}", dir.display()))?;
    let path = note_slot_path(notes_dir, title, slot);
    frame_to_rgb8(frame)
        .save(&path)
        .with_context(|| format!("saving note image {}", path.display()))
}

/// Append a free-text page to `notas.txt` (plan revision) — plain text, one
/// paragraph per entry, no slot of its own (unbounded, unlike the images).
fn append_note_text(notes_dir: &Path, title: &str, text: &str) -> Result<()> {
    let dir = note_dir(notes_dir, title);
    fs::create_dir_all(&dir).with_context(|| format!("creating {}", dir.display()))?;
    let path = note_text_path(notes_dir, title);
    let mut f = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
        .with_context(|| format!("opening {}", path.display()))?;
    writeln!(f, "{text}\n")?;
    Ok(())
}

/// How many of the 15 note slots actually hold an image.
fn count_filled_slots(notes_dir: &Path, title: &str) -> usize {
    (1..=NOTE_SLOTS)
        .filter(|&s| note_slot_path(notes_dir, title, s).is_file())
        .count()
}

/// Recompute the panel's notebook block from what's actually on disk: how
/// many of the 15 slots are used, and the currently-selected one (`slot`) as
/// a thumbnail if it's filled. Call at game start, after every capture, and
/// after cycling the note slot — cheap, at most 15 file-exists checks.
fn refresh_notes(cab: &mut Cabinet, notes_dir: &Path, title: &str, slot: u8) {
    let count = count_filled_slots(notes_dir, title);
    let thumb = note_thumb(notes_dir, title, slot, 200);
    cab.set_notes(
        count,
        thumb.as_ref().map(|(w, h, d)| (*w, *h, d.as_slice())),
    );
}

/// Decode note `slot`'s image if it's filled, `max` pixels on the long
/// side — `None` for an empty slot or a decode failure either way.
fn note_thumb(notes_dir: &Path, title: &str, slot: u8, max: u32) -> Option<(u32, u32, Vec<u8>)> {
    let path = note_slot_path(notes_dir, title, slot);
    path.is_file()
        .then(|| decode_art(&path, max))
        .and_then(Result::ok)
}

/// Push note `slot`'s current image, pin state, and caption onto the pause
/// book's right page — the `set_pause_page` call every slot-change (or
/// pin/rename) site needs.
fn show_note_slot(cab: &mut Cabinet, notes_dir: &Path, title: &str, slot: u8, meta: &NotesMeta) {
    let thumb = note_thumb(notes_dir, title, slot, 900);
    let m = meta.slot(slot);
    cab.set_pause_page(
        (slot - 1) as usize,
        m.pinned,
        &m.label,
        thumb.as_ref().map(|(w, h, d)| (*w, *h, d.as_slice())),
    );
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
    // Note slot (plan revision: fixed 1..=15, not save-state's 0..=9) — the
    // same value both "Nota" captures into and the pause book's right page
    // shows, cycled by "Nota slot: N" (gameplay) or Prev/Next (paused).
    let mut note_slot: u8 = 1;

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

    let title = spec
        .rom
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| "???".to_string());
    // Which note slots are pinned/captioned (plan revision) — loaded once,
    // mutated and re-saved in place on every pin toggle or rename.
    let mut notes_meta = load_notes_meta(&spec.notes_dir, &title);

    // --- side panel: logo, cartridge art, command legend, session timer
    // (plan §3.2) — cartridge art is new (plan revision): a second, optional
    // image alongside the logo, same local-file convention.
    let mut turbo_on = false;
    // "Done!" flash for otherwise-silent actions (Nota/Salvar/Carregar) —
    // see `flashed`/`FLASH_DURATION`.
    let mut flash: HashMap<PanelButton, Instant> = HashMap::new();
    let commands = command_rows(
        slot,
        note_slot,
        turbo_on,
        &flash,
        all_slots_pinned(&notes_meta),
    );
    let decode_panel_art = |path: &Option<PathBuf>, kind: &str| {
        path.as_ref().and_then(|p| match decode_art(p, 640) {
            Ok(img) => Some(img),
            Err(e) => {
                log::warn!("{kind} {}: {e}", p.display());
                None
            }
        })
    };
    let logo_img = decode_panel_art(&spec.logo, "logo");
    let cartridge_img = decode_panel_art(&spec.cartridge, "cartridge");
    cab.set_panel(
        logo_img.as_ref().map(|(w, h, d)| (*w, *h, d.as_slice())),
        cartridge_img
            .as_ref()
            .map(|(w, h, d)| (*w, *h, d.as_slice())),
        &title,
        &commands,
    );

    // --- cheats: a curated slice of libretro-database codes (plan §4.4) ---
    // Matched by the cartridge header title, not the file — see
    // `xperience_domain::cheats`. Empty for anything we haven't picked yet.
    let cheat_defs = xperience_domain::cheats_for_title(internal_name.as_deref().unwrap_or(""));
    let cheat_path = cheat_state_path(&rom_hash, &spec.save_dir);
    let mut cheat_state = load_cheat_state(&cheat_path, cheat_defs.len());
    if !cheat_defs.is_empty() {
        core.cheat_reset();
        for (i, (def, &on)) in cheat_defs.iter().zip(&cheat_state).enumerate() {
            core.cheat_set(i as u32, on, def.code);
        }
    }
    cab.set_cheats(&cheat_rows(cheat_defs, &cheat_state));

    // --- notes: the notebook block, empty until the first capture (§3.4) --
    fs::create_dir_all(&spec.notes_dir).ok();
    refresh_notes(cab, &spec.notes_dir, &title, note_slot);

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
            let filled = count_filled_slots(&spec.notes_dir, &title);
            cab.set_pause_note(&title, NOTE_SLOTS as usize, filled);
            show_note_slot(cab, &spec.notes_dir, &title, note_slot, &notes_meta);
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
    // Which note slot to save into once the next frame is ready — set at
    // click time, consumed after `core.run()` produces a real frame.
    let mut note_request: Option<u8> = None;
    // Debug capture lines up with --shot-frame (default: the very first
    // frame) so the saved page actually shows whatever --shot is inspecting,
    // not just a black boot frame.
    let debug_note_frame = spec.shot.as_ref().map_or(1, |(_, f)| *f).max(1);
    // The console: off until the player presses Power (plan revision —
    // picking a game from the shelf only inserts the cartridge, same as
    // real hardware, it doesn't boot itself); off again after a later
    // "Desligar", idling on snow until Eject either way (plan §3.3).
    // Exception: a plain `--shot` dev capture (not `--shot-off`/
    // `--debug-shot-pause`, both of which already returned above) exists to
    // inspect live gameplay, so it starts powered — there's no Power click
    // to send it in headless mode.
    let mut powered = spec.shot.is_some();
    cab.set_powered(powered);
    let mut static_level = OFF_STATIC_LEVEL;
    // Drives the one deliberate keyboard-typing exception (plan revision),
    // gated entirely on `paused` — either the free-text note or a slot's
    // caption, never both (see `NoteEdit`).
    let mut note_edit = NoteEdit::None;
    let mut note_draft = String::new();
    log::info!("running: rf ntsc + crt tube, run-ahead {runahead}, slot {slot}");

    let exit = 'run: loop {
        let mut step_once = false;
        // Editing (text or a caption) is the one deliberate keyboard-typing
        // exception (plan revision) — while it's open, poll for composed
        // text/backspace/commit/cancel instead of gameplay input, so a key
        // meant for the editor doesn't also twitch the D-pad underneath it.
        if note_edit != NoteEdit::None {
            let te = plat.poll_text_entry();
            if te.quit {
                break 'run GameExit::Quit;
            }
            if te.backspace {
                note_draft.pop();
            }
            for c in te.typed.chars() {
                if note_draft.chars().count() < note_edit.limit() {
                    note_draft.push(c);
                }
            }
            if !te.typed.is_empty() || te.backspace {
                cab.set_pause_draft(Some(&note_draft), note_edit.limit(), note_edit.heading());
            }
            let mut save = te.commit;
            let mut cancel = te.cancel;
            if let Some((x, y)) = te.click {
                let (ox, oy) = cab.window_to_output(x, y);
                match cab.hit_pause_button(ox, oy) {
                    Some(PanelButton::PauseDraftSave) => save = true,
                    Some(PanelButton::PauseDraftCancel) => cancel = true,
                    _ => {}
                }
            }
            if save {
                match note_edit {
                    NoteEdit::Text if !note_draft.trim().is_empty() => {
                        match append_note_text(&spec.notes_dir, &title, note_draft.trim()) {
                            Ok(()) => log::info!("note: text page saved"),
                            Err(e) => log::warn!("note text failed: {e}"),
                        }
                    }
                    NoteEdit::SlotName => {
                        notes_meta.slots.entry(note_slot).or_default().label =
                            note_draft.trim().to_string();
                        save_notes_meta(&spec.notes_dir, &title, &notes_meta);
                        show_note_slot(cab, &spec.notes_dir, &title, note_slot, &notes_meta);
                        log::info!("note slot {note_slot}: renamed");
                    }
                    _ => {}
                }
            }
            if save || cancel {
                note_edit = NoteEdit::None;
                note_draft.clear();
                cab.set_pause_draft(None, 0, "");
                plat.stop_text_input(cab);
            }
            cab.present_pause();
            next += frame_time;
            let now = Instant::now();
            if next > now {
                std::thread::sleep(next - now);
            } else {
                next = now;
            }
            continue;
        }

        // A clicked panel/pause-book button becomes exactly the `UiEvent` its
        // key used to send — the match below doesn't need to know clicks
        // exist at all. Which set of buttons a click can land on depends on
        // `paused`: the pause book replaces the whole window (no side panel
        // drawn alongside it), so it has its own hit-test.
        let events: Vec<UiEvent> = plat
            .poll(&mut input, &cfg.keymap)
            .into_iter()
            .filter_map(|ev| match ev {
                UiEvent::Click(x, y) => {
                    let (ox, oy) = cab.window_to_output(x, y);
                    if paused {
                        // Not reachable here while `note_edit != None` — that
                        // branch polls via `poll_text_entry` instead (below
                        // the main match), so `Click` never comes through
                        // `plat.poll()` during it.
                        match cab.hit_pause_button(ox, oy) {
                            Some(PanelButton::PauseContinue) => Some(UiEvent::TogglePause),
                            Some(PanelButton::PauseStep) => Some(UiEvent::FrameStep),
                            Some(PanelButton::PauseNotePrev) => Some(UiEvent::NotePrev),
                            Some(PanelButton::PauseNoteNext) => Some(UiEvent::NoteNext),
                            Some(PanelButton::PauseWrite) => Some(UiEvent::NoteWriteStart),
                            Some(PanelButton::PauseNotePin) => Some(UiEvent::NotePinToggle),
                            Some(PanelButton::PauseNoteName) => Some(UiEvent::NoteNameStart),
                            Some(PanelButton::CheatRow(i)) => Some(UiEvent::CheatToggle(i)),
                            _ => None,
                        }
                    } else {
                        cab.hit_panel_button(ox, oy).and_then(|b| match b {
                            PanelButton::Power => Some(UiEvent::Quit),
                            PanelButton::Eject => Some(UiEvent::Eject),
                            PanelButton::Reset => Some(UiEvent::Reset),
                            PanelButton::Pause => Some(UiEvent::TogglePause),
                            PanelButton::NoteCapture => Some(UiEvent::NoteCapture),
                            PanelButton::NoteSlot => Some(UiEvent::NoteSlotNext),
                            PanelButton::SaveState => Some(UiEvent::SaveState),
                            PanelButton::LoadState => Some(UiEvent::LoadState),
                            PanelButton::NextSlot => Some(UiEvent::NextSlot),
                            PanelButton::Turbo => Some(UiEvent::ToggleFastForward),
                            // Cheats are only clickable from the pause book
                            // now (plan revision) — same for the rest of
                            // these, all idle-screen- or pause-book-only,
                            // never shown alongside the panel that's up now.
                            PanelButton::Insert
                            | PanelButton::Settings
                            | PanelButton::CheatRow(_)
                            | PanelButton::PauseContinue
                            | PanelButton::PauseStep
                            | PanelButton::PauseNotePrev
                            | PanelButton::PauseNoteNext
                            | PanelButton::PauseWrite
                            | PanelButton::PauseNotePin
                            | PanelButton::PauseNoteName
                            | PanelButton::PauseDraftSave
                            | PanelButton::PauseDraftCancel => None,
                        })
                    }
                }
                other => Some(other),
            })
            .collect();
        for ev in events {
            match ev {
                UiEvent::Quit => {
                    if powered {
                        // Desligar (plan §3.3): flush the cart, then the
                        // signal-off ritual. A second click while already off
                        // does nothing on purpose — it's Ligar (below) now.
                        flush_sram(&sram_path, &mut last_sram, core.sram());
                        cab.set_session_time(session_start.elapsed());
                        static_level = power_off_burst(plat, cab);
                        powered = false;
                        cab.set_powered(false);
                        log::info!("power off — eject to leave, click power to resume");
                    } else {
                        // Ligar de novo: same button as power off, now
                        // toggling back on — the game resumes exactly where
                        // it was, no reload.
                        power_on_burst(plat, cab);
                        powered = true;
                        cab.set_powered(true);
                        log::info!("power on — resuming");
                    }
                }
                UiEvent::Eject => {
                    if powered {
                        eject_clunk(plat); // lock resists while it's still on
                    } else {
                        break 'run GameExit::Ejected { static_level };
                    }
                }
                UiEvent::CloseRequested => break 'run GameExit::Quit,
                UiEvent::FrameStep => step_once = true,
                UiEvent::ToggleFastForward => {
                    turbo_on = !turbo_on;
                    input.set_fast_forward(turbo_on);
                    log::info!("turbo {}", if turbo_on { "on" } else { "off" });
                }
                UiEvent::Reset => {
                    if powered {
                        core.reset();
                        // Momentary rocker (plan revision) — springs back on
                        // its own next frame via `reset_pressed`/`RESET_SPRING`,
                        // same clock as the "feito!" flashes.
                        flash.insert(PanelButton::Reset, Instant::now());
                    }
                }
                UiEvent::TogglePause if powered => {
                    paused = !paused;
                    if paused {
                        // Load once on the way in; present_pause just
                        // redraws it every frame (plan §3.2/§3.4) — on
                        // whichever note slot was last selected.
                        let filled = count_filled_slots(&spec.notes_dir, &title);
                        cab.set_pause_note(&title, NOTE_SLOTS as usize, filled);
                        show_note_slot(cab, &spec.notes_dir, &title, note_slot, &notes_meta);
                    }
                    log::info!("{}", if paused { "paused" } else { "resumed" });
                }
                UiEvent::NotePrev if paused && note_slot > 1 => {
                    note_slot -= 1;
                    show_note_slot(cab, &spec.notes_dir, &title, note_slot, &notes_meta);
                }
                UiEvent::NoteNext if paused && note_slot < NOTE_SLOTS => {
                    note_slot += 1;
                    show_note_slot(cab, &spec.notes_dir, &title, note_slot, &notes_meta);
                }
                UiEvent::NoteWriteStart if paused => {
                    note_edit = NoteEdit::Text;
                    note_draft.clear();
                    cab.set_pause_draft(Some(""), NoteEdit::Text.limit(), NoteEdit::Text.heading());
                    plat.start_text_input(cab);
                }
                UiEvent::NotePinToggle if paused => {
                    let m = notes_meta.slots.entry(note_slot).or_default();
                    m.pinned = !m.pinned;
                    let now_pinned = m.pinned;
                    save_notes_meta(&spec.notes_dir, &title, &notes_meta);
                    show_note_slot(cab, &spec.notes_dir, &title, note_slot, &notes_meta);
                    log::info!(
                        "note slot {note_slot}: {}",
                        if now_pinned { "pinned" } else { "unpinned" }
                    );
                }
                UiEvent::NoteNameStart if paused => {
                    note_edit = NoteEdit::SlotName;
                    note_draft = notes_meta.slot(note_slot).label;
                    cab.set_pause_draft(
                        Some(&note_draft),
                        NoteEdit::SlotName.limit(),
                        NoteEdit::SlotName.heading(),
                    );
                    plat.start_text_input(cab);
                }
                UiEvent::NextSlot if powered => {
                    slot = (slot + 1) % SLOTS;
                    log::info!("slot {slot}");
                }
                UiEvent::NoteSlotNext if powered => {
                    note_slot = if note_slot >= NOTE_SLOTS {
                        1
                    } else {
                        note_slot + 1
                    };
                    refresh_notes(cab, &spec.notes_dir, &title, note_slot);
                    log::info!("note slot {note_slot}");
                }
                UiEvent::SaveState if powered => {
                    match (
                        state_file(&rom_hash, &spec.save_dir, slot),
                        core.save_state(),
                    ) {
                        (Some(p), Some(s)) => match fs::write(&p, &s) {
                            Ok(_) => {
                                log::info!("slot {slot}: saved ({} KiB)", s.len() / 1024);
                                flash.insert(PanelButton::SaveState, Instant::now());
                            }
                            Err(e) => log::warn!("slot {slot}: save failed: {e}"),
                        },
                        _ => log::warn!("no state slot (unidentified ROM?)"),
                    }
                }
                UiEvent::LoadState if powered => {
                    match state_file(&rom_hash, &spec.save_dir, slot).map(|p| fs::read(&p)) {
                        Some(Ok(s)) if core.load_state(&s) => {
                            log::info!("slot {slot}: loaded");
                            flash.insert(PanelButton::LoadState, Instant::now());
                        }
                        Some(Ok(_)) => log::warn!("slot {slot}: core rejected the state"),
                        Some(Err(_)) => log::warn!("slot {slot}: empty"),
                        None => log::warn!("no state slot (unidentified ROM?)"),
                    }
                }
                UiEvent::CheatToggle(i) if powered && i < cheat_defs.len() => {
                    let on = !cheat_state[i];
                    cheat_state[i] = on;
                    core.cheat_set(i as u32, on, cheat_defs[i].code);
                    cab.set_cheats(&cheat_rows(cheat_defs, &cheat_state));
                    save_cheat_state(&cheat_path, &cheat_state);
                    log::info!(
                        "cheat {:?}: {}",
                        cheat_defs[i].desc,
                        if on { "on" } else { "off" }
                    );
                }
                UiEvent::NoteCapture if powered => {
                    // A pinned slot resists a capture same as Eject resists
                    // a powered-on console (plan revision): search forward
                    // from `note_slot` (wrapping) for the first unpinned
                    // one and use that instead, silently redirecting; if
                    // all 15 are pinned there's nowhere to put it — the
                    // "Nota" label itself already says so persistently
                    // (`all_slots_pinned`, in `command_rows`), so this just
                    // no-ops instead of also flashing a misleading "feito!".
                    let target = (0..NOTE_SLOTS)
                        .map(|i| ((note_slot - 1 + i) % NOTE_SLOTS) + 1)
                        .find(|&s| !notes_meta.slot(s).pinned);
                    match target {
                        Some(s) => {
                            note_slot = s;
                            // capture happens after present, below, once a
                            // real frame is ready.
                            note_request = Some(s);
                            flash.insert(PanelButton::NoteCapture, Instant::now());
                            refresh_notes(cab, &spec.notes_dir, &title, note_slot);
                        }
                        None => {
                            log::warn!("note capture skipped: all {NOTE_SLOTS} slots pinned");
                        }
                    }
                }
                // The rest only make sense with the console on (or, for the
                // pause-book trio, only while actually paused); ignored
                // otherwise. `Click` never reaches this match — it's already
                // resolved into one of the arms above (or dropped) before
                // the loop.
                UiEvent::TogglePause
                | UiEvent::NextSlot
                | UiEvent::SaveState
                | UiEvent::LoadState
                | UiEvent::CheatToggle(_)
                | UiEvent::NoteCapture
                | UiEvent::NoteSlotNext
                | UiEvent::NotePrev
                | UiEvent::NoteNext
                | UiEvent::NoteWriteStart
                | UiEvent::NotePinToggle
                | UiEvent::NoteNameStart
                | UiEvent::Click(..) => {}
            }
        }
        cab.set_commands(&command_rows(
            slot,
            note_slot,
            turbo_on,
            &flash,
            all_slots_pinned(&notes_meta),
        ));
        cab.set_reset_pressed(reset_pressed(&flash));

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
                note_request = Some(note_slot);
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

                if let Some(slot) = note_request.take() {
                    match save_note_image(&spec.notes_dir, &title, slot, &frame) {
                        Ok(_) => {
                            refresh_notes(cab, &spec.notes_dir, &title, note_slot);
                            log::info!("note: captured into slot {slot}");
                        }
                        Err(e) => log::warn!("note capture failed: {e}"),
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
    use super::{
        append_note_text, count_filled_slots, frame_to_rgb8, load_cheat_state, note_dir,
        save_cheat_state, save_note_image, EmuFrame,
    };
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
    fn note_slots_save_independently_and_count_correctly() {
        let dir = scratch_dir("notes");
        let frame = EmuFrame {
            width: 2,
            height: 2,
            pitch: 4,
            format: EmuFormat::Rgb565,
            pixels: vec![0u8; 4 * 2],
        };

        assert_eq!(count_filled_slots(&dir, "Aladdin"), 0);
        save_note_image(&dir, "Aladdin", 1, &frame).unwrap();
        save_note_image(&dir, "Aladdin", 15, &frame).unwrap();
        assert_eq!(count_filled_slots(&dir, "Aladdin"), 2);
        // Re-saving the same slot overwrites, doesn't add a second file.
        save_note_image(&dir, "Aladdin", 1, &frame).unwrap();
        assert_eq!(count_filled_slots(&dir, "Aladdin"), 2);

        assert!(note_dir(&dir, "Aladdin").join("01.png").is_file());
        assert!(note_dir(&dir, "Aladdin").join("15.png").is_file());

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn note_text_appends_to_its_own_txt_file() {
        let dir = scratch_dir("notes-text");
        append_note_text(&dir, "Aladdin", "primeira pagina").unwrap();
        append_note_text(&dir, "Aladdin", "segunda pagina").unwrap();

        let text = std::fs::read_to_string(note_dir(&dir, "Aladdin").join("notas.txt")).unwrap();
        assert!(text.contains("primeira pagina"));
        assert!(text.contains("segunda pagina"));

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn note_dir_sanitizes_path_hostile_titles() {
        let dir = scratch_dir("notes-sanitize");
        let d = note_dir(&dir, "Foo/Bar: The \"Game\"?");
        // A single direct child of `dir` — no path traversal, no nested
        // directories from the slashes/colons in the title.
        assert_eq!(d.parent(), Some(dir.as_path()));
        assert!(!d.file_name().unwrap().to_string_lossy().contains('/'));

        std::fs::remove_dir_all(&dir).ok();
    }
}
