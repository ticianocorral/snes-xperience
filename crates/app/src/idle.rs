//! The idle/root screen: TV off, no game loaded — the app's home state, and
//! also literally the "cartridge ejected" screen (plan revision: one screen,
//! not two — startup, backing out of the shelf, and ejecting a game all land
//! here). In place of a game's logo/cartridge art there's the console's own
//! brand logo (`assets/console.png`, optional) and an "Inserir cartucho"
//! button; "Configurações" sits in the panel's footer. This is the outermost
//! screen now: a gamepad's Back button here, same as closing the window,
//! ends the app (plan revision: mouse/gamepad only, no keyboard shortcuts).

use std::path::Path;
use std::sync::mpsc::{Receiver, TryRecvError};

use anyhow::Result;
use xperience_platform::{Cabinet, MenuMode, MenuNav, PanelButton, Platform, Screen};

use crate::update_check::UpdateNotice;

const NOTICE_BG: (u8, u8, u8) = (18, 18, 20);
const NOTICE_TEXT: (u8, u8, u8) = (232, 232, 232);
const NOTICE_DIM: (u8, u8, u8) = (150, 150, 158);
const NOTICE_MARGIN: i32 = 40;

/// The screen's resting static level — startup and "Esc on the shelf" have no
/// prior game to inherit a level from, so they use this (same dim hiss the
/// console settles on after powering off, `runner::OFF_STATIC_LEVEL`).
pub const RESTING_STATIC: f32 = crate::runner::OFF_STATIC_LEVEL;

/// The app's own default idle-screen logo, baked into the binary so it shows
/// out of the box — a real `assets/console.png` still overrides it, same
/// "local file wins over anything built in" convention per-game art already
/// follows (cover/logo/cartridge).
const DEFAULT_CONSOLE_LOGO: &[u8] = include_bytes!("../assets/console_logo.png");

/// What the player did on the idle screen.
pub enum IdleExit {
    /// Window closed / Cmd-Q, or Esc — this is the root screen, so both end
    /// the app.
    Quit,
    /// "Inserir cartucho" confirmed (Enter/gamepad) or clicked.
    OpenShelf,
    /// "Configurações" clicked — the only way into settings from here (no
    /// keyboard shortcut, mouse/gamepad only).
    OpenSettings,
}

/// Run the idle screen until the player opens the shelf or quits.
/// `static_level` is the signal-off snow to show — the steady dim hiss at
/// startup or after Esc on the shelf, or whatever level a just-ejected game
/// settled on (already steady by the time Eject fires, so no extra fade is
/// needed here either way). `notice_rx` is the startup update-check's
/// channel (plan revision: "ao abrir, verificar... avisar o usuario com uma
/// modal") — `None` once either a notice has been shown or the check found
/// nothing to report, so later visits to this screen stop polling a dead
/// receiver.
pub fn run(
    plat: &mut Platform,
    cab: &mut Cabinet,
    static_level: f32,
    notice_rx: &mut Option<Receiver<UpdateNotice>>,
) -> Result<IdleExit> {
    // Whatever game was loaded before (if any) is gone now — without this the
    // panel keeps showing its stale logo/commands instead of the "Inserir
    // cartucho" button, and there'd be nothing to click.
    cab.clear_panel();
    cab.set_close_button(true);
    load_console_logo(cab);
    let mut notice: Option<UpdateNotice> = None;
    loop {
        if let Some(rx) = notice_rx.as_ref() {
            match rx.try_recv() {
                Ok(n) => {
                    notice = Some(n);
                    *notice_rx = None;
                }
                Err(TryRecvError::Disconnected) => *notice_rx = None,
                Err(TryRecvError::Empty) => {}
            }
        }

        let m = plat.poll_menu(MenuMode::Nav);
        if m.quit {
            return Ok(IdleExit::Quit);
        }

        if let Some(n) = &notice {
            let dismiss = m.click.is_some()
                || m.nav
                    .iter()
                    .any(|nav| matches!(nav, MenuNav::Confirm | MenuNav::Back));
            if dismiss {
                notice = None;
                continue;
            }
            let lines = notice_lines(n);
            let render = |d: &mut Screen| draw_notice(d, &lines);
            cab.frame_2d(NOTICE_BG, render);
            continue;
        }

        if let Some((x, y)) = m.click {
            let (ox, oy) = cab.window_to_output(x, y);
            if cab.hit_close_button(ox, oy) {
                return Ok(IdleExit::Quit);
            }
            match cab.hit_panel_button(ox, oy) {
                Some(PanelButton::Insert) => return Ok(IdleExit::OpenShelf),
                Some(PanelButton::Settings) => return Ok(IdleExit::OpenSettings),
                _ => {}
            }
        }
        for nav in m.nav {
            match nav {
                MenuNav::Confirm => return Ok(IdleExit::OpenShelf),
                MenuNav::Back => return Ok(IdleExit::Quit),
                _ => {}
            }
        }
        cab.present_static(static_level);
    }
}

/// Turn an `UpdateNotice` into the lines `draw_notice` wraps and shows — only
/// the parts that actually apply (an app update, a stale core, or both).
fn notice_lines(n: &UpdateNotice) -> Vec<String> {
    let mut lines = Vec::new();
    if let Some(tag) = &n.app_update {
        lines.push(format!("nova versão do app disponível: {tag}"));
        lines.push("baixe em github.com/ticianocorral/snes-xperience/releases".to_string());
    }
    if n.core_stale {
        if !lines.is_empty() {
            lines.push(String::new());
        }
        lines.push("o núcleo snes9x instalado está desatualizado.".to_string());
        lines.push("atualize pelo menu configurações > núcleo.".to_string());
    }
    lines
}

fn draw_notice(d: &mut Screen, lines: &[String]) {
    let w = d.size().0.saturating_sub(NOTICE_MARGIN as u32 * 2);
    d.text(NOTICE_MARGIN, NOTICE_MARGIN, 2, NOTICE_TEXT, "atualização");
    let mut y = NOTICE_MARGIN + 50;
    for line in lines {
        if line.is_empty() {
            y += 16;
            continue;
        }
        y = d.text_wrapped(NOTICE_MARGIN, y, w, 1, NOTICE_TEXT, line) + 4;
    }
    d.text(
        NOTICE_MARGIN,
        d.size().1 as i32 - NOTICE_MARGIN,
        1,
        NOTICE_DIM,
        "clique ou aperte um botão para continuar",
    );
}

/// Headless preview of the idle screen (dev/testing) — same setup as `run`,
/// one frame captured through the tube instead of a live loop.
pub fn capture_preview(cab: &mut Cabinet, static_level: f32, path: &Path) -> Result<()> {
    cab.clear_panel();
    load_console_logo(cab);
    cab.capture_static_bmp(static_level, path)
        .map_err(|e| anyhow::anyhow!(e.to_string()))
}

/// Headless preview of the update notice screen (dev/testing) — `run` only
/// ever shows this live, once a background check reports something.
pub fn capture_notice_preview(cab: &mut Cabinet, notice: &UpdateNotice, path: &Path) -> Result<()> {
    let lines = notice_lines(notice);
    let render = |d: &mut Screen| draw_notice(d, &lines);
    cab.capture_2d(NOTICE_BG, render, path)
        .map_err(|e| anyhow::anyhow!(e.to_string()))
}

/// The console's own brand logo: a local `assets/console.png` overrides the
/// app's built-in default (plan revision — same "local file wins" convention
/// per-game art already follows), used whenever that file isn't there.
/// Either way a decode failure just falls back to plain text (`draw_panel`'s
/// idle branch handles that part) rather than a hard error over a logo image.
fn load_console_logo(cab: &mut Cabinet) {
    let console_png = crate::dirs::assets_dir().join("console.png");
    let console_logo = if console_png.exists() {
        match decode_art(&console_png, 640) {
            Ok(img) => Some(img),
            Err(e) => {
                log::warn!("console logo {}: {e}", console_png.display());
                None
            }
        }
    } else {
        match decode_default_logo(640) {
            Ok(img) => Some(img),
            Err(e) => {
                log::warn!("built-in console logo: {e}");
                None
            }
        }
    };
    cab.set_console_logo(
        console_logo
            .as_ref()
            .map(|(w, h, d)| (*w, *h, d.as_slice())),
    );
}

/// Decode `assets/console.png` to tightly-packed RGBA, downscaled for memory
/// — same helper shape as `runner::decode_art`/`shelf::decode_art`, just for
/// this screen's one fixed (not per-game) image.
fn decode_art(path: &Path, max: u32) -> anyhow::Result<(u32, u32, Vec<u8>)> {
    let img = image::open(path)?.thumbnail(max, max).to_rgba8();
    let (w, h) = img.dimensions();
    Ok((w, h, img.into_raw()))
}

/// Same as `decode_art`, for the built-in default logo (`DEFAULT_CONSOLE_LOGO`).
fn decode_default_logo(max: u32) -> anyhow::Result<(u32, u32, Vec<u8>)> {
    let img = image::load_from_memory(DEFAULT_CONSOLE_LOGO)?
        .thumbnail(max, max)
        .to_rgba8();
    let (w, h) = img.dimensions();
    Ok((w, h, img.into_raw()))
}
