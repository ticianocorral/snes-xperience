//! The selector shelf, factored out of the `selector` binary so `xperience` can
//! show it between games: a scrollable grid of covers (or a multicart-style
//! list when no cover art is around) with a details panel, mouse and gamepad
//! navigation only (plan revision: no keyboard shortcuts, so type-to-search
//! is gone too — except for the shelf's own title filter, the one deliberate
//! exception, same as the pause book's free-text note). Cover/logo art is
//! local — dropped by hand into `assets/cover/`/`assets/logo/` (plan §4.3) —
//! matched by the ROM's file name (peeling variant tags down to the base
//! name) and decoded lazily as tiles scroll into view. The details panel itself is drawn flat by
//! `Cabinet` (plan revision: "o painel nao pode estar dentro da TV") —
//! outside the tube's warp, like the in-game side panel — so this module only
//! ever lays out the grid inside `Cabinet::shelf_screen_size`.

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use anyhow::Result;
use xperience_domain::{Catalog, CatalogEntry, Order};
use xperience_platform::{
    Cabinet, MenuMode, MenuNav, Platform, Screen, ShelfButton, ShelfPanelInfo,
};

use crate::idle;

// Landscape, not portrait: the cover art dropped into `assets/cover/` is
// horizontal (box-front scans, not spine-style portrait covers), so the tile
// itself is wide-short rather than tall-narrow — `image_fit` would otherwise
// letterbox a landscape image down to a sliver inside a portrait frame. Sized
// up from the original 200x150 (same 4:3 tile shape, ~30% larger) so covers
// read clearly on the shelf.
const TILE_W: u32 = 260;
const TILE_H: u32 = 195;
const GAP: u32 = 18;
const MARGIN: i32 = 28;
/// Room reserved at the header row for the "N games" label and the filter
/// box — both sit above the grid/recent row, inside the tube like the rest
/// of the shelf's own content.
const HEADER_H: i32 = 28;
/// Thin strip reserved at the grid's right edge for the scroll indicator
/// (plan revision: "mostrar rolagem na tv para visualizar os jogos") —
/// scrolling itself already worked (wheel/d-pad/page keys); this just makes
/// it visible, and how much more there is to see.
const SCROLLBAR_W: u32 = 6;
const SCROLLBAR_RESERVE: u32 = 16;

/// Row height when the shelf falls back to a plain text list (no cover art
/// loaded at all, plan §3.1) — a multicart-style menu instead of a grid of
/// empty tiles.
const LIST_ROW_H: u32 = 22;
const LIST_GAP: u32 = 6;

/// A "jogados recentemente" strip above the main grid/list (plan revision) —
/// the last few played games, always in a single row (never more than
/// `RECENT_MAX`, so it never needs its own scrolling).
const RECENT_MAX: usize = 5;
const RECENT_TILE_W: u32 = 160;
const RECENT_TILE_H: u32 = 120;
const RECENT_GAP: u32 = 14;
const RECENT_LABEL_H: i32 = 24;

/// The main grid/list's own section heading (plan revision: "colocar um
/// titulo (Todos os jogos)") — mirrors the "jogados recentemente" label
/// above the recent strip, so both sections read the same way.
const ALL_GAMES_LABEL_H: i32 = 24;
const ALL_GAMES_TITLE: &str = "todos os jogos";

/// The title filter box (plan revision: "colocar filtro para facilitar o
/// encontro dos games na lista") — the shelf's own one deliberate keyboard
/// exception, same pattern as the pause book's free-text note.
const FILTER_BOX_W: u32 = 240;
const FILTER_BOX_H: u32 = 24;
const FILTER_LIMIT: usize = 40;

/// "Histórico" button (plan revision: "colocar um botão para histórico
/// listando os jogos mais jogados") — drawn in the header, left of the
/// filter box, same row/height.
const HISTORY_BTN_W: u32 = 110;

const BG: (u8, u8, u8) = (18, 18, 20);
const TILE_BG: (u8, u8, u8, u8) = (34, 34, 40, 255);
const HILITE: (u8, u8, u8, u8) = (240, 200, 80, 255);
const TEXT: (u8, u8, u8) = (232, 232, 232);
const DIM: (u8, u8, u8) = (150, 150, 158);
/// Text color on top of the `HILITE` selection bar in list mode — dark, for
/// contrast against the bright fill.
const HILITE_TEXT: (u8, u8, u8) = (24, 20, 12);
/// How opaque the shelf sits over the resting TV static (plan §3.3) — under
/// 1.0 so the signal-off snow bleeds through faintly instead of a flat
/// background, but not enough to fight with reading the grid/list.
const SHELF_ALPHA: f32 = 0.92;

/// What the player did on the shelf.
pub enum Pick {
    /// Launch this ROM (already marked played in the catalogue).
    Play {
        rom: PathBuf,
        /// Local logo art (`assets/logo/<rom stem>.*`), if one exists — for
        /// the side panel during play (plan §3.2).
        wheel: Option<PathBuf>,
        /// Local cartridge art (`assets/cartridge/<rom stem>.*`), if one
        /// exists — shown in the panel alongside the logo (plan revision).
        cartridge: Option<PathBuf>,
    },
    /// Clicked "Voltar", or a gamepad's Back button — back out to the
    /// idle/root screen. The app keeps running.
    Back,
    /// Window closed / Cmd-Q — tear the app down.
    Quit,
    /// Clicked "Configurações" — open the settings screen, then come back to
    /// the shelf.
    Settings,
    /// Clicked "Histórico" — open the all-time most-played list (plan
    /// revision), then come back to the shelf.
    History,
}

/// Knobs for [`run`].
pub struct ShelfOpts {
    pub order: Order,
    /// Headless smoke test: stop after N frames and return [`Pick::Quit`].
    pub max_frames: Option<u64>,
    /// Headless: on the last frame, save the shelf (through the tube) here.
    pub shot: Option<PathBuf>,
    /// Ease in from residual signal-off static instead of cutting in cold —
    /// the static level to fade from (plan §3.3). `None` draws from frame one.
    pub fade_in: Option<f32>,
    /// Headless only: pre-apply this as the title filter from frame one,
    /// as if the player had already typed and committed it — there's no way
    /// to simulate real typing in a headless run, so this is how `--shot`
    /// verifies the filtered view (plan revision).
    pub preset_filter: Option<String>,
}

impl Default for ShelfOpts {
    fn default() -> Self {
        Self {
            order: Order::Shelf,
            max_frames: None,
            shot: None,
            fade_in: None,
            preset_filter: None,
        }
    }
}

/// Texture key for a game's cover — first 64 bits of the SHA1.
fn cover_id(sha1: &str) -> u64 {
    u64::from_str_radix(sha1.get(..16).unwrap_or("0"), 16).unwrap_or(0)
}

/// Texture key for a game's wheel logo — the *next* 64 bits, so it can't
/// collide with any `cover_id`.
fn wheel_id(sha1: &str) -> u64 {
    u64::from_str_radix(sha1.get(16..32).unwrap_or("0"), 16).unwrap_or(0)
}

/// Texture key for a game's cartridge art (plan revision — the shelf panel
/// now shows it alongside the logo, same as the in-game panel does): derived
/// from `wheel_id` with a fixed splitmix64-style salt rather than a third,
/// thinner slice of the sha1, so it keeps the full 64 bits of entropy and
/// can't collide with `cover_id`/`wheel_id`.
fn cartridge_id(sha1: &str) -> u64 {
    wheel_id(sha1) ^ 0x9E37_79B9_7F4A_7C15
}

/// Texture key for a game's back-cover art (plan revision: "abaixo da
/// logo... colocar o back cover tambem") — same derivation as
/// `cartridge_id`, a different fixed salt so it can't collide with any of
/// the other three.
fn backcover_id(sha1: &str) -> u64 {
    wheel_id(sha1) ^ 0xC2B2_AE3D_27D4_EB4F
}

/// `dir/<rom's file stem>.{png,jpg,jpeg}`, in that order — the convention for
/// locally-supplied art: `roms/Aladdin.sfc` matches `assets/cover/Aladdin.png`.
/// If the exact stem misses, the trailing "(...)" tags are peeled one group at
/// a time ("X (USA) (Rev 1)" -> "X (USA)" -> "X"), so a single base-named art
/// file serves every variant ROM of the same game.
fn find_local_art(dir: &Path, rom_path: &str) -> Option<PathBuf> {
    let stem = Path::new(rom_path).file_stem()?.to_str()?;
    let mut candidates = vec![stem.to_string()];
    let mut cur = stem.to_string();
    while cur.ends_with(')') {
        match cur.rfind(" (") {
            Some(idx) => {
                cur.truncate(idx);
                candidates.push(cur.clone());
            }
            None => break,
        }
    }
    for cand in candidates {
        for ext in ["png", "jpg", "jpeg"] {
            let p = dir.join(format!("{cand}.{ext}"));
            if p.is_file() {
                return Some(p);
            }
        }
    }
    None
}

/// Mark `entry` played and build its launch — shared by Enter/A confirm and
/// clicking the already-selected tile.
fn pick_play(
    catalog: &Catalog,
    logo_dir: &Path,
    cartridge_dir: &Path,
    entry: &CatalogEntry,
) -> Pick {
    let _ = catalog.mark_played(&entry.rom.sha1);
    let wheel = find_local_art(logo_dir, &entry.rom.path);
    let cartridge = find_local_art(cartridge_dir, &entry.rom.path);
    Pick::Play {
        rom: PathBuf::from(&entry.rom.path),
        wheel,
        cartridge,
    }
}

/// Whether `entry`'s title matches a filter query — empty matches everything,
/// otherwise a case-insensitive substring test (plan revision: "colocar
/// filtro para facilitar o encontro dos games na lista").
/// The panel's own file/play facts (plan revision: "o painel... muito
/// vazio") — always available straight from the scan (or, for `playtime_
/// secs`, from the per-game sidecar `runner::total_playtime_secs` already
/// writes), no DAT/local art needed, so a plain title-only game still gets a
/// panel with something in it besides two buttons. Appended after any
/// No-Intro extras.
fn game_info_lines(entry: &CatalogEntry, playtime_secs: u64) -> Vec<(String, String)> {
    vec![
        ("tamanho".to_string(), human_size(entry.rom.size)),
        (
            "jogado".to_string(),
            entry
                .rom
                .last_played_at
                .map(days_ago)
                .unwrap_or_else(|| "nunca".to_string()),
        ),
        ("tempo total".to_string(), format_playtime(playtime_secs)),
    ]
}

fn human_size(bytes: u64) -> String {
    let mb = bytes as f64 / 1_048_576.0;
    if mb >= 1.0 {
        format!("{mb:.1} MB")
    } else {
        format!("{:.0} KB", bytes as f64 / 1024.0)
    }
}

/// Total powered-on time for a game (plan revision: "mostrar tempo total de
/// jogo") — `0` (never turned on, or never played) reads as "nunca" rather
/// than a confusing "0min".
fn format_playtime(secs: u64) -> String {
    if secs == 0 {
        return "nunca".to_string();
    }
    let h = secs / 3600;
    let m = (secs % 3600) / 60;
    match (h, m) {
        (0, 0) => "menos de 1min".to_string(),
        (0, m) => format!("{m}min"),
        (h, m) => format!("{h}h {m}min"),
    }
}

/// "hoje"/"há 1 dia"/"há N dias" — same coarse-days style
/// `settings::core_installed_label` already uses, reused here so the whole
/// app tells relative time the same way instead of two different phrasings.
fn days_ago(unix_secs: i64) -> String {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    let days = (now - unix_secs).max(0) / 86_400;
    match days {
        0 => "hoje".to_string(),
        1 => "há 1 dia".to_string(),
        n => format!("há {n} dias"),
    }
}

fn filter_box_rect(scr_w: u32) -> (i32, i32, u32, u32) {
    (
        scr_w as i32 - MARGIN - FILTER_BOX_W as i32,
        MARGIN - 18,
        FILTER_BOX_W,
        FILTER_BOX_H,
    )
}

fn history_button_rect(scr_w: u32) -> (i32, i32, u32, u32) {
    let (fx, fy, _, fh) = filter_box_rect(scr_w);
    (fx - 12 - HISTORY_BTN_W as i32, fy, HISTORY_BTN_W, fh)
}

/// Geometry of the shelf's item grid — image tiles, or (no cover art loaded
/// at all, plan §3.1) a single-column text list styled like a pirate NES
/// multicart menu. One column of navigation math (`cols`/`vis_rows`) serves
/// both: list mode is just `cols == 1` with a short row instead of a tile.
struct GridLayout {
    x0: i32,
    y0: i32,
    /// Cell stride, including the gap.
    cell_w: i32,
    cell_h: i32,
    /// The clickable item itself, within its cell (no gap).
    item_w: i32,
    item_h: i32,
    cols: usize,
    vis_rows: usize,
    list_mode: bool,
}

impl GridLayout {
    /// `top_y` is where the grid itself starts, below the header row and any
    /// "jogados recentemente" strip above it (plan revision — the panel used
    /// to eat into this width; now the grid gets the whole shelf screen).
    fn new(scr_w: u32, scr_h: u32, list_mode: bool, top_y: i32) -> Self {
        let grid_w = scr_w.saturating_sub(MARGIN as u32 * 2 + SCROLLBAR_RESERVE);
        let (cell_w, cell_h, item_w, item_h, cols) = if list_mode {
            let cell_h = (LIST_ROW_H + LIST_GAP) as i32;
            (grid_w as i32, cell_h, grid_w as i32, LIST_ROW_H as i32, 1)
        } else {
            let cell_w = (TILE_W + GAP) as i32;
            let cell_h = (TILE_H + GAP) as i32;
            let cols = (grid_w / (TILE_W + GAP)).max(1) as usize;
            (cell_w, cell_h, TILE_W as i32, TILE_H as i32, cols)
        };
        let vis_rows = ((scr_h as i32 - top_y - MARGIN) / cell_h).max(1) as usize;
        Self {
            x0: MARGIN,
            y0: top_y,
            cell_w,
            cell_h,
            item_w,
            item_h,
            cols,
            vis_rows,
            list_mode,
        }
    }

    /// Top-left of item `i`'s cell, relative to the grid's own row window
    /// (`top_row` is the first visible row) — `None` if it's scrolled out of
    /// view.
    fn cell_pos(&self, i: usize, top_row: usize) -> Option<(i32, i32)> {
        let row = i / self.cols;
        if row < top_row || row >= top_row + self.vis_rows {
            return None;
        }
        let col = i % self.cols;
        Some((
            self.x0 + col as i32 * self.cell_w,
            self.y0 + (row - top_row) as i32 * self.cell_h,
        ))
    }

    /// Which visible item (if any) a screen-local point sits inside — `None`
    /// in the gap between cells, past the last column, below the last
    /// visible row, or past the end of `view`.
    fn tile_at(&self, x: i32, y: i32, top_row: usize, view_len: usize) -> Option<usize> {
        let (dx, dy) = (x - self.x0, y - self.y0);
        if dx < 0 || dy < 0 || dx % self.cell_w >= self.item_w || dy % self.cell_h >= self.item_h {
            return None;
        }
        let col = (dx / self.cell_w) as usize;
        let row_in_view = (dy / self.cell_h) as usize;
        if col >= self.cols || row_in_view >= self.vis_rows {
            return None;
        }
        let i = (top_row + row_in_view) * self.cols + col;
        (i < view_len).then_some(i)
    }

    fn total_rows(&self, view_len: usize) -> usize {
        view_len.div_ceil(self.cols)
    }
}

/// Geometry of the "jogados recentemente" strip — always a single row, up to
/// `RECENT_MAX` items, so unlike `GridLayout` it never scrolls.
struct RecentLayout {
    x0: i32,
    y0: i32,
    cell_w: i32,
}

impl RecentLayout {
    fn new(y0: i32) -> Self {
        Self {
            x0: MARGIN,
            y0,
            cell_w: (RECENT_TILE_W + RECENT_GAP) as i32,
        }
    }

    fn item_pos(&self, i: usize) -> (i32, i32) {
        (self.x0 + i as i32 * self.cell_w, self.y0)
    }

    fn hit(&self, x: i32, y: i32, count: usize) -> Option<usize> {
        let (dx, dy) = (x - self.x0, y - self.y0);
        if dx < 0
            || dy < 0
            || dy >= RECENT_TILE_H as i32
            || dx % self.cell_w >= RECENT_TILE_W as i32
        {
            return None;
        }
        let i = (dx / self.cell_w) as usize;
        (i < count).then_some(i)
    }
}

/// `roms/` has nothing in it — a friendlier landing than a hard error, since
/// an empty ROMs folder is the expected first-launch state for a portable,
/// autoexecutável app, not a misconfiguration.
fn empty_roms_screen(plat: &mut Platform, cab: &mut Cabinet) -> Result<Pick> {
    let frame = Duration::from_millis(16);
    cab.set_shelf_panel(empty_shelf_panel());
    cab.set_close_button(true);
    loop {
        let m = plat.poll_menu(MenuMode::Nav);
        if m.quit {
            return Ok(Pick::Quit);
        }
        if m.nav.contains(&MenuNav::Back) {
            return Ok(Pick::Back);
        }
        if let Some((x, y)) = m.click {
            let (ox, oy) = cab.window_to_output(x, y);
            if cab.hit_close_button(ox, oy) {
                return Ok(Pick::Quit);
            }
            match cab.hit_shelf_button(ox, oy) {
                Some(ShelfButton::Back) => return Ok(Pick::Back),
                Some(ShelfButton::Settings) => return Ok(Pick::Settings),
                // No game focused on this screen, so nothing to scroll (and
                // no back cover to enlarge).
                Some(ShelfButton::PanelScrollUp)
                | Some(ShelfButton::PanelScrollDown)
                | Some(ShelfButton::Backcover)
                | None => {}
            }
        }
        let (scr_w, _) = cab.shelf_screen_size();
        let render = |d: &mut Screen| {
            d.text(MARGIN, MARGIN, 2, TEXT, "nenhuma rom encontrada");
            d.text_wrapped(
                MARGIN,
                MARGIN + 40,
                scr_w.saturating_sub(MARGIN as u32 * 2),
                1,
                DIM,
                "copie seus arquivos .sfc/.smc para a pasta roms/, ao lado do \
                 executável, e volte para esta tela.",
            );
        };
        cab.frame_shelf(BG, render);
        std::thread::sleep(frame);
    }
}

/// Show the shelf in `cab` (the one persistent window) until the player picks a
/// game or cancels. `plat` and `cab` both outlive the call.
pub fn run(
    plat: &mut Platform,
    cab: &mut Cabinet,
    catalog: &Catalog,
    opts: &ShelfOpts,
) -> Result<Pick> {
    let all_scanned = catalog.list(opts.order)?;
    if all_scanned.is_empty() {
        return empty_roms_screen(plat, cab);
    }
    let all = all_scanned;

    let cover_dir = crate::dirs::assets_dir().join("cover");
    let logo_dir = crate::dirs::assets_dir().join("logo");
    let cartridge_dir = crate::dirs::assets_dir().join("cartridge");
    let backcover_dir = crate::dirs::assets_dir().join("backcover");
    // Never re-stat a game's art more than once per shelf visit — most games
    // won't have any, and disk isn't free even if it's cheap.
    let mut tried_cover: HashSet<String> = HashSet::new();
    // Back cover enlarged (plan revision: "ao clicar no back cover
    // possibilitar mostrar em tamanho maior, com botão de fechar"): Some =
    // the close button's rect — the zoom replaces the whole shelf until
    // dismissed.
    let mut zoom_close: Option<(i32, i32, u32, u32)> = None;
    let mut tried_logo: HashSet<String> = HashSet::new();
    let mut tried_cartridge: HashSet<String> = HashSet::new();
    let mut tried_backcover: HashSet<String> = HashSet::new();

    let mut sel: usize = 0;
    let mut top_row: usize = 0;
    // The "jogados recentemente" strip has its own cursor (plan revision) —
    // independent of `sel`, the main grid/list's own.
    let mut in_recent = false;
    let mut recent_idx: usize = 0;
    // The panel's own scroll position (plan revision: "criar rolagem no
    // painel quando necessario") — reset whenever the focused game changes,
    // via `last_focus` below; `draw_shelf_panel` clamps it to whatever the
    // currently-focused game's content actually needs, so letting it run
    // free on repeated "v Baixo" clicks past the end is harmless.
    let mut panel_scroll: usize = 0;
    let mut last_focus: Option<(bool, usize)> = None;
    // The title filter (plan revision) — `filter_query` is what's actually
    // applied; `filter_draft`/`editing_filter` are live only while typing,
    // same split the pause book's free-text note uses.
    let mut filter_query = opts.preset_filter.clone().unwrap_or_default();
    let mut filter_draft = String::new();
    let mut editing_filter = false;
    let frame = Duration::from_millis(16);
    let mut next = Instant::now() + frame;
    let mut frame_no = 0u64;
    // The filtered/sorted listings are rebuilt only when the applied filter
    // changes (and once up front) — re-sorting ~700 entries with fresh String
    // allocations at 60fps was the shelf's biggest per-frame cost. While the
    // player is still TYPING, `filter_draft` moves but `filter_query` doesn't,
    // so typing doesn't rebuild either.
    let mut applied_filter: Option<String> = None;
    let mut view: Vec<&CatalogEntry> = Vec::new();
    let mut recent: Vec<&CatalogEntry> = Vec::new();
    cab.set_close_button(true);

    // Frames left in the "entering over the static" ease-in (§3.3), if any.
    const FADE_IN_FRAMES: u32 = 18;
    let mut fade_frame: u32 = 0;

    loop {
        frame_no += 1;
        if opts.max_frames.is_some_and(|n| frame_no > n) {
            return Ok(Pick::Quit);
        }

        // The general listing is always alphabetical (plan revision) — the
        // recent strip above it already covers "what did I just play",
        // freeing this one up to just be a plain, predictable A-Z browse.
        if applied_filter.as_deref() != Some(filter_query.as_str()) {
            let query_lc = filter_query.to_lowercase();
            view = all
                .iter()
                .filter(|e| e.title().to_lowercase().contains(&query_lc))
                .collect();
            view.sort_by_key(|e| e.title().to_lowercase());
            // The recent strip only makes sense browsing the unfiltered
            // shelf — once a search narrows things, the whole point is
            // finding a specific game, not re-surfacing what was just
            // played.
            recent = if filter_query.is_empty() {
                let mut r: Vec<&CatalogEntry> = all
                    .iter()
                    .filter(|e| e.rom.last_played_at.is_some())
                    .collect();
                r.sort_by_key(|e| std::cmp::Reverse(e.rom.last_played_at.unwrap_or(0)));
                r.truncate(RECENT_MAX);
                r
            } else {
                Vec::new()
            };
            applied_filter = Some(filter_query.clone());
        }
        if sel >= view.len() {
            sel = view.len().saturating_sub(1);
        }
        let show_recent = !recent.is_empty();
        if recent_idx >= recent.len() {
            recent_idx = recent.len().saturating_sub(1);
        }
        if !show_recent {
            in_recent = false;
        }

        let (scr_w, scr_h) = cab.shelf_screen_size();
        // No cover art loaded anywhere in view yet: a text list (multicart
        // menu) reads as deliberate, where a grid of empty tiles reads as
        // broken (plan §3.1).
        let list_mode = !view.iter().any(|e| cab.has_image(cover_id(&e.rom.sha1)));
        let recent_block_h = if show_recent {
            RECENT_LABEL_H + RECENT_TILE_H as i32 + GAP as i32
        } else {
            0
        };
        let all_games_label_y = MARGIN + HEADER_H + recent_block_h;
        let grid_top_y = all_games_label_y + ALL_GAMES_LABEL_H;
        let grid = GridLayout::new(scr_w, scr_h, list_mode, grid_top_y);
        let recent_layout = RecentLayout::new(MARGIN + HEADER_H + RECENT_LABEL_H);
        let filter_rect = filter_box_rect(scr_w);
        let history_rect = history_button_rect(scr_w);

        // Input.
        if editing_filter {
            let te = plat.poll_text_entry();
            if te.quit {
                return Ok(Pick::Quit);
            }
            if te.backspace {
                filter_draft.pop();
            }
            for c in te.typed.chars() {
                if filter_draft.chars().count() < FILTER_LIMIT {
                    filter_draft.push(c);
                }
            }
            if te.commit {
                filter_query = filter_draft.trim().to_string();
                editing_filter = false;
                plat.stop_text_input(cab);
                sel = 0;
                top_row = 0;
                in_recent = false;
            } else if te.cancel {
                editing_filter = false;
                plat.stop_text_input(cab);
            }
        } else {
            let m = plat.poll_menu(MenuMode::Nav);
            if m.quit {
                return Ok(Pick::Quit);
            }
            if zoom_close.is_some() {
                // The enlarged back cover is up: only the close button (or
                // Back) dismisses it — clicks elsewhere do nothing.
                if m.nav.iter().any(|n| matches!(n, MenuNav::Back)) {
                    zoom_close = None;
                }
                if let Some((x, y)) = m.click {
                    let (ox, oy) = cab.window_to_output(x, y);
                    if let Some(close) = zoom_close {
                        if in_rect(ox, oy, close) {
                            zoom_close = None;
                        }
                    }
                }
            } else {
                for nav in m.nav {
                    match nav {
                        MenuNav::Left => {
                            if in_recent {
                                recent_idx = recent_idx.saturating_sub(1);
                            } else {
                                sel = sel.saturating_sub(1);
                            }
                        }
                        MenuNav::Right => {
                            if in_recent {
                                if recent_idx + 1 < recent.len() {
                                    recent_idx += 1;
                                }
                            } else if sel + 1 < view.len() {
                                sel += 1;
                            }
                        }
                        MenuNav::Up => {
                            if !in_recent {
                                if show_recent && sel < grid.cols {
                                    in_recent = true;
                                    recent_idx = sel.min(recent.len().saturating_sub(1));
                                } else {
                                    sel = sel.saturating_sub(grid.cols);
                                }
                            }
                        }
                        MenuNav::Down => {
                            if in_recent {
                                in_recent = false;
                                sel = recent_idx.min(view.len().saturating_sub(1));
                            } else if sel + grid.cols < view.len() {
                                sel += grid.cols;
                            }
                        }
                        MenuNav::PageUp => {
                            if !in_recent {
                                sel = sel.saturating_sub(grid.cols * grid.vis_rows);
                            }
                        }
                        MenuNav::PageDown => {
                            if !in_recent {
                                sel = (sel + grid.cols * grid.vis_rows)
                                    .min(view.len().saturating_sub(1))
                            }
                        }
                        MenuNav::Home => {
                            if show_recent {
                                in_recent = true;
                                recent_idx = 0;
                            } else {
                                sel = 0;
                            }
                        }
                        MenuNav::End => {
                            in_recent = false;
                            sel = view.len().saturating_sub(1);
                        }
                        MenuNav::Back => return Ok(Pick::Back),
                        MenuNav::Confirm => {
                            let picked = if in_recent {
                                recent.get(recent_idx)
                            } else {
                                view.get(sel)
                            };
                            if let Some(e) = picked {
                                return Ok(pick_play(catalog, &logo_dir, &cartridge_dir, e));
                            }
                        }
                    }
                }

                // Mouse: click a tile to select it, click the already-selected one
                // to launch — the same two-step a controller does (move, then A).
                // The flat panel's "Configurações"/"Voltar" buttons (plan
                // revision: now outside the tube) are the only way into either
                // without a gamepad now.
                if let Some((x, y)) = m.click {
                    let (ox, oy) = cab.window_to_output(x, y);
                    if cab.hit_close_button(ox, oy) {
                        return Ok(Pick::Quit);
                    }
                    if let Some(hit) = cab.hit_shelf_button(ox, oy) {
                        match hit {
                            ShelfButton::Back => return Ok(Pick::Back),
                            ShelfButton::Settings => return Ok(Pick::Settings),
                            ShelfButton::PanelScrollUp => {
                                panel_scroll = panel_scroll.saturating_sub(1)
                            }
                            ShelfButton::PanelScrollDown => panel_scroll += 1,
                            ShelfButton::Backcover => {
                                // Open the enlarged view: re-decode the file at
                                // full resolution for a crisp enlargement (the
                                // panel texture is thumbnailed to 512).
                                let picked = if in_recent {
                                    recent.get(recent_idx)
                                } else {
                                    view.get(sel)
                                };
                                if let Some(e) = picked {
                                    let bid = backcover_id(&e.rom.sha1);
                                    if cab.has_image(bid) {
                                        if let Some(path) =
                                            find_local_art(&backcover_dir, &e.rom.path)
                                        {
                                            if let Ok((w, h, rgba)) = decode_art_scaled(&path, 2048)
                                            {
                                                cab.set_image(bid, w, h, &rgba);
                                                zoom_close = Some((0, 0, 1, 1));
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    } else if let Some((lx, ly)) = cab.hit_screen_point(ox, oy) {
                        if in_rect(lx, ly, filter_rect) {
                            editing_filter = true;
                            filter_draft = filter_query.clone();
                            plat.start_text_input(cab);
                        } else if in_rect(lx, ly, history_rect) {
                            return Ok(Pick::History);
                        } else if let Some(i) = show_recent
                            .then(|| recent_layout.hit(lx, ly, recent.len()))
                            .flatten()
                        {
                            if in_recent && i == recent_idx {
                                if let Some(e) = recent.get(i) {
                                    return Ok(pick_play(catalog, &logo_dir, &cartridge_dir, e));
                                }
                            } else {
                                in_recent = true;
                                recent_idx = i;
                            }
                        } else if let Some(i) = grid.tile_at(lx, ly, top_row, view.len()) {
                            if !in_recent && i == sel {
                                if let Some(e) = view.get(i) {
                                    return Ok(pick_play(catalog, &logo_dir, &cartridge_dir, e));
                                }
                            } else {
                                in_recent = false;
                                sel = i;
                            }
                        }
                    }
                }
            }
        }

        // Keep selection visible.
        let sel_row = sel.checked_div(grid.cols).unwrap_or(0);
        if sel_row < top_row {
            top_row = sel_row;
        } else if sel_row >= top_row + grid.vis_rows {
            top_row = sel_row + 1 - grid.vis_rows;
        }

        // Local art for whatever just scrolled into view — no network, no
        // worker thread, so this happens inline, capped at ONE decode per
        // frame: scrolling a full page fills over a dozen frames instead of
        // hitching on dozens of JPEG decodes in a single one. Each sha1 is
        // tried at most once per visit, whether or not a file turns up; a
        // tile whose budget ran out simply waits for the next frame.
        let mut decode_budget = 1usize;
        for (i, entry) in view.iter().enumerate() {
            if grid.cell_pos(i, top_row).is_none() {
                continue;
            }
            let id = cover_id(&entry.rom.sha1);
            if cab.has_image(id) {
                continue;
            }
            if decode_budget == 0 {
                break;
            }
            if !tried_cover.insert(entry.rom.sha1.clone()) {
                continue;
            }
            if let Some(path) = find_local_art(&cover_dir, &entry.rom.path) {
                if let Ok((w, h, rgba)) = decode_art(&path) {
                    cab.set_image(id, w, h, &rgba);
                    decode_budget -= 1;
                }
            }
        }
        // The recent strip sits outside the main grid's scroll window, so it
        // needs its own pass — a recently-played game might not be among the
        // rows currently visible below. Same one-decode-per-frame cap.
        let mut recent_budget = 1usize;
        for entry in &recent {
            if recent_budget == 0 {
                break;
            }
            let id = cover_id(&entry.rom.sha1);
            if cab.has_image(id) {
                continue;
            }
            if !tried_cover.insert(entry.rom.sha1.clone()) {
                continue;
            }
            if let Some(path) = find_local_art(&cover_dir, &entry.rom.path) {
                if let Ok((w, h, rgba)) = decode_art(&path) {
                    cab.set_image(id, w, h, &rgba);
                    recent_budget -= 1;
                }
            }
        }

        let focused: Option<&CatalogEntry> = if in_recent {
            recent.get(recent_idx).copied()
        } else {
            view.get(sel).copied()
        };
        // A new game focused starts its panel scrolled to the top again —
        // otherwise switching from a long-content game to a short one could
        // leave the scroll position pointing past the end of the new one
        // until `draw_shelf_panel`'s own clamp kicks in.
        let focus_key = focused
            .is_some()
            .then_some((in_recent, if in_recent { recent_idx } else { sel }));
        if focus_key != last_focus {
            panel_scroll = 0;
            last_focus = focus_key;
        }
        if let Some(e) = focused {
            let wid = wheel_id(&e.rom.sha1);
            if !cab.has_image(wid) && tried_logo.insert(e.rom.sha1.clone()) {
                if let Some(path) = find_local_art(&logo_dir, &e.rom.path) {
                    if let Ok((w, h, rgba)) = decode_art(&path) {
                        cab.set_image(wid, w, h, &rgba);
                    }
                }
            }
            let cid = cartridge_id(&e.rom.sha1);
            if !cab.has_image(cid) && tried_cartridge.insert(e.rom.sha1.clone()) {
                if let Some(path) = find_local_art(&cartridge_dir, &e.rom.path) {
                    if let Ok((w, h, rgba)) = decode_art(&path) {
                        cab.set_image(cid, w, h, &rgba);
                    }
                }
            }
            let bid = backcover_id(&e.rom.sha1);
            if !cab.has_image(bid) && tried_backcover.insert(e.rom.sha1.clone()) {
                if let Some(path) = find_local_art(&backcover_dir, &e.rom.path) {
                    if let Ok((w, h, rgba)) = decode_art(&path) {
                        cab.set_image(bid, w, h, &rgba);
                    }
                }
            }
        }

        // The flat side panel (plan revision — drawn by `Cabinet` itself,
        // outside the tube): whatever's focused right now, DAT extras and
        // all when the DAT had any ("se o DAT tiver informacoes do jogo,
        // preencher no painel").
        let shelf_panel = match focused {
            Some(e) => {
                let wid = wheel_id(&e.rom.sha1);
                let cid = cartridge_id(&e.rom.sha1);
                let bid = backcover_id(&e.rom.sha1);
                let save_title = crate::runner::rom_title(Path::new(&e.rom.path));
                let playtime =
                    crate::runner::total_playtime_secs(&crate::dirs::saves_dir(), &save_title);
                // The DAT's year, if it had one, moves into its own "release"
                // slot (plan revision) instead of sitting in the generic
                // info list alongside publisher/category/description.
                let mut nointro = e.rom.nointro_extra.clone();
                let release = nointro
                    .iter()
                    .position(|(k, _)| k == "ano")
                    .map(|i| nointro.remove(i).1);
                let mut info = nointro;
                info.extend(game_info_lines(e, playtime));
                ShelfPanelInfo {
                    title: e.title().into_owned(),
                    logo_img: cab.has_image(wid).then_some(wid),
                    cartridge_img: cab.has_image(cid).then_some(cid),
                    backcover_img: cab.has_image(bid).then_some(bid),
                    release,
                    info,
                    scroll: panel_scroll,
                }
            }
            None => ShelfPanelInfo {
                title: "nenhum jogo encontrado".to_string(),
                logo_img: None,
                cartridge_img: None,
                backcover_img: None,
                release: None,
                info: Vec::new(),
                scroll: 0,
            },
        };
        cab.set_shelf_panel(shelf_panel);

        // --- draw (into a screen-sized buffer, then warped through the tube) --
        let render = |d: &mut Screen| {
            let count_label = if filter_query.is_empty() {
                format!("{} games", view.len())
            } else {
                format!("{} de {} games", view.len(), all.len())
            };
            d.text(MARGIN, MARGIN - 12, 2, DIM, &count_label);
            draw_filter_box(d, filter_rect, &filter_query, &filter_draft, editing_filter);
            draw_history_button(d, history_rect);

            if show_recent {
                d.text(MARGIN, MARGIN + HEADER_H, 1, DIM, "jogados recentemente");
                for (i, entry) in recent.iter().enumerate() {
                    let (x, y) = recent_layout.item_pos(i);
                    d.fill(x, y, RECENT_TILE_W, RECENT_TILE_H, TILE_BG);
                    let id = cover_id(&entry.rom.sha1);
                    if d.has_image(id) {
                        d.image_fit(id, x + 4, y + 4, RECENT_TILE_W - 8, RECENT_TILE_H - 8);
                    } else {
                        d.text_wrapped(
                            x + 6,
                            y + 8,
                            RECENT_TILE_W - 12,
                            1,
                            DIM,
                            &entry.title().to_uppercase(),
                        );
                    }
                    if in_recent && i == recent_idx {
                        d.outline(
                            x - 3,
                            y - 3,
                            RECENT_TILE_W + 6,
                            RECENT_TILE_H + 6,
                            3,
                            HILITE,
                        );
                    }
                }
            }

            d.text(MARGIN, all_games_label_y, 1, DIM, ALL_GAMES_TITLE);

            if grid.list_mode {
                // Multicart menu: a plain numbered list, selection as an
                // inverted bar — no tile, no art, nothing pretending there's
                // a cover coming.
                for (i, entry) in view.iter().enumerate() {
                    let Some((x, y)) = grid.cell_pos(i, top_row) else {
                        continue;
                    };
                    let selected = !in_recent && i == sel;
                    if selected {
                        d.fill(x, y, grid.item_w as u32, grid.item_h as u32, HILITE);
                    }
                    let label = format!("{:03}  {}", i + 1, entry.title().to_uppercase());
                    let color = if selected { HILITE_TEXT } else { TEXT };
                    d.text(x + 6, y + 3, 1, color, &label);
                }
            } else {
                for (i, entry) in view.iter().enumerate() {
                    let Some((x, y)) = grid.cell_pos(i, top_row) else {
                        continue;
                    };
                    d.fill(x, y, TILE_W, TILE_H, TILE_BG);
                    let id = cover_id(&entry.rom.sha1);
                    if d.has_image(id) {
                        d.image_fit(id, x + 4, y + 4, TILE_W - 8, TILE_H - 8);
                    } else {
                        d.text_wrapped(
                            x + 8,
                            y + 10,
                            TILE_W - 16,
                            1,
                            DIM,
                            &entry.title().to_uppercase(),
                        );
                    }
                    if !in_recent && i == sel {
                        d.outline(x - 3, y - 3, TILE_W + 6, TILE_H + 6, 3, HILITE);
                    }
                }
            }

            draw_scrollbar(d, &grid, view.len(), top_row, scr_w);
        };

        if let (Some(path), true) = (&opts.shot, opts.max_frames == Some(frame_no)) {
            cab.capture_shelf(BG, render, path)?;
            return Ok(Pick::Quit);
        }
        // The shelf sits on the same tube as the game — the signal-off snow
        // stays faintly visible underneath it the whole time (plan §3.3),
        // not just during the entrance. Easing in from a fresh eject just
        // ramps *toward* that resting `SHELF_ALPHA` instead of straight to
        // fully opaque.
        // Back cover enlarged: it replaces the whole shelf while up.
        if zoom_close.is_some() {
            match focused {
                Some(e) => {
                    let bid = backcover_id(&e.rom.sha1);
                    let r = cab.frame_image_zoom(bid, "Fechar");
                    zoom_close = Some((r.x(), r.y(), r.width(), r.height()));
                }
                None => zoom_close = None,
            }
        } else {
            match opts.fade_in {
                Some(level) if fade_frame < FADE_IN_FRAMES => {
                    fade_frame += 1;
                    let alpha = (fade_frame as f32 / FADE_IN_FRAMES as f32) * SHELF_ALPHA;
                    cab.frame_shelf_fade_in(BG, render, level, alpha);
                }
                _ => cab.frame_shelf_fade_in(BG, render, idle::RESTING_STATIC, SHELF_ALPHA),
            }
        }

        crate::runner::pace_frame(&mut next, frame);
    }
}

/// A simple, scrollable ranking of every game with any recorded playtime,
/// most time played first (plan revision: "colocar um botao para historico
/// listando os jogos mais jogados em todo periodo") — the shelf's own
/// "Histórico" button. Clicking a row launches it, same as the shelf's own
/// grid; "Voltar" here means "back to the shelf", not all the way to idle
/// (the caller, `xperience.rs`'s main loop, treats this call's `Pick::Back`
/// that way, same as it already special-cases `Pick::Settings`).
pub fn run_history(plat: &mut Platform, cab: &mut Cabinet, catalog: &Catalog) -> Result<Pick> {
    let logo_dir = crate::dirs::assets_dir().join("logo");
    let cartridge_dir = crate::dirs::assets_dir().join("cartridge");
    let ranked = ranked_by_playtime(catalog)?;

    let mut sel: usize = 0;
    let mut top: usize = 0;
    let frame = Duration::from_millis(16);

    cab.set_shelf_panel(empty_shelf_panel());
    cab.set_close_button(true);

    loop {
        let (scr_w, scr_h) = cab.shelf_screen_size();
        let vis_rows = ((scr_h as i32 - MARGIN * 2 - HEADER_H) / HISTORY_ROW_H).max(1) as usize;
        if sel >= ranked.len() {
            sel = ranked.len().saturating_sub(1);
        }

        let m = plat.poll_menu(MenuMode::Nav);
        if m.quit {
            return Ok(Pick::Quit);
        }
        for nav in &m.nav {
            match nav {
                MenuNav::Up => sel = sel.saturating_sub(1),
                MenuNav::Down if sel + 1 < ranked.len() => sel += 1,
                MenuNav::PageUp => sel = sel.saturating_sub(vis_rows),
                MenuNav::PageDown => sel = (sel + vis_rows).min(ranked.len().saturating_sub(1)),
                MenuNav::Home => sel = 0,
                MenuNav::End => sel = ranked.len().saturating_sub(1),
                MenuNav::Back => return Ok(Pick::Back),
                MenuNav::Confirm => {
                    if let Some((e, _)) = ranked.get(sel) {
                        return Ok(pick_play(catalog, &logo_dir, &cartridge_dir, e));
                    }
                }
                _ => {}
            }
        }
        if sel < top {
            top = sel;
        } else if sel >= top + vis_rows {
            top = sel + 1 - vis_rows;
        }

        if let Some((x, y)) = m.click {
            let (ox, oy) = cab.window_to_output(x, y);
            if cab.hit_close_button(ox, oy) {
                return Ok(Pick::Quit);
            }
            if let Some(hit) = cab.hit_shelf_button(ox, oy) {
                match hit {
                    ShelfButton::Back => return Ok(Pick::Back),
                    ShelfButton::Settings => return Ok(Pick::Settings),
                    // The history screen's panel never has any per-game
                    // content, so it never scrolls (and never has a back
                    // cover to enlarge).
                    ShelfButton::Backcover => {}
                    ShelfButton::PanelScrollUp | ShelfButton::PanelScrollDown => {}
                }
            } else if let Some((lx, ly)) = cab.hit_screen_point(ox, oy) {
                if let Some(i) = history_row_at(lx, ly, top, ranked.len()) {
                    if i == sel {
                        let (e, _) = &ranked[i];
                        return Ok(pick_play(catalog, &logo_dir, &cartridge_dir, e));
                    }
                    sel = i;
                }
            }
        }

        let render =
            |d: &mut Screen| draw_history_list(d, &ranked, top, vis_rows, Some(sel), scr_w);
        cab.frame_shelf(BG, render);

        std::thread::sleep(frame);
    }
}

/// Headless preview of the history screen (dev/testing) — same setup as
/// `run_history`, one frame captured through the tube instead of a live loop.
pub fn capture_history_preview(cab: &mut Cabinet, catalog: &Catalog, path: &Path) -> Result<()> {
    let ranked = ranked_by_playtime(catalog)?;
    cab.set_shelf_panel(empty_shelf_panel());
    cab.set_close_button(true);
    let (scr_w, scr_h) = cab.shelf_screen_size();
    let vis_rows = ((scr_h as i32 - MARGIN * 2 - HEADER_H) / HISTORY_ROW_H).max(1) as usize;
    let render = |d: &mut Screen| draw_history_list(d, &ranked, 0, vis_rows, None, scr_w);
    cab.capture_shelf(BG, render, path)
        .map_err(|e| anyhow::anyhow!(e.to_string()))
}

/// Every scanned game with `total_playtime_secs > 0`, most time played
/// first — a game never (knowingly) turned on doesn't clutter a "most
/// played" ranking.
fn ranked_by_playtime(catalog: &Catalog) -> Result<Vec<(CatalogEntry, u64)>> {
    let saves_dir = crate::dirs::saves_dir();
    let mut ranked: Vec<(CatalogEntry, u64)> = catalog
        .list(Order::Name)?
        .into_iter()
        .map(|e| {
            let title = crate::runner::rom_title(Path::new(&e.rom.path));
            let secs = crate::runner::total_playtime_secs(&saves_dir, &title);
            (e, secs)
        })
        .filter(|(_, secs)| *secs > 0)
        .collect();
    ranked.sort_by_key(|(_, secs)| std::cmp::Reverse(*secs));
    Ok(ranked)
}

/// A blank flat panel — just the Voltar/Configuracoes buttons, no game
/// focused (the history list isn't a "selection" the way the shelf's grid
/// is; clicking a row launches straight away instead of just selecting it).
fn empty_shelf_panel() -> ShelfPanelInfo {
    ShelfPanelInfo {
        title: String::new(),
        logo_img: None,
        cartridge_img: None,
        backcover_img: None,
        release: None,
        info: Vec::new(),
        scroll: 0,
    }
}

const HISTORY_ROW_H: i32 = 28;

/// Which visible ranked row (if any) a screen-local point sits inside.
fn history_row_at(x: i32, y: i32, top: usize, len: usize) -> Option<usize> {
    let row_top = MARGIN + HEADER_H;
    if x < MARGIN || y < row_top {
        return None;
    }
    let i = top + ((y - row_top) / HISTORY_ROW_H) as usize;
    (i < len).then_some(i)
}

fn draw_history_list(
    d: &mut Screen,
    ranked: &[(CatalogEntry, u64)],
    top: usize,
    vis_rows: usize,
    sel: Option<usize>,
    scr_w: u32,
) {
    d.text(MARGIN, MARGIN - 12, 2, DIM, "histórico - mais jogados");
    if ranked.is_empty() {
        d.text_wrapped(
            MARGIN,
            MARGIN + 30,
            scr_w.saturating_sub(MARGIN as u32 * 2),
            1,
            DIM,
            "nenhum jogo tem tempo registrado ainda -- jogue algo com o console ligado.",
        );
        return;
    }
    let row_w = scr_w.saturating_sub(MARGIN as u32 * 2);
    for (i, (entry, secs)) in ranked.iter().enumerate().skip(top).take(vis_rows) {
        let y = MARGIN + HEADER_H + (i - top) as i32 * HISTORY_ROW_H;
        let selected = sel == Some(i);
        if selected {
            d.fill(MARGIN, y, row_w, HISTORY_ROW_H as u32 - 4, HILITE);
        }
        let color = if selected { HILITE_TEXT } else { TEXT };
        let label = format!("{:02}. {}", i + 1, entry.title().to_uppercase());
        d.text(MARGIN + 6, y + 4, 1, color, &label);
        let time_label = format_playtime(*secs);
        let tw = time_label.chars().count() as i32 * 9;
        d.text(MARGIN + row_w as i32 - tw - 6, y + 4, 1, color, &time_label);
    }
}

fn in_rect(x: i32, y: i32, (rx, ry, rw, rh): (i32, i32, u32, u32)) -> bool {
    x >= rx && y >= ry && x < rx + rw as i32 && y < ry + rh as i32
}

/// The title filter box (plan revision): shows the live draft with a text
/// cursor while typing, the applied query otherwise, or a placeholder when
/// empty — long text scrolls from the right so the cursor stays visible.
fn draw_filter_box(
    d: &mut Screen,
    (fx, fy, fw, fh): (i32, i32, u32, u32),
    query: &str,
    draft: &str,
    editing: bool,
) {
    d.outline(fx, fy, fw, fh, 1, (150, 150, 158, 255));
    d.fill(fx + 1, fy + 1, fw - 2, fh - 2, (34, 34, 40, 255));
    let (raw, color) = if editing {
        (format!("{draft}_"), TEXT)
    } else if !query.is_empty() {
        (query.to_string(), TEXT)
    } else {
        ("buscar...".to_string(), DIM)
    };
    let max_chars = ((fw as i32 - 16) / 9).max(4) as usize;
    let shown: String = if raw.chars().count() > max_chars {
        let tail: Vec<char> = raw.chars().rev().take(max_chars).collect();
        tail.into_iter().rev().collect()
    } else {
        raw
    };
    d.text(fx + 8, fy + (fh as i32 - 16) / 2, 1, color, &shown);
}

/// The "Histórico" button — same plain box look as the filter box, just a
/// static label (plan revision: "colocar um botão para histórico").
fn draw_history_button(d: &mut Screen, (bx, by, bw, bh): (i32, i32, u32, u32)) {
    d.outline(bx, by, bw, bh, 1, (150, 150, 158, 255));
    d.fill(bx + 1, by + 1, bw - 2, bh - 2, (34, 34, 40, 255));
    d.text(bx + 8, by + (bh as i32 - 16) / 2, 1, TEXT, "histórico");
}

/// A thin vertical scroll indicator at the grid's right edge (plan revision:
/// "mostrar rolagem na tv para visualizar os jogos") — only drawn once the
/// list actually overflows the visible rows.
fn draw_scrollbar(d: &mut Screen, grid: &GridLayout, view_len: usize, top_row: usize, scr_w: u32) {
    let total_rows = grid.total_rows(view_len);
    if total_rows <= grid.vis_rows {
        return;
    }
    let track_x = scr_w as i32 - MARGIN - SCROLLBAR_W as i32;
    let track_y = grid.y0;
    let track_h = (grid.vis_rows as i32 * grid.cell_h - GAP as i32).max(SCROLLBAR_W as i32);
    d.fill(
        track_x,
        track_y,
        SCROLLBAR_W,
        track_h as u32,
        (60, 60, 68, 255),
    );

    let thumb_h = ((grid.vis_rows as f32 / total_rows as f32) * track_h as f32)
        .max(16.0)
        .min(track_h as f32) as u32;
    let max_scroll = (total_rows - grid.vis_rows) as f32;
    let scroll_frac = if max_scroll > 0.0 {
        (top_row as f32 / max_scroll).clamp(0.0, 1.0)
    } else {
        0.0
    };
    let thumb_y = track_y + (scroll_frac * (track_h - thumb_h as i32).max(0) as f32) as i32;
    d.fill(track_x, thumb_y, SCROLLBAR_W, thumb_h, HILITE);
}

/// Decode a cover or logo PNG/JPEG to tightly-packed RGBA, downscaled for
/// memory (covers feed 200x150 tiles, logos a ~340px panel slot — 512 covers
/// both at >2x and keeps alpha for transparent logos).
fn decode_art(path: &Path) -> Result<(u32, u32, Vec<u8>)> {
    decode_art_scaled(path, 512)
}

/// Same decode, with an explicit size cap — the back cover's zoomed view
/// re-decodes at full resolution so the enlargement stays crisp.
fn decode_art_scaled(path: &Path, max: u32) -> Result<(u32, u32, Vec<u8>)> {
    let img = image::open(path)?.thumbnail(max, max).to_rgba8();
    let (w, h) = img.dimensions();
    Ok((w, h, img.into_raw()))
}

#[cfg(test)]
mod tests {
    use super::find_local_art;

    #[test]
    fn finds_art_for_variant_roms_via_the_base_name() {
        let dir = std::env::temp_dir().join(format!("shelf-art-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let base = dir.join("Killer Instinct (USA).jpg");
        std::fs::write(&base, b"jpg").unwrap();

        // Exact stem hits.
        let hit = find_local_art(&dir, "/x/Killer Instinct (USA).sfc").unwrap();
        assert_eq!(hit, base);
        // A variant ROM ("Rev 1") peels tags down to the base-named file.
        let hit = find_local_art(&dir, "/x/Killer Instinct (USA) (Rev 1).sfc").unwrap();
        assert_eq!(hit, base);

        // PNG wins over JPG for the same stem.
        std::fs::write(dir.join("Killer Instinct (USA).png"), b"png").unwrap();
        let hit = find_local_art(&dir, "/x/Killer Instinct (USA).sfc").unwrap();
        assert_eq!(hit.extension().unwrap(), "png");

        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn nothing_found_leaves_none() {
        let dir = std::env::temp_dir().join(format!("shelf-art-none-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        assert_eq!(find_local_art(&dir, "/x/Missing Game (USA).sfc"), None);
        // A stem that only shares a prefix must not match anything.
        std::fs::write(dir.join("Game (USA).jpg"), b"x").unwrap();
        assert_eq!(find_local_art(&dir, "/x/Gam.sfc"), None);
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
