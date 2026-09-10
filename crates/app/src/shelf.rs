//! The selector shelf, factored out of the `selector` binary so `xperience` can
//! show it between games: a scrollable grid of covers with a details panel,
//! gamepad-first navigation and type-to-search. Covers stream in on a background
//! thread ("preenchimento progressivo", plan §3.1).

use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::time::{Duration, Instant};

use anyhow::{anyhow, bail, Result};
use xperience_domain::{Catalog, CatalogEntry, Order};
use xperience_platform::{MenuNav, Platform};

const TILE_W: u32 = 150;
const TILE_H: u32 = 200;
const GAP: u32 = 18;
const MARGIN: i32 = 28;
const PANEL_W: u32 = 380;

const BG: (u8, u8, u8) = (18, 18, 20);
const TILE_BG: (u8, u8, u8, u8) = (34, 34, 40, 255);
const HILITE: (u8, u8, u8, u8) = (240, 200, 80, 255);
const TEXT: (u8, u8, u8) = (232, 232, 232);
const DIM: (u8, u8, u8) = (150, 150, 158);

/// What the player did on the shelf.
pub enum Pick {
    /// Launch this ROM (already marked played in the catalogue).
    Play(PathBuf),
    /// Cancelled — quit the app.
    Quit,
}

/// Knobs for [`run`].
pub struct ShelfOpts {
    pub order: Order,
    /// Headless smoke test: stop after N frames and return [`Pick::Quit`].
    pub max_frames: Option<u64>,
}

impl Default for ShelfOpts {
    fn default() -> Self {
        Self {
            order: Order::Shelf,
            max_frames: None,
        }
    }
}

struct DecodedCover {
    id: u64,
    w: u32,
    h: u32,
    rgba: Vec<u8>,
}

fn cover_id(sha1: &str) -> u64 {
    u64::from_str_radix(sha1.get(..16).unwrap_or("0"), 16).unwrap_or(0)
}

/// Show the shelf on `plat` until the player picks a game or cancels. The `plat`
/// outlives the call; the shelf window is created and dropped inside.
pub fn run(plat: &mut Platform, catalog: &Catalog, opts: &ShelfOpts) -> Result<Pick> {
    let all = catalog.list(opts.order)?;
    if all.is_empty() {
        bail!("catalogue is empty — run:  library scan --roms <dir>");
    }

    // Background cover decoder.
    let (tx, rx) = mpsc::channel::<DecodedCover>();
    {
        let jobs: Vec<(u64, PathBuf)> = all
            .iter()
            .filter_map(|e| {
                let p = e.meta.as_ref()?.cover_path.clone()?;
                Path::new(&p)
                    .is_file()
                    .then(|| (cover_id(&e.rom.sha1), PathBuf::from(p)))
            })
            .collect();
        std::thread::spawn(move || {
            for (id, path) in jobs {
                match decode_cover(&path) {
                    Ok((w, h, rgba)) => {
                        let _ = tx.send(DecodedCover { id, w, h, rgba });
                    }
                    Err(e) => log::warn!("cover {}: {e}", path.display()),
                }
            }
        });
    }

    let mut ui = plat
        .create_ui_window("SNES Xperience", 1280, 800)
        .map_err(|e| anyhow!(e.to_string()))?;

    let mut sel: usize = 0;
    let mut top_row: usize = 0;
    let mut search = String::new();
    let frame = Duration::from_millis(16);
    let mut frame_no = 0u64;

    loop {
        let started = Instant::now();
        frame_no += 1;
        if opts.max_frames.is_some_and(|n| frame_no > n) {
            return Ok(Pick::Quit);
        }

        // Drain decoded covers.
        while let Ok(c) = rx.try_recv() {
            ui.set_image(c.id, c.w, c.h, &c.rgba);
        }

        // Current (filtered) view.
        let view: Vec<&CatalogEntry> = if search.is_empty() {
            all.iter().collect()
        } else {
            let q = search.to_lowercase();
            all.iter()
                .filter(|e| e.title().to_lowercase().contains(&q))
                .collect()
        };
        if sel >= view.len() {
            sel = view.len().saturating_sub(1);
        }

        let (win_w, win_h) = ui.size();
        let grid_w = win_w.saturating_sub(PANEL_W + MARGIN as u32 * 2);
        let cols = (grid_w / (TILE_W + GAP)).max(1) as usize;
        let vis_rows = ((win_h as i32 - MARGIN * 2 - 40) / (TILE_H + GAP) as i32).max(1) as usize;

        // Input.
        let m = plat.poll_menu();
        if m.quit {
            return Ok(Pick::Quit);
        }
        if m.toggle_fullscreen {
            ui.toggle_fullscreen();
        }
        if m.backspace {
            search.pop();
            sel = 0;
        }
        if !m.typed.is_empty() {
            search.push_str(&m.typed);
            sel = 0;
        }
        for nav in m.nav {
            match nav {
                MenuNav::Left => sel = sel.saturating_sub(1),
                MenuNav::Right if sel + 1 < view.len() => sel += 1,
                MenuNav::Up => sel = sel.saturating_sub(cols),
                MenuNav::Down if sel + cols < view.len() => sel += cols,
                MenuNav::PageUp => sel = sel.saturating_sub(cols * vis_rows),
                MenuNav::PageDown => {
                    sel = (sel + cols * vis_rows).min(view.len().saturating_sub(1))
                }
                MenuNav::Home => sel = 0,
                MenuNav::End => sel = view.len().saturating_sub(1),
                MenuNav::Back => {
                    if search.is_empty() {
                        return Ok(Pick::Quit);
                    }
                    search.clear();
                    sel = 0;
                }
                MenuNav::Confirm => {
                    if let Some(e) = view.get(sel) {
                        let _ = catalog.mark_played(&e.rom.sha1);
                        return Ok(Pick::Play(PathBuf::from(&e.rom.path)));
                    }
                }
                _ => {}
            }
        }

        // Keep selection visible.
        let sel_row = sel.checked_div(cols).unwrap_or(0);
        if sel_row < top_row {
            top_row = sel_row;
        } else if sel_row >= top_row + vis_rows {
            top_row = sel_row + 1 - vis_rows;
        }

        // --- draw --------------------------------------------------------
        ui.begin(BG);

        // Search line.
        let label = if search.is_empty() {
            format!("{} games — type to search", view.len())
        } else {
            format!("search: {search}_   ({} match)", view.len())
        };
        ui.text(MARGIN, MARGIN - 12, 2, DIM, &label);

        let grid_x0 = MARGIN;
        let grid_y0 = MARGIN + 28;
        for (i, entry) in view.iter().enumerate() {
            let row = i / cols;
            if row < top_row || row >= top_row + vis_rows {
                continue;
            }
            let col = i % cols;
            let x = grid_x0 + col as i32 * (TILE_W + GAP) as i32;
            let y = grid_y0 + (row - top_row) as i32 * (TILE_H + GAP) as i32;

            ui.fill(x, y, TILE_W, TILE_H, TILE_BG);
            let id = cover_id(&entry.rom.sha1);
            if ui.has_image(id) {
                ui.image_fit(id, x + 4, y + 4, TILE_W - 8, TILE_H - 8);
            } else {
                ui.text_wrapped(
                    x + 8,
                    y + 10,
                    TILE_W - 16,
                    1,
                    DIM,
                    &entry.title().to_uppercase(),
                );
            }
            if i == sel {
                ui.outline(x - 3, y - 3, TILE_W + 6, TILE_H + 6, 3, HILITE);
            }
        }

        // --- details panel ---------------------------------------------
        let px = (win_w - PANEL_W) as i32;
        ui.fill(px, 0, PANEL_W, win_h, (24, 24, 28, 255));
        if let Some(e) = view.get(sel) {
            let ix = px + 22;
            let mut iy = MARGIN;
            iy = ui.text_wrapped(ix, iy, PANEL_W - 44, 2, TEXT, &e.title()) + 8;

            let m = e.meta.as_ref();
            let plays = e.rom.play_count.to_string();
            let rows: [(&str, Option<&str>); 7] = [
                ("year", m.and_then(|m| m.year.as_deref())),
                ("developer", m.and_then(|m| m.developer.as_deref())),
                ("publisher", m.and_then(|m| m.publisher.as_deref())),
                ("genre", m.and_then(|m| m.genre.as_deref())),
                ("players", m.and_then(|m| m.players.as_deref())),
                ("region", m.and_then(|m| m.region.as_deref())),
                ("plays", (e.rom.play_count > 0).then_some(plays.as_str())),
            ];
            for (k, v) in rows {
                if let Some(v) = v {
                    ui.text(ix, iy, 1, DIM, k);
                    ui.text(ix + 90, iy, 1, TEXT, v);
                    iy += 16;
                }
            }
            iy += 10;
            if let Some(s) = m.and_then(|m| m.synopsis.as_deref()) {
                ui.text_wrapped(ix, iy, PANEL_W - 44, 1, DIM, s);
            } else if e.meta.is_none() {
                ui.text(ix, iy, 1, DIM, "not scraped yet");
            }
        }
        ui.text(
            px + 22,
            win_h as i32 - 30,
            1,
            DIM,
            "A / Enter: play   B / Esc: quit",
        );

        ui.present();

        let elapsed = started.elapsed();
        if elapsed < frame {
            std::thread::sleep(frame - elapsed);
        }
    }
}

fn decode_cover(path: &Path) -> Result<(u32, u32, Vec<u8>)> {
    let img = image::open(path)?;
    // Downscale for memory; the tile is small and sampled linearly.
    let img = img.thumbnail(320, 420).to_rgba8();
    let (w, h) = img.dimensions();
    Ok((w, h, img.into_raw()))
}
