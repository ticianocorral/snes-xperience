//! The selector shelf, factored out of the `selector` binary so `xperience` can
//! show it between games: a scrollable grid of covers with a details panel,
//! gamepad-first navigation and type-to-search. Covers stream in on a background
//! thread ("preenchimento progressivo", plan §3.1), and — when ScreenScraper
//! credentials are present — the game you rest on is scraped on the spot.

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::time::{Duration, Instant};

use anyhow::{anyhow, bail, Result};
use xperience_domain::{
    download_art, ArtPaths, Catalog, CatalogEntry, Client, Credentials, GameInfo, Order, RomId,
    ScrapeError,
};
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

/// Frames the selection must sit still on an unscraped game before we fetch it
/// (~130 ms at 60 fps) — so fast scrolling doesn't queue the whole shelf.
const SCRAPE_DWELL_FRAMES: u32 = 8;
/// Politeness gap between ScreenScraper calls on the worker thread.
const SCRAPE_GAP: Duration = Duration::from_millis(700);
/// Frames to hold a long synopsis still before it starts auto-scrolling (~1.3 s).
const SYNOPSIS_HOLD_FRAMES: u32 = 80;

/// What the player did on the shelf.
pub enum Pick {
    /// Launch this ROM (already marked played in the catalogue).
    Play(PathBuf),
    /// Cancelled — quit the app.
    Quit,
}

/// ScreenScraper access for on-demand metadata.
pub struct ScrapeSetup {
    pub creds: Credentials,
    /// Where downloaded art lands (`<dir>/<sha1>-<kind>.png`).
    pub art_dir: PathBuf,
}

/// Knobs for [`run`].
pub struct ShelfOpts {
    pub order: Order,
    /// Headless smoke test: stop after N frames and return [`Pick::Quit`].
    pub max_frames: Option<u64>,
    /// On-demand scrape of the focused game. `None` disables it.
    pub scrape: Option<ScrapeSetup>,
}

impl Default for ShelfOpts {
    fn default() -> Self {
        Self {
            order: Order::Shelf,
            max_frames: None,
            scrape: None,
        }
    }
}

struct DecodedCover {
    id: u64,
    w: u32,
    h: u32,
    rgba: Vec<u8>,
}

/// A ROM the shelf asked the worker to scrape.
struct ScrapeJob {
    sha1: String,
    path: PathBuf,
}

/// What the worker sends back for one job.
enum ScrapeMsg {
    Done {
        sha1: String,
        info: Box<GameInfo>,
        art: ArtPaths,
    },
    /// Not in ScreenScraper, file changed, or a transient error — don't retry.
    Missing,
    /// Daily quota hit; the worker has stopped.
    Quota,
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

/// Show the shelf on `plat` until the player picks a game or cancels. The `plat`
/// outlives the call; the shelf window is created and dropped inside.
pub fn run(plat: &mut Platform, catalog: &Catalog, opts: &ShelfOpts) -> Result<Pick> {
    let mut all = catalog.list(opts.order)?;
    if all.is_empty() {
        bail!("catalogue is empty — run:  library scan --roms <dir>");
    }

    // Background art decoder — a fed queue so freshly-scraped covers/wheels join.
    let (art_tx, art_rx) = mpsc::channel::<(u64, PathBuf)>();
    let (decoded_tx, decoded_rx) = mpsc::channel::<DecodedCover>();
    for e in &all {
        let Some(m) = e.meta.as_ref() else { continue };
        for (id, path) in [
            (cover_id(&e.rom.sha1), m.cover_path.clone()),
            (wheel_id(&e.rom.sha1), m.wheel_path.clone()),
        ] {
            if let Some(p) = path.filter(|p| Path::new(p).is_file()) {
                let _ = art_tx.send((id, PathBuf::from(p)));
            }
        }
    }
    std::thread::spawn(move || {
        while let Ok((id, path)) = art_rx.recv() {
            match decode_art(&path) {
                Ok((w, h, rgba)) => {
                    let _ = decoded_tx.send(DecodedCover { id, w, h, rgba });
                }
                Err(e) => log::warn!("art {}: {e}", path.display()),
            }
        }
    });

    // Background scraper (only if we have credentials).
    let (job_tx, job_rx) = mpsc::channel::<ScrapeJob>();
    let (scraped_tx, scraped_rx) = mpsc::channel::<ScrapeMsg>();
    let scrape_enabled = opts.scrape.is_some();
    if let Some(setup) = &opts.scrape {
        let creds = setup.creds.clone();
        let art_dir = setup.art_dir.clone();
        std::thread::spawn(move || scrape_worker(creds, art_dir, job_rx, scraped_tx));
    }

    let mut ui = plat
        .create_ui_window("SNES Xperience", 1280, 800)
        .map_err(|e| anyhow!(e.to_string()))?;

    let mut sel: usize = 0;
    let mut top_row: usize = 0;
    let mut search = String::new();
    let frame = Duration::from_millis(16);
    let mut frame_no = 0u64;

    // Scrape bookkeeping: never ask twice, notice when the selection settles.
    let mut requested: HashSet<String> = all
        .iter()
        .filter(|e| e.meta.is_some())
        .map(|e| e.rom.sha1.clone())
        .collect();
    let mut quota_hit = false;
    // Frames the selection has sat still (drives on-demand scrape + synopsis scroll).
    let mut dwell: u32 = 0;
    let mut dwell_sha1: Option<String> = None;
    let mut synopsis_scroll: i32 = 0;

    loop {
        let started = Instant::now();
        frame_no += 1;
        if opts.max_frames.is_some_and(|n| frame_no > n) {
            return Ok(Pick::Quit);
        }

        // Drain decoded covers.
        while let Ok(c) = decoded_rx.try_recv() {
            ui.set_image(c.id, c.w, c.h, &c.rgba);
        }

        // Drain scrape results; a hit rewrites the catalogue and the view.
        let mut refresh = false;
        while let Ok(msg) = scraped_rx.try_recv() {
            match msg {
                ScrapeMsg::Done { sha1, info, art } => {
                    if let Err(e) = catalog.set_meta(&sha1, &info, &art) {
                        log::warn!("catalogue set_meta {sha1}: {e}");
                    }
                    if let Some(cover) = &art.cover {
                        let _ = art_tx.send((cover_id(&sha1), PathBuf::from(cover)));
                    }
                    if let Some(wheel) = &art.wheel {
                        let _ = art_tx.send((wheel_id(&sha1), PathBuf::from(wheel)));
                    }
                    refresh = true;
                }
                ScrapeMsg::Missing => {}
                ScrapeMsg::Quota => {
                    quota_hit = true;
                    log::warn!("ScreenScraper quota exhausted — on-demand scrape paused");
                }
            }
        }
        if refresh {
            all = catalog.list(opts.order)?;
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

        // How long has the selection sat on this game?
        let cur = view.get(sel).map(|e| e.rom.sha1.clone());
        if cur == dwell_sha1 {
            dwell += 1;
        } else {
            dwell = 0;
            dwell_sha1 = cur;
            synopsis_scroll = 0;
        }

        // On-demand scrape: once it has rested on an unscraped game.
        if scrape_enabled && !quota_hit && dwell == SCRAPE_DWELL_FRAMES {
            if let Some(e) = view.get(sel) {
                if e.meta.is_none() && requested.insert(e.rom.sha1.clone()) {
                    let _ = job_tx.send(ScrapeJob {
                        sha1: e.rom.sha1.clone(),
                        path: PathBuf::from(&e.rom.path),
                    });
                }
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
            let inner_w = PANEL_W - 44;
            let mut iy = MARGIN;

            // Header: the wheel logo if we have it, else the title in text.
            let wid = wheel_id(&e.rom.sha1);
            if ui.has_image(wid) {
                ui.image_fit(wid, ix, iy, inner_w, 72);
                iy += 72 + 12;
            } else {
                iy = ui.text_wrapped(ix, iy, inner_w, 2, TEXT, &e.title()) + 8;
            }

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
                // Scrollable region between the ficha and the footer. After a
                // short rest it creeps upward until the end is visible.
                let vp_y = iy;
                let vp_h = (win_h as i32 - 40 - vp_y).max(0);
                if vp_h > 12 {
                    let overflow = (ui.wrapped_height(inner_w, 1, s) - vp_h).max(0);
                    if dwell > SYNOPSIS_HOLD_FRAMES {
                        synopsis_scroll = (synopsis_scroll + 1).min(overflow);
                    }
                    ui.clip(Some((px, vp_y, PANEL_W, vp_h as u32)));
                    ui.text_wrapped(ix, vp_y - synopsis_scroll, inner_w, 1, DIM, s);
                    ui.clip(None);
                }
            } else if e.meta.is_none() {
                let note = if quota_hit {
                    "scrape quota reached"
                } else if scrape_enabled && requested.contains(&e.rom.sha1) {
                    "scraping\u{2026}"
                } else if scrape_enabled {
                    "not scraped yet"
                } else {
                    "not scraped (no credentials)"
                };
                ui.text(ix, iy, 1, DIM, note);
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

/// Pull jobs off `jobs` until the channel closes, scraping each and reporting
/// back on `out`. Stops for good on a quota response.
fn scrape_worker(
    creds: Credentials,
    art_dir: PathBuf,
    jobs: mpsc::Receiver<ScrapeJob>,
    out: mpsc::Sender<ScrapeMsg>,
) {
    let client = Client::new(creds);
    while let Ok(job) = jobs.recv() {
        let filename = job
            .path
            .file_name()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_default();
        let id = match RomId::from_path(&job.path) {
            Ok(id) if id.sha1 == job.sha1 => id,
            Ok(_) => {
                log::info!("scrape {filename}: file changed on disk");
                let _ = out.send(ScrapeMsg::Missing);
                continue;
            }
            Err(e) => {
                log::warn!("scrape {filename}: {e}");
                let _ = out.send(ScrapeMsg::Missing);
                continue;
            }
        };
        match client.lookup(&id, &filename) {
            Ok(info) => {
                let art = download_art(&client, &art_dir, &job.sha1, &info);
                let _ = out.send(ScrapeMsg::Done {
                    sha1: job.sha1,
                    info: Box::new(info),
                    art,
                });
            }
            Err(ScrapeError::QuotaExhausted) => {
                let _ = out.send(ScrapeMsg::Quota);
                return;
            }
            Err(e) => {
                log::info!("scrape {filename}: {e}");
                let _ = out.send(ScrapeMsg::Missing);
            }
        }
        std::thread::sleep(SCRAPE_GAP);
    }
}

/// Decode a cover or wheel PNG/JPEG to tightly-packed RGBA, downscaled for
/// memory (covers feed 150px tiles, wheels a ~340px panel slot — 512 covers both
/// at >2x and keeps alpha for the transparent wheels).
fn decode_art(path: &Path) -> Result<(u32, u32, Vec<u8>)> {
    let img = image::open(path)?.thumbnail(512, 512).to_rgba8();
    let (w, h) = img.dimensions();
    Ok((w, h, img.into_raw()))
}
