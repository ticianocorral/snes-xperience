//! The selector shelf, factored out of the `selector` binary so `xperience` can
//! show it between games: a scrollable grid of covers (or a multicart-style
//! list when no cover art is around) with a details panel, mouse and gamepad
//! navigation only (plan revision: no keyboard shortcuts, so type-to-search
//! is gone too — nothing left to type with). Cover/logo art is local —
//! dropped by hand into `assets/cover/`/`assets/logo/` (plan §4.3, no more
//! ScreenScraper) — matched by the ROM's file name and decoded lazily as
//! tiles scroll into view.

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use anyhow::Result;
use xperience_domain::{Catalog, CatalogEntry, Order};
use xperience_platform::{Cabinet, MenuMode, MenuNav, Platform, Screen};

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
const PANEL_W: u32 = 380;

/// Row height when the shelf falls back to a plain text list (no cover art
/// loaded at all, plan §3.1) — a multicart-style menu instead of a grid of
/// empty tiles.
const LIST_ROW_H: u32 = 22;
const LIST_GAP: u32 = 6;

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
    /// Clicked "Configuracoes" — open the settings screen, then come back to
    /// the shelf.
    Settings,
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
}

impl Default for ShelfOpts {
    fn default() -> Self {
        Self {
            order: Order::Shelf,
            max_frames: None,
            shot: None,
            fade_in: None,
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

/// `dir/<rom's file stem>.{png,jpg,jpeg}`, in that order — the convention for
/// locally-supplied art: `roms/Aladdin.sfc` matches `assets/cover/Aladdin.png`.
fn find_local_art(dir: &Path, rom_path: &str) -> Option<PathBuf> {
    let stem = Path::new(rom_path).file_stem()?.to_str()?;
    ["png", "jpg", "jpeg"]
        .into_iter()
        .map(|ext| dir.join(format!("{stem}.{ext}")))
        .find(|p| p.is_file())
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
    fn new(scr_w: u32, scr_h: u32, list_mode: bool) -> Self {
        let grid_w = scr_w.saturating_sub(PANEL_W + MARGIN as u32 * 2);
        let (cell_w, cell_h, item_w, item_h, cols) = if list_mode {
            let cell_h = (LIST_ROW_H + LIST_GAP) as i32;
            (grid_w as i32, cell_h, grid_w as i32, LIST_ROW_H as i32, 1)
        } else {
            let cell_w = (TILE_W + GAP) as i32;
            let cell_h = (TILE_H + GAP) as i32;
            let cols = (grid_w / (TILE_W + GAP)).max(1) as usize;
            (cell_w, cell_h, TILE_W as i32, TILE_H as i32, cols)
        };
        let vis_rows = ((scr_h as i32 - MARGIN * 2 - 40) / cell_h).max(1) as usize;
        Self {
            x0: MARGIN,
            y0: MARGIN + 28,
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
}

/// `roms/` has nothing in it — a friendlier landing than a hard error, since
/// an empty ROMs folder is the expected first-launch state for a portable,
/// autoexecutável app, not a misconfiguration.
fn empty_roms_screen(plat: &mut Platform, cab: &mut Cabinet) -> Result<Pick> {
    let frame = Duration::from_millis(16);
    loop {
        let m = plat.poll_menu(MenuMode::Nav);
        if m.quit {
            return Ok(Pick::Quit);
        }
        if m.nav.contains(&MenuNav::Back) {
            return Ok(Pick::Back);
        }
        let (scr_w, scr_h) = cab.screen_size();
        let back_rect = (MARGIN, scr_h as i32 - MARGIN - 36, 160u32, 32u32);
        if let Some((x, y)) = m.click {
            let (ox, oy) = cab.window_to_output(x, y);
            if let Some((lx, ly)) = cab.hit_screen_point(ox, oy) {
                if in_rect(lx, ly, back_rect) {
                    return Ok(Pick::Back);
                }
            }
        }
        let render = |d: &mut Screen| {
            d.text(MARGIN, MARGIN, 2, TEXT, "nenhuma rom encontrada");
            d.text_wrapped(
                MARGIN,
                MARGIN + 40,
                scr_w.saturating_sub(MARGIN as u32 * 2),
                1,
                DIM,
                "copie seus arquivos .sfc/.smc para a pasta roms/, ao lado do \
                 executavel, e volte para esta tela.",
            );
            draw_panel_button(d, back_rect, "Voltar");
        };
        cab.frame_2d(BG, render);
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
    // Never re-stat a game's art more than once per shelf visit — most games
    // won't have any, and disk isn't free even if it's cheap.
    let mut tried_cover: HashSet<String> = HashSet::new();
    let mut tried_logo: HashSet<String> = HashSet::new();

    let mut sel: usize = 0;
    let mut top_row: usize = 0;
    let frame = Duration::from_millis(16);
    let mut frame_no = 0u64;

    // Frames left in the "entering over the static" ease-in (§3.3), if any.
    const FADE_IN_FRAMES: u32 = 18;
    let mut fade_frame: u32 = 0;

    loop {
        let started = Instant::now();
        frame_no += 1;
        if opts.max_frames.is_some_and(|n| frame_no > n) {
            return Ok(Pick::Quit);
        }

        let view: Vec<&CatalogEntry> = all.iter().collect();
        if sel >= view.len() {
            sel = view.len().saturating_sub(1);
        }

        let (scr_w, scr_h) = cab.screen_size();
        // No cover art loaded anywhere in view yet: a text list (multicart
        // menu) reads as deliberate, where a grid of empty tiles reads as
        // broken (plan §3.1).
        let list_mode = !view.iter().any(|e| cab.has_image(cover_id(&e.rom.sha1)));
        let grid = GridLayout::new(scr_w, scr_h, list_mode);

        // Input.
        let m = plat.poll_menu(MenuMode::Nav);
        if m.quit {
            return Ok(Pick::Quit);
        }
        for nav in m.nav {
            match nav {
                MenuNav::Left => sel = sel.saturating_sub(1),
                MenuNav::Right if sel + 1 < view.len() => sel += 1,
                MenuNav::Up => sel = sel.saturating_sub(grid.cols),
                MenuNav::Down if sel + grid.cols < view.len() => sel += grid.cols,
                MenuNav::PageUp => sel = sel.saturating_sub(grid.cols * grid.vis_rows),
                MenuNav::PageDown => {
                    sel = (sel + grid.cols * grid.vis_rows).min(view.len().saturating_sub(1))
                }
                MenuNav::Home => sel = 0,
                MenuNav::End => sel = view.len().saturating_sub(1),
                MenuNav::Back => return Ok(Pick::Back),
                MenuNav::Confirm => {
                    if let Some(e) = view.get(sel) {
                        return Ok(pick_play(catalog, &logo_dir, &cartridge_dir, e));
                    }
                }
                _ => {}
            }
        }

        // Mouse: click a tile to select it, click the already-selected one
        // to launch — the same two-step a controller does (move, then A).
        // The panel's "Configuracoes"/"Voltar" buttons are the only way into
        // either without a gamepad now (plan revision: no keyboard).
        if let Some((x, y)) = m.click {
            let (ox, oy) = cab.window_to_output(x, y);
            if let Some((lx, ly)) = cab.hit_screen_point(ox, oy) {
                if let Some(i) = grid.tile_at(lx, ly, top_row, view.len()) {
                    if i == sel {
                        if let Some(e) = view.get(i) {
                            return Ok(pick_play(catalog, &logo_dir, &cartridge_dir, e));
                        }
                    } else {
                        sel = i;
                    }
                } else if in_rect(lx, ly, settings_btn_rect(scr_w, scr_h)) {
                    return Ok(Pick::Settings);
                } else if in_rect(lx, ly, back_btn_rect(scr_w, scr_h)) {
                    return Ok(Pick::Back);
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
        // worker thread, so this can just happen inline. Each sha1 is tried
        // at most once per visit, whether or not a file turns up.
        for (i, entry) in view.iter().enumerate() {
            if grid.cell_pos(i, top_row).is_none() {
                continue;
            }
            let id = cover_id(&entry.rom.sha1);
            if !cab.has_image(id) && tried_cover.insert(entry.rom.sha1.clone()) {
                if let Some(path) = find_local_art(&cover_dir, &entry.rom.path) {
                    if let Ok((w, h, rgba)) = decode_art(&path) {
                        cab.set_image(id, w, h, &rgba);
                    }
                }
            }
        }
        if let Some(e) = view.get(sel) {
            let id = wheel_id(&e.rom.sha1);
            if !cab.has_image(id) && tried_logo.insert(e.rom.sha1.clone()) {
                if let Some(path) = find_local_art(&logo_dir, &e.rom.path) {
                    if let Ok((w, h, rgba)) = decode_art(&path) {
                        cab.set_image(id, w, h, &rgba);
                    }
                }
            }
        }

        // --- draw (into a screen-sized buffer, then warped through the tube) --
        let render = |d: &mut Screen| {
            d.text(
                MARGIN,
                MARGIN - 12,
                2,
                DIM,
                &format!("{} games", view.len()),
            );

            if grid.list_mode {
                // Multicart menu: a plain numbered list, selection as an
                // inverted bar — no tile, no art, nothing pretending there's
                // a cover coming.
                for (i, entry) in view.iter().enumerate() {
                    let Some((x, y)) = grid.cell_pos(i, top_row) else {
                        continue;
                    };
                    let selected = i == sel;
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
                    if i == sel {
                        d.outline(x - 3, y - 3, TILE_W + 6, TILE_H + 6, 3, HILITE);
                    }
                }
            }

            // --- details panel ---------------------------------------------
            let px = (scr_w - PANEL_W) as i32;
            d.fill(px, 0, PANEL_W, scr_h, (24, 24, 28, 255));
            if let Some(e) = view.get(sel) {
                let ix = px + 22;
                let inner_w = PANEL_W - 44;
                let mut iy = MARGIN;

                // Header: the local logo if we have it, else the title in text.
                let wid = wheel_id(&e.rom.sha1);
                if d.has_image(wid) {
                    d.image_fit(wid, ix, iy, inner_w, 72);
                    iy += 72 + 12;
                } else {
                    iy = d.text_wrapped(ix, iy, inner_w, 2, TEXT, &e.title()) + 8;
                }

                if e.rom.play_count > 0 {
                    d.text(ix, iy, 1, DIM, "plays");
                    d.text(ix + 90, iy, 1, TEXT, &e.rom.play_count.to_string());
                }
            }

            // Buttons — mouse's only way into either without a gamepad (plan
            // revision: no keyboard). A gamepad still confirms/backs out
            // with South/East anywhere on this screen, tile grid included.
            draw_panel_button(d, back_btn_rect(scr_w, scr_h), "Voltar");
            draw_panel_button(d, settings_btn_rect(scr_w, scr_h), "Configuracoes");
        };

        if let (Some(path), true) = (&opts.shot, opts.max_frames == Some(frame_no)) {
            cab.capture_2d(BG, render, path)?;
            return Ok(Pick::Quit);
        }
        // The shelf sits on the same tube as the game — the signal-off snow
        // stays faintly visible underneath it the whole time (plan §3.3),
        // not just during the entrance. Easing in from a fresh eject just
        // ramps *toward* that resting `SHELF_ALPHA` instead of straight to
        // fully opaque.
        match opts.fade_in {
            Some(level) if fade_frame < FADE_IN_FRAMES => {
                fade_frame += 1;
                let alpha = (fade_frame as f32 / FADE_IN_FRAMES as f32) * SHELF_ALPHA;
                cab.frame_2d_fade_in(BG, render, level, alpha);
            }
            _ => cab.frame_2d_fade_in(BG, render, idle::RESTING_STATIC, SHELF_ALPHA),
        }

        let elapsed = started.elapsed();
        if elapsed < frame {
            std::thread::sleep(frame - elapsed);
        }
    }
}

/// Bottom of the details panel: two stacked buttons, "Voltar" above
/// "Configuracoes" — the shelf's only mouse path into either (plan revision:
/// no keyboard shortcuts left to reach them by).
fn back_btn_rect(scr_w: u32, scr_h: u32) -> (i32, i32, u32, u32) {
    let px = (scr_w - PANEL_W) as i32;
    (px + 22, scr_h as i32 - MARGIN - 74, PANEL_W - 44, 32)
}

fn settings_btn_rect(scr_w: u32, scr_h: u32) -> (i32, i32, u32, u32) {
    let px = (scr_w - PANEL_W) as i32;
    (px + 22, scr_h as i32 - MARGIN - 36, PANEL_W - 44, 32)
}

fn in_rect(x: i32, y: i32, (rx, ry, rw, rh): (i32, i32, u32, u32)) -> bool {
    x >= rx && y >= ry && x < rx + rw as i32 && y < ry + rh as i32
}

fn draw_panel_button(d: &mut Screen, (x, y, w, h): (i32, i32, u32, u32), label: &str) {
    d.outline(x, y, w, h, 1, (150, 150, 158, 255));
    d.fill(x + 1, y + 1, w - 2, h - 2, (34, 34, 40, 255));
    d.text(x + 10, y + (h as i32 - 16) / 2, 1, TEXT, label);
}

/// Decode a cover or logo PNG/JPEG to tightly-packed RGBA, downscaled for
/// memory (covers feed 200x150 tiles, logos a ~340px panel slot — 512 covers
/// both at >2x and keeps alpha for transparent logos).
fn decode_art(path: &Path) -> Result<(u32, u32, Vec<u8>)> {
    let img = image::open(path)?.thumbnail(512, 512).to_rgba8();
    let (w, h) = img.dimensions();
    Ok((w, h, img.into_raw()))
}
