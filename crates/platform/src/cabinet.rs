//! The one window: a static dark cabinet with the screen recessed into it. Both
//! the running game and the selector draw into that screen area — the game as a
//! frame through a barrel-distorted CRT mesh, the selector as flat 2D (rects,
//! bitmap text, letterboxed images). The cabinet furniture (the chamfer ring
//! from the window edge down to the glass) is redrawn every frame so nothing
//! ever recreates the window. NTSC colour bleed is applied upstream
//! (`xperience-ntsc`).

use std::collections::HashMap;
use std::time::Duration;

use noto_sans_mono_bitmap::{get_raster, FontWeight, RasterHeight};
use sdl3::pixels::{Color, FColor, PixelFormat as SdlFormat};
use sdl3::rect::Rect;
use sdl3::render::{
    BlendMode, ClippingRect, ScaleMode as SdlScaleMode, Texture, Vertex, WindowCanvas,
};
use sdl3::VideoSubsystem;

use crate::PlatformError;

/// CRT tube shape. `WARP` is how hard the edges bow (0 = flat); `VIGNETTE` is
/// how much the corners darken; `GRID` is the mesh resolution.
const CRT_WARP: f32 = 0.06;
const CRT_VIGNETTE: f32 = 0.22;
const CRT_GRID: usize = 32;

/// Cabinet around the tube. The screen is inset from the window by these
/// fractions (a bit more at the bottom for the "chin"); everything outside is
/// the cabinet face, chamfered down to a near-black recess at the screen edge.
const BEZEL_SIDE: f32 = 0.070;
const BEZEL_TOP: f32 = 0.070;
const BEZEL_CHIN: f32 = 0.110;
/// Cabinet face — dark warm-grey plastic. Reads as a surface, still far darker
/// than a lit game screen (plan §3.2).
const CABINET: (u8, u8, u8) = (40, 37, 33);
/// The lip right against the glass, in shadow.
const RECESS: (u8, u8, u8) = (4, 4, 5);
/// Reserved image-cache key for the side panel's logo art.
const PANEL_LOGO_IMG: u64 = u64::MAX - 1;
/// Reserved image-cache key for the panel's most-recent note thumbnail.
const PANEL_NOTE_IMG: u64 = u64::MAX - 2;
/// Reserved image-cache key for the pause book's right-hand page.
const PAUSE_THUMB_IMG: u64 = u64::MAX - 3;
/// Reserved image-cache key for the side panel's cartridge art.
const PANEL_CARTRIDGE_IMG: u64 = u64::MAX - 4;
/// Reserved image-cache key for the idle screen's console-brand logo (plan
/// revision — `assets/console.png`, set once via `Cabinet::set_console_logo`,
/// not per-game).
const PANEL_CONSOLE_LOGO_IMG: u64 = u64::MAX - 5;

/// The pause book: two pages, not warped by the tube — a dedicated screen
/// (plan §3.2/§3.4), not cabinet furniture, so it replaces the whole window
/// rather than sharing space with the panel/cartridge/brand.
const PAUSE_MARGIN: f32 = 0.06;
const PAUSE_GAP: f32 = 0.02;
const PAUSE_TOP: f32 = 0.08;

/// The side panel: a column of plain widgets beside the tube during play —
/// not warped, drawn straight on the window (plan §2's presentation order,
/// §3.2). A fixed fraction of the window, clamped so it neither disappears on
/// a small window nor swallows a huge one.
const PANEL_FRAC: f32 = 0.25;
const PANEL_MIN: u32 = 260;
const PANEL_MAX: u32 = 520;
const PANEL_BG: (u8, u8, u8) = (16, 15, 14);
const PANEL_TEXT: (u8, u8, u8) = (225, 220, 210);
const PANEL_DIM: (u8, u8, u8) = (140, 134, 124);
/// Fill for a clickable panel button (`draw_button`) — a shade lighter than
/// `PANEL_BG` so it reads as its own control, not flat background text.
const PANEL_BTN_BG: (u8, u8, u8) = (34, 32, 29);

/// Power/Reset rocker-switch colours (plan revision: styled after the real
/// console's purple switches, see `draw_rocker`) — a mid violet with a
/// lighter bevel sliver on the thumb's top edge and a dim grey stand-in for
/// "not interactive right now" (Reset while powered off).
const SWITCH_PURPLE: (u8, u8, u8) = (107, 70, 168);
const SWITCH_PURPLE_HI: (u8, u8, u8) = (152, 112, 214);
const SWITCH_DIM: (u8, u8, u8) = (58, 55, 62);
const SWITCH_TRACK_BG: (u8, u8, u8) = (24, 22, 26);
const SWITCH_TRACK_BORDER: (u8, u8, u8) = (60, 58, 64);

/// The set's own nameplate: a small wordmark printed into the chin, left of
/// the cartridge — a touch lighter than the cabinet plastic, like an embossed
/// badge rather than a lit label.
const BRAND: &str = "SNES Xperience";
const BRAND_TEXT: (u8, u8, u8) = (92, 86, 78);

/// Glyph cell (Noto Sans Mono, anti-aliased, rasterized once at boot into an
/// atlas texture — see `build_font_atlas`), before scaling: 20px tall gives
/// noticeably bigger, smoother UI text than the old 8x8 bitmap font while
/// staying monospace, so every existing cell-based layout calculation below
/// keeps working unchanged.
const FONT_HEIGHT: RasterHeight = RasterHeight::Size20;
const FONT_WEIGHT: FontWeight = FontWeight::Regular;
/// Advance width and line height of one glyph cell, before scaling.
const GLYPH_W: u32 = 9;
const GLYPH_H: u32 = 20;
/// Atlas covers one contiguous Unicode range: printable Basic Latin through
/// Latin-1 Supplement (space through `ÿ`) — plain ASCII plus the accented
/// letters Portuguese needs (á, ã, ç, é, õ, ...), in one indexable block.
const GLYPH_FIRST: u32 = 0x20;
const GLYPH_LAST: u32 = 0xFF;
const GLYPH_COLS: u32 = GLYPH_LAST - GLYPH_FIRST + 1;

/// Map a char to its column in the font atlas; anything outside the covered
/// range (or with no glyph in the font) falls back to `?`.
fn glyph_index(ch: char) -> u32 {
    let c = ch as u32;
    if (GLYPH_FIRST..=GLYPH_LAST).contains(&c) {
        c - GLYPH_FIRST
    } else {
        '?' as u32 - GLYPH_FIRST
    }
}

/// Pixel layout of a core framebuffer. Mirrors `xperience_emulation::PixelFormat`
/// so the platform layer stays independent of the emulation crate.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PixelFormat {
    Rgb1555,
    Xrgb8888,
    Rgb565,
}

impl PixelFormat {
    fn sdl(self) -> SdlFormat {
        match self {
            PixelFormat::Rgb1555 => SdlFormat::XRGB1555,
            PixelFormat::Xrgb8888 => SdlFormat::XRGB8888,
            PixelFormat::Rgb565 => SdlFormat::RGB565,
        }
    }
    fn bytes_per_pixel(self) -> usize {
        match self {
            PixelFormat::Rgb1555 | PixelFormat::Rgb565 => 2,
            PixelFormat::Xrgb8888 => 4,
        }
    }
}

/// A borrowed core frame ready to upload.
pub struct FrameRef<'a> {
    pub width: u32,
    pub height: u32,
    pub pitch: usize,
    pub format: PixelFormat,
    pub pixels: &'a [u8],
}

pub struct Cabinet {
    canvas: WindowCanvas,
    /// Streaming texture at the incoming frame's resolution (game path).
    src: Option<SrcTexture>,
    /// Cached CRT mesh; rebuilt only when the game rect resizes.
    mesh: Option<CrtMesh>,
    /// Cached cabinet mesh; rebuilt only when the window or screen rect changes.
    bezel: Option<BezelMesh>,
    /// Render target the selector draws its flat 2D into, then composited
    /// through the tube like a game frame.
    screen_tex: Option<SizedTex>,
    /// Small streaming texture for the signal-off snow.
    noise_tex: Option<SizedTex>,
    noise: Vec<u8>,
    rng: u32,
    /// 128 glyphs laid out horizontally, white on transparent (2D path).
    font: Texture,
    images: HashMap<u64, ImgTex>,
    /// The current inner-screen rect (the tube opening).
    screen: Rect,
    /// The side panel content (game path only, plan §3.2). `None` = no game
    /// loaded — the idle/root screen (§3.2 item 1's "Inserir cartucho"
    /// button) draws there instead.
    panel: Option<PanelInfo>,
    /// Elapsed time to show at the bottom of the panel; the caller updates
    /// this once a frame (`set_session_time`).
    session: Duration,
    /// The pause book's content (plan §3.2/§3.4), set once on entering pause
    /// and read every frame while `present_pause` is what's on screen.
    pause: Option<PauseNote>,
    fullscreen: bool,
    /// Clickable panel buttons drawn last frame, in output/canvas coordinates
    /// — `hit_panel_button` scans this. Repopulated by `present_frame`/
    /// `present_static` right after `draw_panel`.
    panel_buttons: Vec<(PanelButton, Rect)>,
    /// Clickable buttons on the pause book screen ("Continuar"/"Avancar
    /// quadro") drawn last frame — `hit_pause_button` scans this. The pause
    /// book replaces the whole window (no side panel drawn alongside it), so
    /// it needs its own list rather than sharing `panel_buttons`.
    pause_buttons: Vec<(PanelButton, Rect)>,
    /// Where the cabinet actually drew last frame, in real window/output
    /// pixels — always 16:9, letterboxed/pillarboxed to fit whatever the
    /// window's own shape is (plan: don't distort on an ultrawide monitor).
    /// Every other stored rect (`screen`, `panel_buttons`, …) lives in this
    /// rect's own local space; `window_to_output` subtracts its offset
    /// before any hit-test runs.
    canvas_rect: Rect,
}

/// A clickable spot in the side panel: the idle screen's "Inserir cartucho"
/// button, or one of the in-game console commands. `hit_panel_button` turns a
/// click into one of these; the caller (idle screen / `run_game`) decides
/// what each one does. `Hash` is for `runner`'s brief "done!" flash on
/// silent actions (screenshot, note capture, save/load) — keyed by button.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PanelButton {
    Insert,
    Settings,
    Power,
    Eject,
    Reset,
    Pause,
    NoteCapture,
    /// Cycle the note slot `NoteCapture` targets (plan revision: fixed
    /// 1..=15 slots, not a screenshot feature — that's gone, it didn't earn
    /// its keep next to a real notebook).
    NoteSlot,
    SaveState,
    LoadState,
    NextSlot,
    Turbo,
    /// One cheat row, addressed directly — a click both selects and toggles
    /// it, no separate cursor step (plan revision: mouse/gamepad only).
    /// Drawn on the pause book now, not the side panel (see `PanelInfo`'s
    /// doc comment) — there's room there, and it isn't racing gameplay for
    /// a slot in the always-visible panel.
    CheatRow(usize),
    /// Drawn only on the pause book screen (not the side panel): resume play.
    PauseContinue,
    /// Drawn only on the pause book screen: advance exactly one frame.
    PauseStep,
    /// Pause book: step the right page to an earlier/later capture.
    PauseNotePrev,
    PauseNoteNext,
    /// Pause book: open the free-text note editor on the left page.
    PauseWrite,
    /// Pause book, while writing: commit the draft to the notebook.
    PauseDraftSave,
    /// Pause book, while writing: discard the draft, back to the status view.
    PauseDraftCancel,
    /// Pause book: toggle whether the currently-shown slot is protected from
    /// being overwritten by a future "Nota" capture (plan revision).
    PauseNotePin,
    /// Pause book: open the editor for the currently-shown slot's caption
    /// (plan revision) — same text-editor UI as "Escrever anotacao", a
    /// shorter limit and a different destination.
    PauseNoteName,
}

/// The pause book: `captures` is the fixed note-slot count (plan revision),
/// `filled` how many actually hold an image, `has_thumb` whether the
/// currently-shown slot (`page`) decoded into one.
struct PauseNote {
    title: String,
    /// Total note slots (plan revision: a fixed 1..=15, always this many,
    /// unlike the open-ended list it used to be) — the pagination bound.
    captures: usize,
    /// How many of those slots actually hold an image — for the left
    /// page's status line, distinct from `captures` (the bound).
    filled: usize,
    /// 0-based index of the slot currently shown on the right page.
    page: usize,
    has_thumb: bool,
    /// Whether the shown slot is protected from a future "Nota" overwrite
    /// (plan revision), and its caption if one was given — both empty/false
    /// for a slot nobody's touched yet.
    pinned: bool,
    slot_label: String,
    /// `Some(text)` while the player is editing something (plan revision:
    /// either the free-text note or a slot's caption — `draft_heading`
    /// says which): replaces the left page's status with a live, editable
    /// draft. `draft_limit` is the max character count `text` may reach —
    /// shown as a counter alongside it.
    draft: Option<String>,
    draft_limit: usize,
    draft_heading: String,
}

/// What to draw at the top of the side panel: the `wheel` logo if we have
/// it, else the ROM's title, plus cartridge art right below when there's a
/// local file for it (plan revision — both are optional, independent of
/// each other). `commands` is the button legend (plan §3.2, item 3), one
/// clickable row per entry, rebuilt every frame by the caller since several
/// labels are live state (current slot, turbo on/off, ...) — see
/// `Cabinet::set_commands`. `cheats` (item 4, plan §4.4) is
/// informational-only here now — `(description, on)` pairs, only the ones
/// that are on get drawn, plain text; toggling moved to the pause book
/// (`draw_pause_book`), which isn't fighting the panel for vertical space.
struct PanelInfo {
    has_logo: bool,
    has_cartridge: bool,
    title: String,
    commands: Vec<(PanelButton, String)>,
    /// Whether the console is on right now — dims/brightens the command
    /// buttons (Eject only clunks while powered, Reset and everything past
    /// it only act while powered), and which way the Power rocker sits. Set
    /// via `Cabinet::set_powered`; starts `false` (plan revision: picking a
    /// game only inserts the cartridge, it doesn't start it — the player
    /// presses Power themselves, same as the real console).
    powered: bool,
    /// Reset's rocker is momentary (plan revision): true for a short spring
    /// window right after a click, then the caller (`runner`) lets it lapse —
    /// see `Cabinet::set_reset_pressed`. Power's rocker has no such flag; its
    /// position is just `powered` itself (a real toggle, stays where left).
    reset_pressed: bool,
    cheats: Vec<(String, bool)>,
    /// Notebook block (item 5, plan §3.4): absent entirely when this is 0 —
    /// no "no notes" filler.
    note_count: usize,
    has_note_thumb: bool,
}

struct SrcTexture {
    tex: Texture,
    w: u32,
    h: u32,
    format: PixelFormat,
}

/// A plain RGBA texture kept at a known size.
struct SizedTex {
    tex: Texture,
    w: u32,
    h: u32,
}

struct CrtMesh {
    verts: Vec<Vertex>,
    indices: Vec<i32>,
    w: u32,
    h: u32,
}

/// The chamfered ring from the window edge to the recessed screen.
struct BezelMesh {
    verts: Vec<Vertex>,
    indices: Vec<i32>,
    /// Cache key: (window w, window h, screen w, screen h).
    key: (u32, u32, u32, u32),
}

struct ImgTex {
    tex: Texture,
    w: u32,
    h: u32,
}

impl Cabinet {
    pub(crate) fn new(
        video: &VideoSubsystem,
        title: &str,
        width: u32,
        height: u32,
    ) -> Result<Self, PlatformError> {
        let window = video
            .window(title, width, height)
            .position_centered()
            .resizable()
            .build()
            .map_err(|e| PlatformError::Sdl(e.to_string()))?;
        let mut canvas = window.into_canvas();
        canvas.set_blend_mode(BlendMode::Blend);
        canvas.set_draw_color(Color::RGB(RECESS.0, RECESS.1, RECESS.2));
        canvas.clear();
        canvas.present();

        let font = build_font_atlas(&mut canvas)?;
        let (w, h) = canvas.output_size().unwrap_or((width, height));
        let canvas_rect = cabinet_canvas_rect(w, h);
        Ok(Self {
            canvas,
            src: None,
            mesh: None,
            bezel: None,
            screen_tex: None,
            noise_tex: None,
            noise: Vec::new(),
            rng: 0x9E37_79B9,
            font,
            images: HashMap::new(),
            screen: screen_area(canvas_rect.width(), canvas_rect.height()),
            panel: None,
            session: Duration::ZERO,
            pause: None,
            fullscreen: false,
            panel_buttons: Vec::new(),
            pause_buttons: Vec::new(),
            canvas_rect,
        })
    }

    pub fn toggle_fullscreen(&mut self) {
        self.fullscreen = !self.fullscreen;
        let _ = self.canvas.window_mut().set_fullscreen(self.fullscreen);
    }

    /// The underlying window — `Platform::start_text_input`/`stop_text_input`
    /// need it (SDL's text-input API is per-window).
    pub(crate) fn window(&self) -> &sdl3::video::Window {
        self.canvas.window()
    }

    /// Convert a click's window coordinates (what SDL reports) into the
    /// cabinet's own local space — what `panel_rect`/`draw_panel`/`screen`
    /// all lay out in. Two corrections stack here: window → real output
    /// pixels (they differ on a HiDPI display), then output → cabinet-local
    /// (subtracting `canvas_rect`'s offset — on an ultrawide window the
    /// cabinet is letterboxed, not the whole output). A point outside
    /// `canvas_rect` (in the letterbox bars) still comes back as a
    /// coordinate — callers bounds-check against their own rects, e.g.
    /// `hit_panel_button`/`hit_screen_point`.
    pub fn window_to_output(&self, x: i32, y: i32) -> (i32, i32) {
        let (ww, wh) = self.canvas.window().size();
        let (ow, oh) = self.canvas.output_size().unwrap_or((ww, wh));
        let sx = ow as f32 / ww.max(1) as f32;
        let sy = oh as f32 / wh.max(1) as f32;
        let (ox, oy) = ((x as f32 * sx) as i32, (y as f32 * sy) as i32);
        (ox - self.canvas_rect.x(), oy - self.canvas_rect.y())
    }

    /// Which panel button, if any, sits under an output-space point — the
    /// idle screen's "Inserir cartucho" or one of the in-game commands drawn
    /// last frame. Coordinates from a click go through `window_to_output`
    /// first.
    pub fn hit_panel_button(&self, out_x: i32, out_y: i32) -> Option<PanelButton> {
        self.panel_buttons
            .iter()
            .find(|(_, r)| r.contains_point((out_x, out_y)))
            .map(|(b, _)| *b)
    }

    /// Same as `hit_panel_button`, for the pause book's own buttons — a
    /// separate list since the pause screen replaces the whole window
    /// instead of sharing it with the side panel (see `pause_buttons`).
    pub fn hit_pause_button(&self, out_x: i32, out_y: i32) -> Option<PanelButton> {
        self.pause_buttons
            .iter()
            .find(|(_, r)| r.contains_point((out_x, out_y)))
            .map(|(b, _)| *b)
    }

    /// Map an output/canvas-space click into the 2D screen buffer's local
    /// coordinates — what `Screen::fill`/`text`/`image_fit` see, e.g. in the
    /// shelf's grid (plan §3.1). `None` outside the tube. Approximate: the
    /// buffer is warped onto the tube by `build_crt_mesh` (a mild barrel bow,
    /// `CRT_WARP`), which this ignores — a click right at the curved edge can
    /// land a few px off, but shelf tiles sit well inside it.
    pub fn hit_screen_point(&self, out_x: i32, out_y: i32) -> Option<(i32, i32)> {
        let (lx, ly) = (out_x - self.screen.x(), out_y - self.screen.y());
        if lx < 0 || ly < 0 || lx >= self.screen.width() as i32 || ly >= self.screen.height() as i32
        {
            return None;
        }
        Some((lx, ly))
    }

    /// Show the side panel during play: `logo` (width, height, RGBA) is the
    /// local art if there is one, else the panel falls back to `title` in
    /// text (plan §3.2, item 1); `cartridge` is a second, independent local
    /// image drawn right below it when present (plan revision — neither
    /// needs the other). `commands` is the initial button legend (item 3,
    /// plan revision: mouse-only) — see `set_commands` for the per-frame
    /// updates that follow (labels like the current slot or turbo state
    /// change live). Call once per game.
    pub fn set_panel(
        &mut self,
        logo: Option<(u32, u32, &[u8])>,
        cartridge: Option<(u32, u32, &[u8])>,
        title: &str,
        commands: &[(PanelButton, String)],
    ) {
        let has_logo = if let Some((w, h, rgba)) = logo {
            self.set_image(PANEL_LOGO_IMG, w, h, rgba);
            true
        } else {
            false
        };
        let has_cartridge = if let Some((w, h, rgba)) = cartridge {
            self.set_image(PANEL_CARTRIDGE_IMG, w, h, rgba);
            true
        } else {
            false
        };
        self.panel = Some(PanelInfo {
            has_logo,
            has_cartridge,
            title: title.to_string(),
            commands: commands.to_vec(),
            // A freshly inserted cartridge (plan revision): the console
            // doesn't boot itself any more — see `run_game`'s initial
            // `powered = false` — so the rocker starts down, not up.
            powered: false,
            reset_pressed: false,
            cheats: Vec::new(),
            note_count: 0,
            has_note_thumb: false,
        });
    }

    /// Refresh the command legend's labels (plan revision: several are live
    /// state now — the current save/load slot, whether turbo is on — so the
    /// caller rebuilds and passes this every frame instead of once). A no-op
    /// before `set_panel`.
    pub fn set_commands(&mut self, commands: &[(PanelButton, String)]) {
        if let Some(panel) = &mut self.panel {
            panel.commands = commands.to_vec();
        }
    }

    /// Dim/brighten the command buttons to match the console's power state
    /// (Eject only clunks while powered, Reset only acts while powered) — see
    /// `PanelInfo::powered`. A no-op before `set_panel`.
    pub fn set_powered(&mut self, powered: bool) {
        if let Some(panel) = &mut self.panel {
            panel.powered = powered;
        }
    }

    /// Reset's rocker springs up for a short moment after a click, then back
    /// down on its own (plan revision) — the caller (`runner`) recomputes
    /// this every frame from its own click timestamp, same pattern as the
    /// "(feito!)" flash on the other buttons.
    pub fn set_reset_pressed(&mut self, pressed: bool) {
        if let Some(panel) = &mut self.panel {
            panel.reset_pressed = pressed;
        }
    }

    /// Drop the last game's panel (logo, commands, cheats, notes, clock) —
    /// call on the way back to the idle/root screen, so `draw_panel` shows
    /// the "Inserir cartucho" button instead of the previous game's stale
    /// info. A no-op if there's nothing set.
    pub fn clear_panel(&mut self) {
        self.panel = None;
    }

    /// The idle screen's console-brand logo (plan revision: the idle/root
    /// screen — startup, backing out of the shelf, or ejecting a game, all
    /// the same screen — shows this where a game's own logo would go).
    /// `assets/console.png`, decoded once by the caller (`idle::run`); a
    /// no-op with `None` (the file doesn't exist) — `draw_panel`'s idle
    /// branch falls back to plain text the same way a missing per-game logo
    /// does.
    pub fn set_console_logo(&mut self, logo: Option<(u32, u32, &[u8])>) {
        if let Some((w, h, rgba)) = logo {
            self.set_image(PANEL_CONSOLE_LOGO_IMG, w, h, rgba);
        }
    }

    /// Update the panel's notebook block (plan §3.4, item 5): `count` of the
    /// 15 note slots (plan revision) filled so far, `thumb` the
    /// currently-selected slot's image (width, height, RGBA) if it has one.
    /// Call once at game start and again after every capture or slot change.
    /// A no-op before `set_panel`.
    pub fn set_notes(&mut self, count: usize, thumb: Option<(u32, u32, &[u8])>) {
        let has_thumb = if let Some((w, h, rgba)) = thumb {
            self.set_image(PANEL_NOTE_IMG, w, h, rgba);
            true
        } else {
            false
        };
        if let Some(panel) = &mut self.panel {
            panel.note_count = count;
            panel.has_note_thumb = has_thumb;
        }
    }

    /// Update the side panel's cheat list (plan §4.4): `cheats` is
    /// `(description, on)` pairs in the curated order — each becomes its own
    /// clickable row (`PanelButton::CheatRow`), no cursor to move any more.
    /// Call once at game start and again on every toggle — the list is
    /// always tiny. A no-op before `set_panel`.
    pub fn set_cheats(&mut self, cheats: &[(String, bool)]) {
        if let Some(panel) = &mut self.panel {
            panel.cheats = cheats.to_vec();
        }
    }

    /// Update the session clock shown at the bottom of the panel. Call once a
    /// frame; it just stores the value for the next present/capture.
    pub fn set_session_time(&mut self, elapsed: Duration) {
        self.session = elapsed;
    }

    /// Load the pause book's content (plan §3.2/§3.4): call once when pause
    /// opens, not every frame — `present_pause` just redraws what's already
    /// set. `captures` is the fixed slot count (plan revision: always 15),
    /// `filled` how many actually hold an image. No thumb loaded yet —
    /// follow with `set_pause_page` to actually show one.
    pub fn set_pause_note(&mut self, title: &str, captures: usize, filled: usize) {
        self.pause = Some(PauseNote {
            title: title.to_string(),
            captures,
            filled,
            page: captures.saturating_sub(1),
            has_thumb: false,
            pinned: false,
            slot_label: String::new(),
            draft: None,
            draft_limit: 0,
            draft_heading: String::new(),
        });
    }

    /// Show a different capture on the right page (pagination, plan
    /// revision) — call on entering pause (page = the last one) and again on
    /// every Prev/Next/pin/rename change. `pinned`/`label` are that slot's
    /// current protection state and caption. A no-op before `set_pause_note`.
    pub fn set_pause_page(
        &mut self,
        page: usize,
        pinned: bool,
        label: &str,
        thumb: Option<(u32, u32, &[u8])>,
    ) {
        let has_thumb = if let Some((w, h, rgba)) = thumb {
            self.set_image(PAUSE_THUMB_IMG, w, h, rgba);
            true
        } else {
            false
        };
        if let Some(p) = &mut self.pause {
            p.page = page;
            p.has_thumb = has_thumb;
            p.pinned = pinned;
            p.slot_label = label.to_string();
        }
    }

    /// Enter/update/leave an editor on the left page (plan revision: either
    /// the free-text note or a slot's caption — `heading` names which,
    /// shown above the live text): `Some(text)` shows `text` in place of the
    /// status view, with a `.../limit` counter; `None` leaves editing (any
    /// `heading` passed alongside `None` is ignored). Call on every
    /// keystroke while editing — cheap, at most a few hundred characters.
    /// A no-op before `set_pause_note`.
    pub fn set_pause_draft(&mut self, draft: Option<&str>, limit: usize, heading: &str) {
        if let Some(p) = &mut self.pause {
            p.draft = draft.map(str::to_string);
            p.draft_limit = limit;
            p.draft_heading = heading.to_string();
        }
    }

    /// Draw the pause book to the window: two pages, not warped by the tube,
    /// replacing the whole window rather than sharing it with the cabinet
    /// (plan §3.2/§3.4), plus its own two clickable buttons ("Continuar",
    /// "Avancar quadro" — plan revision: mouse/gamepad only, no keyboard).
    pub fn present_pause(&mut self) {
        let (real_w, real_h) = self.canvas.output_size().unwrap_or((1280, 720));
        let rect = cabinet_canvas_rect(real_w, real_h);
        self.canvas_rect = rect;
        self.canvas
            .set_draw_color(Color::RGB(RECESS.0, RECESS.1, RECESS.2));
        self.canvas.clear();
        self.canvas.set_viewport(Some(rect));
        let cheats = self
            .panel
            .as_ref()
            .map(|p| p.cheats.as_slice())
            .unwrap_or(&[]);
        self.pause_buttons = draw_pause_book(
            &mut self.canvas,
            &mut self.font,
            &self.images,
            self.pause.as_ref(),
            cheats,
            rect.width(),
            rect.height(),
        );
        self.canvas.set_viewport(None);
        self.canvas.present();
    }

    /// Like [`Cabinet::present_pause`] but composited into an offscreen
    /// target and saved as a BMP (headless preview).
    pub fn capture_pause_bmp(&mut self, path: &std::path::Path) -> Result<(), PlatformError> {
        let (real_w, real_h) = self.canvas.output_size().unwrap_or((1280, 720));
        let rect = cabinet_canvas_rect(real_w, real_h);
        let mut target = self
            .canvas
            .create_texture_target(SdlFormat::RGBA32, real_w, real_h)
            .map_err(|e| PlatformError::Sdl(e.to_string()))?;
        let pause = self.pause.as_ref();
        let cheats = self
            .panel
            .as_ref()
            .map(|p| p.cheats.as_slice())
            .unwrap_or(&[]);
        let font = &mut self.font;
        let images = &self.images;
        let mut saved: Result<(), PlatformError> = Ok(());
        let outcome = self.canvas.with_texture_canvas(&mut target, |c| {
            c.set_draw_color(Color::RGB(RECESS.0, RECESS.1, RECESS.2));
            c.clear();
            c.set_viewport(Some(rect));
            let _ = draw_pause_book(c, font, images, pause, cheats, rect.width(), rect.height());
            c.set_viewport(None);
            saved = c
                .read_pixels(None::<Rect>)
                .and_then(|s| s.save_bmp(path))
                .map_err(|e| PlatformError::Sdl(e.to_string()));
        });
        outcome.map_err(|e| PlatformError::Sdl(e.to_string()))?;
        saved
    }

    /// Whether image `id` is already decoded and cached — e.g. to decide
    /// whether any cover art exists at all before choosing how to lay out
    /// the shelf (grid vs. list, plan §3.1). Same lookup `Screen::has_image`
    /// does from inside a `frame_2d` closure, exposed here for callers that
    /// need the answer *before* they can build that closure.
    pub fn has_image(&self, id: u64) -> bool {
        self.images.contains_key(&id)
    }

    /// Size of the recessed screen area — what the selector lays itself out in.
    pub fn screen_size(&self) -> (u32, u32) {
        let (real_w, real_h) = self.canvas.output_size().unwrap_or((1280, 720));
        let rect = cabinet_canvas_rect(real_w, real_h);
        let s = screen_area(rect.width(), rect.height());
        (s.width(), s.height())
    }

    // --- game path -------------------------------------------------------

    /// Draw one frame to the window. `aspect_ratio <= 0` means "use 4:3".
    pub fn present_frame(&mut self, frame: &FrameRef, aspect_ratio: f32) {
        self.ensure_src(frame.width, frame.height, frame.format);
        self.upload(frame);

        let (real_w, real_h) = self
            .canvas
            .output_size()
            .unwrap_or((frame.width, frame.height));
        let canvas_rect = cabinet_canvas_rect(real_w, real_h);
        self.canvas_rect = canvas_rect;
        let (out_w, out_h) = (canvas_rect.width(), canvas_rect.height());
        let panel = panel_rect(out_w, out_h);
        let cab_w = out_w.saturating_sub(panel.width());
        let screen = screen_area(cab_w, out_h);
        self.screen = screen;
        let dst = fit_aspect_in(screen, resolve_aspect(aspect_ratio));
        self.ensure_mesh(dst);
        self.ensure_bezel(cab_w, out_h, dst);

        self.canvas
            .set_draw_color(Color::RGB(RECESS.0, RECESS.1, RECESS.2));
        self.canvas.clear();
        self.canvas.set_viewport(Some(canvas_rect));

        let src = self.src.take().unwrap();
        let mesh = self.mesh.take().unwrap();
        let bezel = self.bezel.take().unwrap();
        let _ = self
            .canvas
            .render_geometry(&mesh.verts, Some(&src.tex), &mesh.indices[..]);
        let _ = self
            .canvas
            .render_geometry(&bezel.verts, None, &bezel.indices[..]);
        self.src = Some(src);
        self.mesh = Some(mesh);
        self.bezel = Some(bezel);

        draw_brand(&mut self.canvas, &mut self.font, self.screen, out_h);
        self.panel_buttons = draw_panel(
            &mut self.canvas,
            &mut self.font,
            &self.images,
            self.panel.as_ref(),
            panel,
            self.session,
        );
        self.canvas.set_viewport(None);
        self.canvas.present();
    }

    /// Render one frame into an offscreen target and save it as a BMP. Works
    /// headless (a background window never composites on macOS), so the real
    /// output — cabinet, CRT warp and all — can be eyeballed without a window.
    pub fn capture_bmp(
        &mut self,
        frame: &FrameRef,
        aspect_ratio: f32,
        path: &std::path::Path,
    ) -> Result<(), PlatformError> {
        self.ensure_src(frame.width, frame.height, frame.format);
        self.upload(frame);

        let (real_w, real_h) = self
            .canvas
            .output_size()
            .unwrap_or((frame.width, frame.height));
        let canvas_rect = cabinet_canvas_rect(real_w, real_h);
        self.canvas_rect = canvas_rect;
        let (out_w, out_h) = (canvas_rect.width(), canvas_rect.height());
        let panel = panel_rect(out_w, out_h);
        let cab_w = out_w.saturating_sub(panel.width());
        let screen = screen_area(cab_w, out_h);
        self.screen = screen;
        let dst = fit_aspect_in(screen, resolve_aspect(aspect_ratio));
        self.ensure_mesh(dst);
        self.ensure_bezel(cab_w, out_h, dst);

        let mut target = self
            .canvas
            .create_texture_target(SdlFormat::RGBA32, real_w, real_h)
            .map_err(|e| PlatformError::Sdl(e.to_string()))?;

        let src = self.src.take().unwrap();
        let mesh = self.mesh.take().unwrap();
        let bezel = self.bezel.take().unwrap();
        let panel_info = self.panel.as_ref();
        let session = self.session;
        let font = &mut self.font;
        let images = &self.images;
        let mut saved: Result<(), PlatformError> = Ok(());
        let outcome = self.canvas.with_texture_canvas(&mut target, |c| {
            c.set_draw_color(Color::RGB(RECESS.0, RECESS.1, RECESS.2));
            c.clear();
            c.set_viewport(Some(canvas_rect));
            let _ = c.render_geometry(&mesh.verts, Some(&src.tex), &mesh.indices[..]);
            let _ = c.render_geometry(&bezel.verts, None, &bezel.indices[..]);
            draw_brand(c, font, screen, out_h);
            draw_panel(c, font, images, panel_info, panel, session);
            c.set_viewport(None);
            saved = c
                .read_pixels(None::<Rect>)
                .and_then(|s| s.save_bmp(path))
                .map_err(|e| PlatformError::Sdl(e.to_string()));
        });
        self.src = Some(src);
        self.mesh = Some(mesh);
        self.bezel = Some(bezel);
        outcome.map_err(|e| PlatformError::Sdl(e.to_string()))?;
        saved
    }

    fn upload(&mut self, frame: &FrameRef) {
        let expected_pitch = frame.width as usize * frame.format.bytes_per_pixel();
        let src = self.src.as_mut().unwrap();
        let pitch = if frame.pitch == 0 {
            expected_pitch
        } else {
            frame.pitch
        };
        src.tex
            .update(None, frame.pixels, pitch)
            .expect("upload frame");
    }

    fn ensure_src(&mut self, w: u32, h: u32, format: PixelFormat) {
        let stale = match &self.src {
            Some(s) => s.w != w || s.h != h || s.format != format,
            None => true,
        };
        if stale {
            let mut tex = self
                .canvas
                .create_texture_streaming(format.sdl(), w, h)
                .expect("create streaming texture");
            tex.set_scale_mode(SdlScaleMode::Linear);
            self.src = Some(SrcTexture { tex, w, h, format });
        }
    }

    fn ensure_mesh(&mut self, dst: Rect) {
        let (w, h) = (dst.width(), dst.height());
        if matches!(&self.mesh, Some(m) if m.w == w && m.h == h) {
            return;
        }
        self.mesh = Some(build_crt_mesh(dst, 1.0));
    }

    fn ensure_bezel(&mut self, out_w: u32, out_h: u32, screen: Rect) {
        let key = (out_w, out_h, screen.width(), screen.height());
        if matches!(&self.bezel, Some(b) if b.key == key) {
            return;
        }
        self.bezel = Some(build_bezel_mesh(out_w, out_h, screen, key));
    }

    // --- 2D path (selector) --------------------------------------------
    //
    // The selector draws flat 2D into an offscreen buffer the size of the tube
    // opening, which is then composited through the CRT mesh just like a game
    // frame — so the shelf bulges with the same tube.

    /// Register/replace an image from tightly-packed RGBA8 (call between frames).
    pub fn set_image(&mut self, id: u64, w: u32, h: u32, rgba: &[u8]) {
        if w == 0 || h == 0 || rgba.len() < (w * h * 4) as usize {
            return;
        }
        let mut tex = match self.canvas.create_texture_static(SdlFormat::RGBA32, w, h) {
            Ok(t) => t,
            Err(e) => {
                log::warn!("cabinet: texture {w}x{h}: {e}");
                return;
            }
        };
        if tex.update(None, rgba, (w * 4) as usize).is_err() {
            return;
        }
        tex.set_blend_mode(BlendMode::Blend);
        tex.set_scale_mode(SdlScaleMode::Linear);
        self.images.insert(id, ImgTex { tex, w, h });
    }

    /// Draw a 2D frame: `draw` renders into a screen-sized buffer (coords
    /// 0..screen), which is then warped through the tube, framed and presented.
    pub fn frame_2d<F: FnOnce(&mut Screen)>(&mut self, bg: (u8, u8, u8), draw: F) {
        self.paint_2d(bg, draw);
        self.composite_screen();
        self.canvas.present();
    }

    /// Like [`Cabinet::frame_2d`] but composited into an offscreen target and
    /// saved as a BMP (headless — a background window never composites on macOS).
    pub fn capture_2d<F: FnOnce(&mut Screen)>(
        &mut self,
        bg: (u8, u8, u8),
        draw: F,
        path: &std::path::Path,
    ) -> Result<(), PlatformError> {
        self.paint_2d(bg, draw);

        let (ww, wh) = self.canvas.output_size().unwrap_or((1280, 720));
        let canvas_rect = self.canvas_rect;
        let screen = self.screen;
        let mesh = build_crt_mesh(screen, 1.0);
        let mut target = self
            .canvas
            .create_texture_target(SdlFormat::RGBA32, ww, wh)
            .map_err(|e| PlatformError::Sdl(e.to_string()))?;
        let st = self.screen_tex.take().unwrap();
        let bezel = self.bezel.take().unwrap();
        let font = &mut self.font;
        let mut saved: Result<(), PlatformError> = Ok(());
        let outcome = self.canvas.with_texture_canvas(&mut target, |c| {
            c.set_draw_color(Color::RGB(RECESS.0, RECESS.1, RECESS.2));
            c.clear();
            c.set_viewport(Some(canvas_rect));
            let _ = c.render_geometry(&mesh.verts, Some(&st.tex), &mesh.indices[..]);
            let _ = c.render_geometry(&bezel.verts, None, &bezel.indices[..]);
            draw_brand(c, font, screen, canvas_rect.height());
            c.set_viewport(None);
            saved = c
                .read_pixels(None::<Rect>)
                .and_then(|s| s.save_bmp(path))
                .map_err(|e| PlatformError::Sdl(e.to_string()));
        });
        self.screen_tex = Some(st);
        self.bezel = Some(bezel);
        outcome.map_err(|e| PlatformError::Sdl(e.to_string()))?;
        saved
    }

    /// One frame of signal-off snow through the tube, cartridge still visible
    /// in its slot if one is set. `level` 1.0 = a full blizzard, 0.0 = a dim,
    /// near-still hiss. Never a full-screen flash.
    pub fn present_static(&mut self, level: f32) {
        let (real_w, real_h) = self.canvas.output_size().unwrap_or((1280, 720));
        let canvas_rect = cabinet_canvas_rect(real_w, real_h);
        self.canvas_rect = canvas_rect;
        let (ww, wh) = (canvas_rect.width(), canvas_rect.height());
        let panel = panel_rect(ww, wh);
        let cab_w = ww.saturating_sub(panel.width());
        self.screen = screen_area(cab_w, wh);
        self.ensure_bezel(cab_w, wh, self.screen);
        self.update_noise_tex(level);

        let mesh = build_crt_mesh(self.screen, 1.0);
        self.canvas
            .set_draw_color(Color::RGB(RECESS.0, RECESS.1, RECESS.2));
        self.canvas.clear();
        self.canvas.set_viewport(Some(canvas_rect));
        let nt = self.noise_tex.take().unwrap();
        let _ = self
            .canvas
            .render_geometry(&mesh.verts, Some(&nt.tex), &mesh.indices[..]);
        self.noise_tex = Some(nt);
        let bezel = self.bezel.take().unwrap();
        let _ = self
            .canvas
            .render_geometry(&bezel.verts, None, &bezel.indices[..]);
        self.bezel = Some(bezel);
        draw_brand(&mut self.canvas, &mut self.font, self.screen, wh);
        self.panel_buttons = draw_panel(
            &mut self.canvas,
            &mut self.font,
            &self.images,
            self.panel.as_ref(),
            panel,
            self.session,
        );
        self.canvas.set_viewport(None);
        self.canvas.present();
    }

    /// Like [`Cabinet::present_static`] but composited into an offscreen
    /// target and saved as a BMP (headless — eyeball the power-off / idle-off
    /// screen, cartridge and all, without a window).
    pub fn capture_static_bmp(
        &mut self,
        level: f32,
        path: &std::path::Path,
    ) -> Result<(), PlatformError> {
        let (real_w, real_h) = self.canvas.output_size().unwrap_or((1280, 720));
        let canvas_rect = cabinet_canvas_rect(real_w, real_h);
        self.canvas_rect = canvas_rect;
        let (ww, wh) = (canvas_rect.width(), canvas_rect.height());
        let panel = panel_rect(ww, wh);
        let cab_w = ww.saturating_sub(panel.width());
        self.screen = screen_area(cab_w, wh);
        self.ensure_bezel(cab_w, wh, self.screen);
        self.update_noise_tex(level);
        let screen = self.screen;

        let mesh = build_crt_mesh(screen, 1.0);
        let mut target = self
            .canvas
            .create_texture_target(SdlFormat::RGBA32, real_w, real_h)
            .map_err(|e| PlatformError::Sdl(e.to_string()))?;
        let nt = self.noise_tex.take().unwrap();
        let bezel = self.bezel.take().unwrap();
        let panel_info = self.panel.as_ref();
        let session = self.session;
        let font = &mut self.font;
        let images = &self.images;
        let mut saved: Result<(), PlatformError> = Ok(());
        let outcome = self.canvas.with_texture_canvas(&mut target, |c| {
            c.set_draw_color(Color::RGB(RECESS.0, RECESS.1, RECESS.2));
            c.clear();
            c.set_viewport(Some(canvas_rect));
            let _ = c.render_geometry(&mesh.verts, Some(&nt.tex), &mesh.indices[..]);
            let _ = c.render_geometry(&bezel.verts, None, &bezel.indices[..]);
            draw_brand(c, font, screen, wh);
            draw_panel(c, font, images, panel_info, panel, session);
            c.set_viewport(None);
            saved = c
                .read_pixels(None::<Rect>)
                .and_then(|s| s.save_bmp(path))
                .map_err(|e| PlatformError::Sdl(e.to_string()));
        });
        self.noise_tex = Some(nt);
        self.bezel = Some(bezel);
        outcome.map_err(|e| PlatformError::Sdl(e.to_string()))?;
        saved
    }

    /// Like [`Cabinet::frame_2d`], but blended up from residual signal-off snow
    /// instead of cutting in cold: `static_level` is the snow still showing
    /// behind it, `shelf_alpha` (0..1) how much of the drawn frame shows on top
    /// (plan §3.3, "a estante entra por cima"). Call with `shelf_alpha` ramping
    /// 0.0 -> 1.0 over the first handful of frames after a game closes.
    pub fn frame_2d_fade_in<F: FnOnce(&mut Screen)>(
        &mut self,
        bg: (u8, u8, u8),
        draw: F,
        static_level: f32,
        shelf_alpha: f32,
    ) {
        self.paint_2d(bg, draw);
        self.update_noise_tex(static_level);
        let canvas_rect = self.canvas_rect;

        let mesh_static = build_crt_mesh(self.screen, 1.0);
        let mesh_shelf = build_crt_mesh(self.screen, shelf_alpha.clamp(0.0, 1.0));

        self.canvas
            .set_draw_color(Color::RGB(RECESS.0, RECESS.1, RECESS.2));
        self.canvas.clear();
        self.canvas.set_viewport(Some(canvas_rect));

        let nt = self.noise_tex.take().unwrap();
        let _ = self.canvas.render_geometry(
            &mesh_static.verts,
            Some(&nt.tex),
            &mesh_static.indices[..],
        );
        self.noise_tex = Some(nt);

        let st = self.screen_tex.take().unwrap();
        let _ =
            self.canvas
                .render_geometry(&mesh_shelf.verts, Some(&st.tex), &mesh_shelf.indices[..]);
        self.screen_tex = Some(st);

        let bezel = self.bezel.take().unwrap();
        let _ = self
            .canvas
            .render_geometry(&bezel.verts, None, &bezel.indices[..]);
        self.bezel = Some(bezel);
        draw_brand(
            &mut self.canvas,
            &mut self.font,
            self.screen,
            canvas_rect.height(),
        );

        self.canvas.set_viewport(None);
        self.canvas.present();
    }

    /// Fill the noise texture for the current window size at `level` (1.0 =
    /// full blizzard, 0.0 = a dim near-still hiss); leaves it in `noise_tex`.
    fn update_noise_tex(&mut self, level: f32) {
        const NW: u32 = 320;
        const NH: u32 = 240;
        self.ensure_noise_tex(NW, NH);
        let k = level.clamp(0.0, 1.0);
        let hi = (26.0 + 150.0 * k) as u32; // cap well under white
        let lo = (6.0 * k) as u32;
        let span = hi - lo + 1;
        {
            let Self { noise, rng, .. } = &mut *self;
            if noise.len() != (NW * NH * 4) as usize {
                *noise = vec![0u8; (NW * NH * 4) as usize];
            }
            for px in noise.as_chunks_mut::<4>().0 {
                *rng ^= *rng << 13;
                *rng ^= *rng >> 17;
                *rng ^= *rng << 5;
                let v = (lo + *rng % span) as u8;
                *px = [v, v, v, 255];
            }
        }
        let nt = self.noise_tex.as_mut().unwrap();
        let _ = nt.tex.update(None, &self.noise, (NW * 4) as usize);
    }

    /// Render `draw` into the screen buffer. Shared by `frame_2d` / `capture_2d`.
    fn paint_2d<F: FnOnce(&mut Screen)>(&mut self, bg: (u8, u8, u8), draw: F) {
        let (real_w, real_h) = self.canvas.output_size().unwrap_or((1280, 720));
        self.canvas_rect = cabinet_canvas_rect(real_w, real_h);
        let (ww, wh) = (self.canvas_rect.width(), self.canvas_rect.height());
        self.screen = screen_area(ww, wh);
        let (sw, sh) = (self.screen.width(), self.screen.height());
        self.ensure_screen_tex(sw, sh);
        self.ensure_bezel(ww, wh, self.screen);

        let Self {
            canvas,
            screen_tex,
            font,
            images,
            ..
        } = self;
        let images = &*images;
        let st = screen_tex.as_mut().unwrap();
        let _ = canvas.with_texture_canvas(&mut st.tex, |c| {
            c.set_draw_color(Color::RGB(bg.0, bg.1, bg.2));
            c.clear();
            let mut s = Screen {
                canvas: c,
                font,
                images,
                w: sw,
                h: sh,
            };
            draw(&mut s);
        });
        // The clip lives on the shared underlying renderer; clear it.
        self.canvas.set_clip_rect(ClippingRect::None);
    }

    /// Warp the screen buffer through the tube into the live window, then frame.
    fn composite_screen(&mut self) {
        let mesh = build_crt_mesh(self.screen, 1.0);
        self.canvas
            .set_draw_color(Color::RGB(RECESS.0, RECESS.1, RECESS.2));
        self.canvas.clear();
        self.canvas.set_viewport(Some(self.canvas_rect));
        let st = self.screen_tex.take().unwrap();
        let _ = self
            .canvas
            .render_geometry(&mesh.verts, Some(&st.tex), &mesh.indices[..]);
        self.screen_tex = Some(st);
        let bezel = self.bezel.take().unwrap();
        let _ = self
            .canvas
            .render_geometry(&bezel.verts, None, &bezel.indices[..]);
        self.bezel = Some(bezel);
        draw_brand(
            &mut self.canvas,
            &mut self.font,
            self.screen,
            self.canvas_rect.height(),
        );
        self.canvas.set_viewport(None);
    }

    fn ensure_screen_tex(&mut self, w: u32, h: u32) {
        if matches!(&self.screen_tex, Some(s) if s.w == w && s.h == h) {
            return;
        }
        let mut tex = self
            .canvas
            .create_texture_target(SdlFormat::RGBA32, w, h)
            .expect("create screen target");
        tex.set_scale_mode(SdlScaleMode::Linear);
        // So `frame_2d_fade_in`'s vertex alpha can blend it over the snow.
        tex.set_blend_mode(BlendMode::Blend);
        self.screen_tex = Some(SizedTex { tex, w, h });
    }

    fn ensure_noise_tex(&mut self, w: u32, h: u32) {
        if matches!(&self.noise_tex, Some(s) if s.w == w && s.h == h) {
            return;
        }
        let mut tex = self
            .canvas
            .create_texture_streaming(SdlFormat::RGBA32, w, h)
            .expect("create noise texture");
        tex.set_scale_mode(SdlScaleMode::Linear);
        self.noise_tex = Some(SizedTex { tex, w, h });
    }
}

/// A 2D drawing surface, screen-local coordinates (0,0 = top-left of the tube
/// opening). Handed to the `frame_2d` / `capture_2d` closure.
pub struct Screen<'a> {
    canvas: &'a mut WindowCanvas,
    font: &'a mut Texture,
    images: &'a HashMap<u64, ImgTex>,
    w: u32,
    h: u32,
}

impl Screen<'_> {
    /// Size of the drawing surface.
    pub fn size(&self) -> (u32, u32) {
        (self.w, self.h)
    }

    pub fn fill(&mut self, x: i32, y: i32, w: u32, h: u32, c: (u8, u8, u8, u8)) {
        self.canvas.set_draw_color(Color::RGBA(c.0, c.1, c.2, c.3));
        let _ = self.canvas.fill_rect(Rect::new(x, y, w, h));
    }

    pub fn outline(&mut self, x: i32, y: i32, w: u32, h: u32, thick: u32, c: (u8, u8, u8, u8)) {
        let t = thick as i32;
        self.fill(x, y, w, thick, c);
        self.fill(x, y + h as i32 - t, w, thick, c);
        self.fill(x, y, thick, h, c);
        self.fill(x + w as i32 - t, y, thick, h, c);
    }

    /// Draw `s` at `(x, y)`, `scale`x the glyph cell. Returns the advance width.
    pub fn text(&mut self, x: i32, y: i32, scale: u32, c: (u8, u8, u8), s: &str) -> i32 {
        self.font.set_color_mod(c.0, c.1, c.2);
        let cell = (GLYPH_W * scale) as i32;
        let mut pen = x;
        for ch in s.chars() {
            let idx = glyph_index(ch);
            if ch != ' ' {
                let src = Rect::new(idx as i32 * GLYPH_W as i32, 0, GLYPH_W, GLYPH_H);
                let dst = Rect::new(pen, y, GLYPH_W * scale, GLYPH_H * scale);
                let _ = self.canvas.copy(self.font, src, dst);
            }
            pen += cell;
        }
        pen - x
    }

    /// Word-wrap `s` into `max_w`, returning the y past the last line.
    pub fn text_wrapped(
        &mut self,
        x: i32,
        y: i32,
        max_w: u32,
        scale: u32,
        c: (u8, u8, u8),
        s: &str,
    ) -> i32 {
        let cols = (max_w / (GLYPH_W * scale)).max(1) as usize;
        let row = (GLYPH_H * scale) as i32;
        let mut line = String::new();
        let mut cy = y;
        for word in s.split_whitespace() {
            if !line.is_empty() && line.len() + 1 + word.len() > cols {
                self.text(x, cy, scale, c, &line);
                cy += row + 2;
                line.clear();
            }
            if !line.is_empty() {
                line.push(' ');
            }
            line.push_str(word);
            while line.len() > cols {
                let (head, tail) = line.split_at(cols);
                self.text(x, cy, scale, c, head);
                cy += row + 2;
                line = tail.to_string();
            }
        }
        if !line.is_empty() {
            self.text(x, cy, scale, c, &line);
            cy += row + 2;
        }
        cy
    }

    /// Height [`Screen::text_wrapped`] would take for `s`, without drawing.
    pub fn wrapped_height(&self, max_w: u32, scale: u32, s: &str) -> i32 {
        wrapped_height(max_w, scale, s)
    }

    /// Clip drawing to `rect`; `None` clears the clip.
    pub fn clip(&mut self, rect: Option<(i32, i32, u32, u32)>) {
        self.canvas.set_clip_rect(match rect {
            Some((x, y, w, h)) => ClippingRect::Some(Rect::new(x, y, w, h)),
            None => ClippingRect::None,
        });
    }

    pub fn has_image(&self, id: u64) -> bool {
        self.images.contains_key(&id)
    }

    /// Draw image `id` letterboxed inside the box, centered. No-op if unknown.
    pub fn image_fit(&mut self, id: u64, x: i32, y: i32, bw: u32, bh: u32) {
        let Some(img) = self.images.get(&id) else {
            return;
        };
        let (iw, ih) = (img.w as f32, img.h as f32);
        let scale = (bw as f32 / iw).min(bh as f32 / ih);
        let dw = (iw * scale).round() as i32;
        let dh = (ih * scale).round() as i32;
        let dx = x + (bw as i32 - dw) / 2;
        let dy = y + (bh as i32 - dh) / 2;
        let _ = self.canvas.copy(
            &img.tex,
            None::<sdl3::render::FRect>,
            Rect::new(dx, dy, dw.max(1) as u32, dh.max(1) as u32),
        );
    }
}

fn resolve_aspect(a: f32) -> f32 {
    if a > 0.0 {
        a
    } else {
        4.0 / 3.0
    }
}

fn centered_in(area: Rect, w: u32, h: u32) -> Rect {
    let x = area.x() + (area.width() as i32 - w as i32) / 2;
    let y = area.y() + (area.height() as i32 - h as i32) / 2;
    Rect::new(x, y, w, h)
}

/// Largest `aspect`-shaped rect that fits inside `area`, centered in it. Fills
/// the height unless that would overflow the width, then fills the width.
fn fit_aspect_in(area: Rect, aspect: f32) -> Rect {
    let mut h = area.height();
    let mut w = (h as f32 * aspect).round() as u32;
    if w > area.width() {
        w = area.width();
        h = (w as f32 / aspect).round() as u32;
    }
    centered_in(area, w, h)
}

/// Where the cabinet actually draws within the real window/display: the
/// largest 16:9 rect that fits, centered — a modern-TV shape, regardless of
/// the window's own. `screen_area`/`panel_rect` (and everything downstream)
/// only ever see this rect's width/height, never the raw output size, so an
/// ultrawide monitor gets letterbox bars on the sides instead of a
/// stretched-wide tube.
fn cabinet_canvas_rect(out_w: u32, out_h: u32) -> Rect {
    fit_aspect_in(Rect::new(0, 0, out_w, out_h), 16.0 / 9.0)
}

/// The cabinet opening: the window inset by the bezel fractions (a wider chin).
fn screen_area(out_w: u32, out_h: u32) -> Rect {
    let sx = (out_w as f32 * BEZEL_SIDE).round() as i32;
    let ty = (out_h as f32 * BEZEL_TOP).round() as i32;
    let by = (out_h as f32 * BEZEL_CHIN).round() as i32;
    let w = (out_w as i32 - 2 * sx).max(16) as u32;
    let h = (out_h as i32 - ty - by).max(16) as u32;
    Rect::new(sx, ty, w, h)
}

/// A four-quad ring from the window edge (cabinet colour) to the recessed
/// screen edge (near-black), so the screen sits in a shadowed well.
fn build_bezel_mesh(out_w: u32, out_h: u32, screen: Rect, key: (u32, u32, u32, u32)) -> BezelMesh {
    let norm = |c: (u8, u8, u8)| {
        FColor::RGBA(
            c.0 as f32 / 255.0,
            c.1 as f32 / 255.0,
            c.2 as f32 / 255.0,
            1.0,
        )
    };
    let (cab, rec) = (norm(CABINET), norm(RECESS));
    let z = sdl3::render::FPoint::new(0.0, 0.0);
    let vtx = |x: i32, y: i32, c: FColor| Vertex {
        position: sdl3::render::FPoint::new(x as f32, y as f32),
        color: c,
        tex_coord: z,
    };

    let (or, ob) = (out_w as i32, out_h as i32);
    let (il, it, ir, ib) = (screen.left(), screen.top(), screen.right(), screen.bottom());

    let verts = vec![
        vtx(0, 0, cab),
        vtx(or, 0, cab),
        vtx(or, ob, cab),
        vtx(0, ob, cab), // 0..3 outer
        vtx(il, it, rec),
        vtx(ir, it, rec),
        vtx(ir, ib, rec),
        vtx(il, ib, rec), // 4..7 inner
    ];
    #[rustfmt::skip]
    let indices = vec![
        0, 1, 5, 0, 5, 4, // top
        1, 2, 6, 1, 6, 5, // right
        2, 3, 7, 2, 7, 6, // bottom
        3, 0, 4, 3, 4, 7, // left
    ];
    BezelMesh {
        verts,
        indices,
        key,
    }
}

/// The set's nameplate, printed into the chin left of the tube — part of the
/// cabinet itself, so unlike the panel it's drawn in every context (shelf,
/// game, idle-off) and never disappears. `None` if the chin is too short to
/// hold it.
fn draw_brand(canvas: &mut WindowCanvas, font: &mut Texture, screen: Rect, out_h: u32) {
    let chin_top = screen.bottom();
    let chin_h = out_h as i32 - chin_top;
    if chin_h < 24 {
        return;
    }
    let y = chin_top + (chin_h - GLYPH_H as i32) / 2;
    draw_text_absolute(
        canvas,
        font,
        screen.left(),
        y,
        TextStyle::new(1, BRAND_TEXT),
        BRAND,
        usize::MAX,
    );
}

/// Like `Screen::image_fit`, but at absolute window coordinates instead of
/// offset into the 2D screen buffer — for cabinet furniture and panel art.
fn draw_image_absolute(
    canvas: &mut WindowCanvas,
    images: &HashMap<u64, ImgTex>,
    id: u64,
    x: i32,
    y: i32,
    bw: u32,
    bh: u32,
) {
    let Some(img) = images.get(&id) else {
        return;
    };
    let (iw, ih) = (img.w as f32, img.h as f32);
    let scale = (bw as f32 / iw).min(bh as f32 / ih);
    let dw = (iw * scale).round() as i32;
    let dh = (ih * scale).round() as i32;
    let dx = x + (bw as i32 - dw) / 2;
    let dy = y + (bh as i32 - dh) / 2;
    let _ = canvas.copy(
        &img.tex,
        None::<sdl3::render::FRect>,
        Rect::new(dx, dy, dw.max(1) as u32, dh.max(1) as u32),
    );
}

/// Scale + colour for one of the absolute-coordinate text helpers below —
/// bundled so those functions stay under clippy's argument-count limit.
#[derive(Clone, Copy)]
struct TextStyle {
    scale: u32,
    color: (u8, u8, u8),
}

impl TextStyle {
    fn new(scale: u32, color: (u8, u8, u8)) -> Self {
        Self { scale, color }
    }
}

/// A single line of text at absolute window coordinates, truncated to
/// `max_chars` (`usize::MAX` for no truncation) — for cabinet furniture that
/// isn't inside a `Screen`.
fn draw_text_absolute(
    canvas: &mut WindowCanvas,
    font: &mut Texture,
    x: i32,
    y: i32,
    style: TextStyle,
    s: &str,
    max_chars: usize,
) {
    let (r, g, b) = style.color;
    font.set_color_mod(r, g, b);
    let cell = (GLYPH_W * style.scale) as i32;
    let mut pen = x;
    for ch in s.chars().take(max_chars) {
        let idx = glyph_index(ch);
        if ch != ' ' {
            let src = Rect::new(idx as i32 * GLYPH_W as i32, 0, GLYPH_W, GLYPH_H);
            let dst = Rect::new(pen, y, GLYPH_W * style.scale, GLYPH_H * style.scale);
            let _ = canvas.copy(font, src, dst);
        }
        pen += cell;
    }
}

/// Word-wrapped text at absolute window coordinates, mirroring
/// `Screen::text_wrapped`'s layout. Returns the y past the last line.
fn draw_text_wrapped_absolute(
    canvas: &mut WindowCanvas,
    font: &mut Texture,
    x: i32,
    y: i32,
    max_w: u32,
    style: TextStyle,
    s: &str,
) -> i32 {
    let cols = (max_w / (GLYPH_W * style.scale)).max(1) as usize;
    let row = (GLYPH_H * style.scale) as i32;
    let mut line = String::new();
    let mut cy = y;
    for word in s.split_whitespace() {
        if !line.is_empty() && line.len() + 1 + word.len() > cols {
            draw_text_absolute(canvas, font, x, cy, style, &line, usize::MAX);
            cy += row + 2;
            line.clear();
        }
        if !line.is_empty() {
            line.push(' ');
        }
        line.push_str(word);
        while line.len() > cols {
            let (head, tail) = line.split_at(cols);
            draw_text_absolute(canvas, font, x, cy, style, head, usize::MAX);
            cy += row + 2;
            line = tail.to_string();
        }
    }
    if !line.is_empty() {
        draw_text_absolute(canvas, font, x, cy, style, &line, usize::MAX);
        cy += row + 2;
    }
    cy
}

/// Where the side panel sits: a column on the right, `PANEL_FRAC` of the
/// window, clamped so it neither collapses nor swallows a small window.
fn panel_rect(out_w: u32, out_h: u32) -> Rect {
    let w = ((out_w as f32) * PANEL_FRAC)
        .round()
        .clamp(PANEL_MIN as f32, PANEL_MAX as f32) as u32;
    let w = w.min(out_w.saturating_sub(64));
    Rect::new((out_w - w) as i32, 0, w, out_h)
}

/// Draw the side panel: background, logo (or title) at top, session timer at
/// the bottom. Plain widgets, not warped by the tube (plan §2, §3.2). `None`
/// (no game loaded — the idle/root screen) draws just the "Inserir cartucho"
/// button in the logo's spot. Returns the clickable buttons drawn this frame,
/// in `rect`'s (output/canvas) coordinate space — the caller stores them for
/// `Cabinet::hit_panel_button`.
fn draw_panel(
    canvas: &mut WindowCanvas,
    font: &mut Texture,
    images: &HashMap<u64, ImgTex>,
    panel: Option<&PanelInfo>,
    rect: Rect,
    session: Duration,
) -> Vec<(PanelButton, Rect)> {
    if rect.width() == 0 {
        return Vec::new();
    }
    canvas.set_draw_color(Color::RGB(PANEL_BG.0, PANEL_BG.1, PANEL_BG.2));
    let _ = canvas.fill_rect(rect);

    let pad = 20i32;
    let inner_w = rect.width().saturating_sub(pad as u32 * 2);
    let x = rect.x() + pad;
    let y = rect.y() + pad;

    let Some(panel) = panel else {
        // Idle/root screen == the "cartridge ejected" screen (plan revision:
        // one screen, not two — startup, backing out of the shelf, and
        // ejecting a game all land here). Same two slots a loaded game uses
        // (logo, then cartridge art) with idle-appropriate stand-ins: the
        // console's own brand logo where a game's logo would sit, "Inserir
        // cartucho" where its cartridge art would sit. No command legend —
        // there's nothing loaded to command — and Configuracoes moves to the
        // footer, the same spot the session clock uses during play.
        let mut cy = if images.contains_key(&PANEL_CONSOLE_LOGO_IMG) {
            draw_image_absolute(canvas, images, PANEL_CONSOLE_LOGO_IMG, x, y, inner_w, 110);
            y + 110
        } else {
            draw_text_wrapped_absolute(
                canvas,
                font,
                x,
                y,
                inner_w,
                TextStyle::new(2, PANEL_TEXT),
                BRAND,
            )
        };
        cy += 8;
        const INSERT_H: u32 = 150;
        let insert = Rect::new(x, cy, inner_w, INSERT_H);
        let insert_drawn = draw_button(canvas, font, insert, "Inserir cartucho", true);

        let btn_h = (GLYPH_H + 12) as i32;
        let settings = Rect::new(x, rect.bottom() - pad - btn_h, inner_w, btn_h as u32);
        return vec![
            (PanelButton::Insert, insert_drawn),
            (
                PanelButton::Settings,
                draw_button(canvas, font, settings, "Configuracoes", true),
            ),
        ];
    };

    // 1. Logo, or the title if there isn't one (plan §3.2, item 1).
    let mut cy = if panel.has_logo {
        draw_image_absolute(canvas, images, PANEL_LOGO_IMG, x, y, inner_w, 110);
        y + 110
    } else {
        draw_text_wrapped_absolute(
            canvas,
            font,
            x,
            y,
            inner_w,
            TextStyle::new(2, PANEL_TEXT),
            &panel.title,
        )
    };

    // 1b. Cartridge art (plan revision) — independent of the logo, drawn
    // right below whichever of the two just ran.
    if panel.has_cartridge {
        cy += 8;
        const CARTRIDGE_H: u32 = 150;
        draw_image_absolute(
            canvas,
            images,
            PANEL_CARTRIDGE_IMG,
            x,
            cy,
            inner_w,
            CARTRIDGE_H,
        );
        cy += CARTRIDGE_H as i32;
    }

    // Below this y, stop — leave the session clock's own band (item 6)
    // clear. There are a lot more command rows than there used to be (plan
    // revision added six), so a long title plus a game with several curated
    // cheats can genuinely run out of room; a clean cutoff beats spilling
    // into the clock. No scrolling yet (a real gap, not silently accepted —
    // see `docs/fase-4.md`'s revision note).
    let limit = rect.bottom() - pad - (GLYPH_H as i32 + 12);

    // 2. Power / Eject / Reset (plan revision): styled after the real
    // console's own controls instead of three more text rows — Power and
    // Reset are rocker switches (see `draw_rocker`), Eject sits between
    // them the way the cartridge-slot label does on the actual hardware.
    // Pulled out of the generic command loop below (which skips these three
    // by kind) so they get this dedicated look instead of a plain button.
    const SWITCH_TRACK_H: i32 = 64;
    const SWITCH_GROUP_H: i32 = SWITCH_TRACK_H + 4 + GLYPH_H as i32;
    let mut buttons = Vec::new();
    if cy + SWITCH_GROUP_H <= limit {
        cy += 14;
        let gap = 10i32;
        // Eject gets first claim on width (its "EJETAR" label doesn't
        // shrink), the two switches split whatever's left evenly — on a
        // narrow panel that leaves them tighter, not the label overflowing
        // its box.
        let eject_w = (GLYPH_W as i32) * "EJETAR".len() as i32 + 16;
        let switch_w = ((inner_w as i32 - gap * 2 - eject_w) / 2).max(1);
        let eject_w = (inner_w as i32 - gap * 2 - switch_w * 2).max(eject_w);
        let power_track = Rect::new(x, cy, switch_w as u32, SWITCH_TRACK_H as u32);
        let eject_rect = Rect::new(
            x + switch_w + gap,
            cy + SWITCH_TRACK_H - (GLYPH_H as i32 + 6),
            eject_w as u32,
            GLYPH_H + 6,
        );
        let reset_track = Rect::new(
            x + switch_w + gap + eject_w + gap,
            cy,
            switch_w as u32,
            SWITCH_TRACK_H as u32,
        );
        buttons.push((
            PanelButton::Power,
            draw_rocker(canvas, font, power_track, "POWER", panel.powered, true),
        ));
        buttons.push((
            PanelButton::Eject,
            draw_button(canvas, font, eject_rect, "EJETAR", !panel.powered),
        ));
        buttons.push((
            PanelButton::Reset,
            draw_rocker(
                canvas,
                font,
                reset_track,
                "RESET",
                panel.reset_pressed,
                panel.powered,
            ),
        ));
        cy += SWITCH_TRACK_H + 4 + GLYPH_H as i32;
    }

    // 3. Commands — the console's own buttons, not the emulator's extras
    // (plan §3.2, item 3), every one of them clickable (plan revision:
    // mouse/gamepad only, no keyboard legend any more). Everything here
    // only acts while powered — Power/Eject/Reset are drawn separately
    // above (item 2) with their own dim rules, so they're filtered out of
    // this list rather than drawn twice.
    let other_commands: Vec<&(PanelButton, String)> = panel
        .commands
        .iter()
        .filter(|(k, _)| {
            !matches!(
                k,
                PanelButton::Power | PanelButton::Eject | PanelButton::Reset
            )
        })
        .collect();
    if !other_commands.is_empty() && cy < limit {
        cy += 16;
        draw_text_absolute(
            canvas,
            font,
            x,
            cy,
            TextStyle::new(1, PANEL_DIM),
            "comandos",
            usize::MAX,
        );
        cy += GLYPH_H as i32 + 6;
    }
    for (kind, label) in other_commands {
        // Check the row's projected bottom, not just its top — otherwise
        // the last row drawn can still poke into the session clock's band.
        if cy + 4 + (GLYPH_H + 6) as i32 > limit {
            break;
        }
        cy += 4;
        // Tighter than a lone idle-screen button (plan revision: nine of
        // these stack now, not three — every extra pixel per row adds up).
        let btn = Rect::new(x, cy, inner_w, GLYPH_H + 6);
        let drawn = draw_button(canvas, font, btn, label, panel.powered);
        cy = drawn.bottom();
        buttons.push((*kind, drawn));
    }

    // 4. Cheats — informational only here now (plan revision): plain text,
    // only the ones actually on, no click (toggling moved to the pause book,
    // `draw_pause_book`, which has the room and isn't racing the clock for
    // space). Absent entirely with none on, not an empty "cheats" label.
    let cheats_on: Vec<&str> = panel
        .cheats
        .iter()
        .filter(|(_, on)| *on)
        .map(|(desc, _)| desc.as_str())
        .collect();
    // Room for the header plus at least one line — otherwise skip the whole
    // section instead of showing a "cheats" label with nothing under it.
    // The header hint can wrap to 2 lines on a narrow panel — reserve for
    // that plus one row's worth, rather than assuming a single line.
    let cheats_fit = cy + 16 + (GLYPH_H as i32 + 2) * 2 + GLYPH_H as i32 <= limit;
    if !cheats_on.is_empty() && cheats_fit {
        cy += 16;
        // "Pausar pra editar" is the discoverability fix for a real report:
        // with no hint here, a player who wants to turn a cheat off has no
        // way to know the interruptor moved to the pause book.
        cy = draw_text_wrapped_absolute(
            canvas,
            font,
            x,
            cy,
            inner_w,
            TextStyle::new(1, PANEL_DIM),
            "cheats ativos (pausar pra editar)",
        );
        cy += 4;
        for desc in cheats_on {
            // Same projected-bottom check as the commands loop — a single
            // line's worth; a long description that wraps to more may still
            // poke past `limit`, but curated descriptions are short.
            if cy + GLYPH_H as i32 + 2 > limit {
                break;
            }
            cy = draw_text_wrapped_absolute(
                canvas,
                font,
                x,
                cy,
                inner_w,
                TextStyle::new(1, PANEL_TEXT),
                desc,
            );
        }
    }

    // 5. Notes — most-recent capture + counter (plan §3.2, item 5; §3.4).
    // Absent entirely with nothing captured yet, not a "no notes" filler.
    // Same room-for-header-plus-content check as cheats, above — the
    // thumbnail (when there is one) needs its own extra room accounted for.
    let notes_h =
        16 + (GLYPH_H as i32 + 6) + if panel.has_note_thumb { 70 + 6 } else { 0 } + GLYPH_H as i32;
    if panel.note_count > 0 && cy + notes_h <= limit {
        cy += 16;
        draw_text_absolute(
            canvas,
            font,
            x,
            cy,
            TextStyle::new(1, PANEL_DIM),
            "notas",
            usize::MAX,
        );
        cy += GLYPH_H as i32 + 6;
        if panel.has_note_thumb {
            draw_image_absolute(canvas, images, PANEL_NOTE_IMG, x, cy, inner_w, 70);
            cy += 70 + 6;
        }
        let label = format!(
            "{} slot{} usados",
            panel.note_count,
            if panel.note_count == 1 { "" } else { "s" }
        );
        draw_text_absolute(
            canvas,
            font,
            x,
            cy,
            TextStyle::new(1, PANEL_TEXT),
            &label,
            usize::MAX,
        );
    }

    // 6. Session clock, pinned to the bottom (plan §3.2, item 6).
    let secs = session.as_secs();
    let stamp = if secs >= 3600 {
        format!("{}:{:02}:{:02}", secs / 3600, (secs / 60) % 60, secs % 60)
    } else {
        format!("{:02}:{:02}", secs / 60, secs % 60)
    };
    let ty = rect.bottom() - pad - GLYPH_H as i32;
    draw_text_absolute(
        canvas,
        font,
        x,
        ty,
        TextStyle::new(1, PANEL_DIM),
        "session",
        usize::MAX,
    );
    let label_w = (GLYPH_W as i32) * "session ".len() as i32;
    draw_text_absolute(
        canvas,
        font,
        x + label_w,
        ty,
        TextStyle::new(1, PANEL_TEXT),
        &stamp,
        usize::MAX,
    );

    buttons
}

/// Draw one clickable panel button: a filled box (brighter/bordered when
/// `lit`, flush with the panel background otherwise) with `text` centered
/// inside. Returns the box's rect for hit-testing.
fn draw_button(
    canvas: &mut WindowCanvas,
    font: &mut Texture,
    rect: Rect,
    text: &str,
    lit: bool,
) -> Rect {
    let (bg, border, fg) = if lit {
        (PANEL_BTN_BG, PANEL_TEXT, PANEL_TEXT)
    } else {
        (PANEL_BG, PANEL_BTN_BG, PANEL_DIM)
    };
    canvas.set_draw_color(Color::RGB(border.0, border.1, border.2));
    let _ = canvas.fill_rect(rect);
    let inset = Rect::new(
        rect.x() + 1,
        rect.y() + 1,
        rect.width() - 2,
        rect.height() - 2,
    );
    canvas.set_draw_color(Color::RGB(bg.0, bg.1, bg.2));
    let _ = canvas.fill_rect(inset);

    let text_w = (GLYPH_W as i32) * text.chars().count() as i32;
    let tx = rect.x() + (rect.width() as i32 - text_w).max(4) / 2;
    let ty = rect.y() + (rect.height() as i32 - GLYPH_H as i32) / 2;
    draw_text_absolute(
        canvas,
        font,
        tx,
        ty,
        TextStyle::new(1, fg),
        text,
        usize::MAX,
    );
    rect
}

/// One Power/Reset rocker switch (plan revision — styled after the real
/// console's own controls, not another text row): a recessed track with a
/// purple thumb that sits at the top when `up` (Power: on; Reset: mid-press)
/// or the bottom otherwise, plus a label underneath. `lit` dims the whole
/// thing the same way `draw_button` does for a control that wouldn't do
/// anything right now (Reset while the console is off). Returns `track` for
/// hit-testing — the whole switch body is clickable, not just the thumb.
fn draw_rocker(
    canvas: &mut WindowCanvas,
    font: &mut Texture,
    track: Rect,
    label: &str,
    up: bool,
    lit: bool,
) -> Rect {
    canvas.set_draw_color(Color::RGB(
        SWITCH_TRACK_BORDER.0,
        SWITCH_TRACK_BORDER.1,
        SWITCH_TRACK_BORDER.2,
    ));
    let _ = canvas.fill_rect(track);
    let inset = Rect::new(
        track.x() + 2,
        track.y() + 2,
        track.width().saturating_sub(4),
        track.height().saturating_sub(4),
    );
    canvas.set_draw_color(Color::RGB(
        SWITCH_TRACK_BG.0,
        SWITCH_TRACK_BG.1,
        SWITCH_TRACK_BG.2,
    ));
    let _ = canvas.fill_rect(inset);

    let thumb_h = (inset.height() / 2).saturating_sub(3).max(1);
    let thumb_y = if up {
        inset.y() + 2
    } else {
        inset.bottom() - thumb_h as i32 - 2
    };
    let thumb = Rect::new(
        inset.x() + 2,
        thumb_y,
        inset.width().saturating_sub(4),
        thumb_h,
    );
    let base = if lit { SWITCH_PURPLE } else { SWITCH_DIM };
    canvas.set_draw_color(Color::RGB(base.0, base.1, base.2));
    let _ = canvas.fill_rect(thumb);
    if lit {
        // A lighter sliver along the thumb's top edge — cheap stand-in for a
        // bevel/highlight with only flat-fill rects to work with.
        let hi = Rect::new(thumb.x(), thumb.y(), thumb.width(), thumb.height().min(3));
        canvas.set_draw_color(Color::RGB(
            SWITCH_PURPLE_HI.0,
            SWITCH_PURPLE_HI.1,
            SWITCH_PURPLE_HI.2,
        ));
        let _ = canvas.fill_rect(hi);
    }

    let text_w = (GLYPH_W as i32) * label.chars().count() as i32;
    let tx = track.x() + (track.width() as i32 - text_w).max(0) / 2;
    let ty = track.bottom() + 4;
    draw_text_absolute(
        canvas,
        font,
        tx,
        ty,
        TextStyle::new(1, if lit { PANEL_TEXT } else { PANEL_DIM }),
        label,
        usize::MAX,
    );
    track
}

/// Where the pause book's two pages sit: a symmetric spread with a spine gap
/// between them, margins all round. Not tied to the tube's geometry — this
/// screen replaces the whole window (plan §3.2/§3.4).
fn pause_pages(out_w: u32, out_h: u32) -> (Rect, Rect) {
    let margin = (out_w as f32 * PAUSE_MARGIN) as i32;
    let gap = (out_w as f32 * PAUSE_GAP) as i32;
    let top = (out_h as f32 * PAUSE_TOP) as i32;
    let page_w = ((out_w as i32 - 2 * margin - gap) / 2).max(32) as u32;
    let page_h = (out_h as i32 - 2 * top).max(32) as u32;
    let left = Rect::new(margin, top, page_w, page_h);
    let right = Rect::new(margin + page_w as i32 + gap, top, page_w, page_h);
    (left, right)
}

/// Draw the pause book: left page is the notebook's status (how many pages
/// captured so far, or a hint if there are none), right page is the most
/// recent capture, full-size, plus two clickable buttons below the pages —
/// "Continuar" (resume) and "Avancar quadro" (step one frame, plan revision:
/// mouse/gamepad only, no keyboard). `None` (shouldn't happen — always set
/// before `present_pause`/`capture_pause_bmp` are called) draws nothing and
/// returns no buttons.
fn draw_pause_book(
    canvas: &mut WindowCanvas,
    font: &mut Texture,
    images: &HashMap<u64, ImgTex>,
    pause: Option<&PauseNote>,
    cheats: &[(String, bool)],
    out_w: u32,
    out_h: u32,
) -> Vec<(PanelButton, Rect)> {
    canvas.set_draw_color(Color::RGB(CABINET.0, CABINET.1, CABINET.2));
    let _ = canvas.fill_rect(Rect::new(0, 0, out_w, out_h));
    let Some(pause) = pause else {
        return Vec::new();
    };

    let (left, right) = pause_pages(out_w, out_h);
    for page in [left, right] {
        canvas.set_draw_color(Color::RGB(PANEL_BG.0, PANEL_BG.1, PANEL_BG.2));
        let _ = canvas.fill_rect(page);
    }

    let pad = 24i32;
    let lx = left.x() + pad;
    let ly = left.y() + pad;
    let lw = left.width().saturating_sub(pad as u32 * 2);
    let btn_h = (GLYPH_H + 10) as i32;
    let mut buttons = Vec::new();

    let mut cy = draw_text_wrapped_absolute(
        canvas,
        font,
        lx,
        ly,
        lw,
        TextStyle::new(2, PANEL_TEXT),
        &pause.title,
    );
    cy += 16;

    if let Some(draft) = &pause.draft {
        // Editing mode (plan revision: either the free-text note or a
        // slot's caption, `draft_heading` says which): the rest of the left
        // page becomes a live text editor — Continuar/Avancar quadro (below
        // both pages) and the cheats list are hidden until the draft is
        // saved/cancelled, so there's nothing to accidentally lose by
        // clicking.
        draw_text_absolute(
            canvas,
            font,
            lx,
            cy,
            TextStyle::new(1, PANEL_DIM),
            &pause.draft_heading,
            usize::MAX,
        );
        cy += GLYPH_H as i32 + 8;
        cy = draw_text_wrapped_absolute(
            canvas,
            font,
            lx,
            cy,
            lw,
            TextStyle::new(1, PANEL_TEXT),
            &format!("{draft}_"),
        );
        cy += 8;
        draw_text_absolute(
            canvas,
            font,
            lx,
            cy,
            TextStyle::new(1, PANEL_DIM),
            &format!("{}/{}", draft.chars().count(), pause.draft_limit),
            usize::MAX,
        );

        let cancel_y = left.bottom() - pad - btn_h;
        let save_y = cancel_y - btn_h - 6;
        let save_btn = Rect::new(lx, save_y, lw, btn_h as u32);
        let cancel_btn = Rect::new(lx, cancel_y, lw, btn_h as u32);
        buttons.push((
            PanelButton::PauseDraftSave,
            draw_button(canvas, font, save_btn, "Salvar", true),
        ));
        buttons.push((
            PanelButton::PauseDraftCancel,
            draw_button(canvas, font, cancel_btn, "Cancelar", true),
        ));
    } else {
        draw_text_absolute(
            canvas,
            font,
            lx,
            cy,
            TextStyle::new(1, PANEL_DIM),
            "anotacoes",
            usize::MAX,
        );
        cy += GLYPH_H as i32 + 8;
        let status = if pause.filled == 0 {
            format!("Nenhum slot usado ainda (de {}).", pause.captures)
        } else {
            format!("{} de {} slots usados.", pause.filled, pause.captures)
        };
        cy = draw_text_wrapped_absolute(
            canvas,
            font,
            lx,
            cy,
            lw,
            TextStyle::new(1, PANEL_TEXT),
            &status,
        );

        // Cheats — moved here from the side panel (plan revision): plenty of
        // room, and pausing already stopped the game from consuming input,
        // so gamepad-menu-nav could reach these too (not wired up yet).
        let write_y = left.bottom() - pad - btn_h;
        if !cheats.is_empty() {
            cy += 16;
            draw_text_absolute(
                canvas,
                font,
                lx,
                cy,
                TextStyle::new(1, PANEL_DIM),
                "cheats",
                usize::MAX,
            );
            cy += GLYPH_H as i32 + 6;
            let cheats_limit = write_y - 10;
            for (i, (desc, on)) in cheats.iter().enumerate() {
                if cy >= cheats_limit {
                    break;
                }
                let mark = if *on { "[x] " } else { "[ ] " };
                let btn = Rect::new(lx, cy, lw, GLYPH_H + 8);
                let drawn = draw_button(canvas, font, btn, &format!("{mark}{desc}"), true);
                cy = drawn.bottom() + 4;
                buttons.push((PanelButton::CheatRow(i), drawn));
            }
        }

        let write_btn = Rect::new(lx, write_y, lw, btn_h as u32);
        buttons.push((
            PanelButton::PauseWrite,
            draw_button(canvas, font, write_btn, "Escrever anotacao", true),
        ));
    }

    // Right page: the selected slot's image, a pin/caption row, and
    // pagination through all 15 (plan revision — used to always show just
    // the latest, no pinning or naming).
    let rx = right.x() + pad;
    let ry = right.y() + pad;
    let rw = right.width().saturating_sub(pad as u32 * 2);
    // Bottom-up: pin/rename row, then Prev/Next, then the info line(s) —
    // always reserved (whether or not there's a caption) so the image's
    // height doesn't jump around as captions come and go.
    let pin_y = right.bottom() - pad - btn_h;
    let nav_y = pin_y - 6 - btn_h;
    let label_y = nav_y - 6 - (GLYPH_H as i32 + 4);
    let info_y = label_y - (GLYPH_H as i32 + 6);
    let img_h = (info_y - ry).max(0) as u32;

    if pause.has_thumb {
        draw_image_absolute(canvas, images, PAUSE_THUMB_IMG, rx, ry, rw, img_h);
    } else {
        draw_text_absolute(
            canvas,
            font,
            rx,
            ry,
            TextStyle::new(1, PANEL_DIM),
            "slot vazio",
            usize::MAX,
        );
    }

    let info = format!(
        "{}/{}{}",
        pause.page + 1,
        pause.captures,
        if pause.pinned { " (fixado)" } else { "" }
    );
    draw_text_absolute(
        canvas,
        font,
        rx,
        info_y,
        TextStyle::new(1, PANEL_DIM),
        &info,
        usize::MAX,
    );
    if !pause.slot_label.is_empty() {
        draw_text_absolute(
            canvas,
            font,
            rx,
            label_y,
            TextStyle::new(1, PANEL_TEXT),
            &pause.slot_label,
            usize::MAX,
        );
    }

    let half = (rw / 2).saturating_sub(4);
    let prev_btn = Rect::new(rx, nav_y, half, btn_h as u32);
    let next_btn = Rect::new(rx + half as i32 + 8, nav_y, half, btn_h as u32);
    buttons.push((
        PanelButton::PauseNotePrev,
        draw_button(canvas, font, prev_btn, "< anterior", pause.page > 0),
    ));
    buttons.push((
        PanelButton::PauseNoteNext,
        draw_button(
            canvas,
            font,
            next_btn,
            "proxima >",
            pause.page + 1 < pause.captures,
        ),
    ));

    let pin_btn = Rect::new(rx, pin_y, half, btn_h as u32);
    let name_btn = Rect::new(rx + half as i32 + 8, pin_y, half, btn_h as u32);
    buttons.push((
        PanelButton::PauseNotePin,
        draw_button(
            canvas,
            font,
            pin_btn,
            if pause.pinned { "Fixado" } else { "Fixar" },
            true,
        ),
    ));
    buttons.push((
        PanelButton::PauseNoteName,
        draw_button(
            canvas,
            font,
            name_btn,
            if pause.slot_label.is_empty() {
                "Nomear print"
            } else {
                "Renomear"
            },
            true,
        ),
    ));

    // Buttons below both pages, in the same margin band above them
    // (`pause_pages` leaves a symmetric top/bottom gap of `PAUSE_TOP`) —
    // hidden while writing (commit/cancel the draft first).
    if pause.draft.is_none() {
        let band = (out_h as f32 * PAUSE_TOP) as i32;
        let bottom_btn_h = (GLYPH_H + 12) as i32;
        let btn_y = out_h as i32 - band + (band - bottom_btn_h).max(0) / 2;
        let gap = (out_w as f32 * PAUSE_GAP) as i32;
        let continue_btn = Rect::new(left.x(), btn_y, left.width(), bottom_btn_h as u32);
        let step_btn = Rect::new(
            left.x() + left.width() as i32 + gap,
            btn_y,
            right.width(),
            bottom_btn_h as u32,
        );
        buttons.push((
            PanelButton::PauseContinue,
            draw_button(canvas, font, continue_btn, "Continuar", true),
        ));
        buttons.push((
            PanelButton::PauseStep,
            draw_button(canvas, font, step_btn, "Avancar quadro", true),
        ));
    }

    buttons
}

/// Build a textured grid over `dst` whose vertex positions are barrel-distorted
/// (edges bow out, corners pull in) with an edge vignette baked into the vertex
/// colours. Texture coordinates stay a plain grid, so the picture — not just the
/// outline — curves. `alpha` (1.0 normally) lets a caller fade the whole tube in.
fn build_crt_mesh(dst: Rect, alpha: f32) -> CrtMesh {
    let n = CRT_GRID;
    let (ox, oy) = (dst.x() as f32, dst.y() as f32);
    let (dw, dh) = (dst.width() as f32, dst.height() as f32);

    let mut verts = Vec::with_capacity((n + 1) * (n + 1));
    for j in 0..=n {
        for i in 0..=n {
            let u = i as f32 / n as f32; // 0..1 texture / grid coord
            let v = j as f32 / n as f32;
            let cx = u * 2.0 - 1.0; // -1..1
            let cy = v * 2.0 - 1.0;

            // Barrel: corners move toward the centre, mid-edges stay put.
            let dx = cx * (1.0 - CRT_WARP * cy * cy);
            let dy = cy * (1.0 - CRT_WARP * cx * cx);

            let px = ox + (dx * 0.5 + 0.5) * dw;
            let py = oy + (dy * 0.5 + 0.5) * dh;

            let r2 = (cx * cx + cy * cy).min(2.0) / 2.0;
            let shade = (1.0 - CRT_VIGNETTE * r2 * r2).clamp(0.0, 1.0);

            verts.push(Vertex {
                position: sdl3::render::FPoint::new(px, py),
                color: FColor::RGBA(shade, shade, shade, alpha),
                tex_coord: sdl3::render::FPoint::new(u, v),
            });
        }
    }

    let stride = (n + 1) as i32;
    let mut indices = Vec::with_capacity(n * n * 6);
    for j in 0..n as i32 {
        for i in 0..n as i32 {
            let a = j * stride + i;
            let b = a + 1;
            let c = a + stride;
            let d = c + 1;
            indices.extend_from_slice(&[a, c, b, b, c, d]);
        }
    }

    CrtMesh {
        verts,
        indices,
        w: dst.width(),
        h: dst.height(),
    }
}

/// Line-count math shared by [`Cabinet::text_wrapped`]'s layout and
/// [`Cabinet::wrapped_height`]. Mirrors the wrap loop: greedy word packing into
/// `cols` chars, hard-splitting any word longer than a line.
fn wrapped_height(max_w: u32, scale: u32, s: &str) -> i32 {
    let cols = (max_w / (GLYPH_W * scale)).max(1) as usize;
    let row = (GLYPH_H * scale) as i32;
    let mut lines = 0i32;
    let mut len = 0usize;
    let mut open = false;
    for word in s.split_whitespace() {
        if open && len + 1 + word.len() > cols {
            lines += 1;
            len = 0;
            open = false;
        }
        if open {
            len += 1;
        }
        len += word.len();
        open = true;
        while len > cols {
            lines += 1;
            len -= cols;
        }
    }
    if open {
        lines += 1;
    }
    lines * (row + 2)
}

/// Rasterize the covered Unicode range once into one wide texture strip —
/// each cell holds one anti-aliased glyph (alpha = the crate's per-pixel
/// intensity, RGB left white so every draw call tints it via
/// `set_color_mod`), left-aligned in its `GLYPH_W`-wide cell exactly like
/// the atlas the old bitmap font used, just with grayscale edges instead of
/// hard 1-bit ones.
fn build_font_atlas(canvas: &mut WindowCanvas) -> Result<Texture, PlatformError> {
    let w = GLYPH_COLS * GLYPH_W;
    let h = GLYPH_H;
    let mut rgba = vec![0u8; (w * h * 4) as usize];
    for i in 0..GLYPH_COLS {
        let Some(ch) = char::from_u32(GLYPH_FIRST + i) else {
            continue;
        };
        let Some(glyph) = get_raster(ch, FONT_WEIGHT, FONT_HEIGHT) else {
            continue;
        };
        for (row, line) in glyph.raster().iter().enumerate().take(h as usize) {
            for (col, &v) in line.iter().enumerate().take(GLYPH_W as usize) {
                if v == 0 {
                    continue;
                }
                let px = i * GLYPH_W + col as u32;
                let o = ((row as u32 * w + px) * 4) as usize;
                rgba[o..o + 4].copy_from_slice(&[255, 255, 255, v]);
            }
        }
    }
    let mut tex = canvas
        .create_texture_static(SdlFormat::RGBA32, w, h)
        .map_err(|e| PlatformError::Sdl(e.to_string()))?;
    tex.update(None, &rgba, (w * 4) as usize)
        .map_err(|e| PlatformError::Sdl(e.to_string()))?;
    tex.set_blend_mode(BlendMode::Blend);
    tex.set_scale_mode(SdlScaleMode::Nearest);
    Ok(tex)
}

#[cfg(test)]
mod tests {
    use super::{fit_aspect_in, screen_area, wrapped_height, GLYPH_H};
    use sdl3::rect::Rect;

    #[test]
    fn screen_area_insets_with_a_wider_chin() {
        let s = screen_area(1000, 1000);
        assert_eq!(s.x(), 70); // 7% side
        assert_eq!(s.y(), 70); // 7% top
        assert_eq!(s.width(), 860); // 1000 - 2*70
        assert_eq!(s.height(), 820); // 1000 - 70 top - 110 chin
                                     // Never collapses to nothing on a tiny window.
        assert!(screen_area(4, 4).width() >= 16);
    }

    #[test]
    fn fit_aspect_in_centers_a_43_rect() {
        // A wide area: 4:3 fills the height, centered horizontally.
        let r = fit_aspect_in(Rect::new(0, 0, 800, 300), 4.0 / 3.0);
        assert_eq!(r.height(), 300);
        assert_eq!(r.width(), 400);
        assert_eq!(r.x(), 200);
        assert_eq!(r.y(), 0);
        // A tall area: 4:3 fills the width instead.
        let r = fit_aspect_in(Rect::new(0, 0, 400, 900), 4.0 / 3.0);
        assert_eq!(r.width(), 400);
        assert_eq!(r.height(), 300);
        assert_eq!(r.y(), 300);
    }

    #[test]
    fn wrapped_height_counts_lines() {
        let row = GLYPH_H as i32 + 2; // one line's advance at scale 1
        assert_eq!(wrapped_height(90, 1, ""), 0);
        // 90px / 9px advance = 10 cols. "hello world" -> "hello" + " world"
        // = 11 > 10, so two lines.
        assert_eq!(wrapped_height(90, 1, "hello world"), 2 * row);
        // Fits on one line.
        assert_eq!(wrapped_height(90, 1, "hello you"), row);
        // A single word longer than the line is hard-split.
        assert_eq!(wrapped_height(90, 1, &"x".repeat(25)), 3 * row);
    }
}
