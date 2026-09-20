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
use std::time::{Duration, Instant};

use std::sync::mpsc;

use anyhow::Result;
use xperience_platform::{Cabinet, MenuMode, MenuNav, PanelButton, Platform, Screen};

use crate::core_update::{self, CoreUpdateMsg};
use crate::update_check::UpdateNotice;

const NOTICE_BG: (u8, u8, u8) = (18, 18, 20);
const NOTICE_TEXT: (u8, u8, u8) = (232, 232, 232);
const NOTICE_DIM: (u8, u8, u8) = (150, 150, 158);
const NOTICE_MARGIN: i32 = 40;

/// The screen's resting static level — startup and "Esc on the shelf" have no
/// prior game to inherit a level from, so they use this (same dim hiss the
/// console settles on after powering off, `runner::OFF_STATIC_LEVEL`).
pub const RESTING_STATIC: f32 = crate::runner::OFF_STATIC_LEVEL;

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
    core_installed: bool,
) -> Result<IdleExit> {
    // Whatever game was loaded before (if any) is gone now — without this the
    // panel keeps showing its stale logo/commands instead of the "Inserir
    // cartucho" button, and there'd be nothing to click.
    cab.clear_panel();
    cab.set_close_button(true);
    crate::console_art::load_brand_images(cab);
    // The idle screen is where this app spends most of its life — pace it
    // (it used to spin unthrottled, redrawing 60fps+ of static forever).
    let frame = Duration::from_millis(16);
    let mut next = Instant::now() + frame;
    let mut notice: Option<UpdateNotice> = None;
    // Core missing (plan revision: "ao iniciar o app pela primeira vez e/ou
    // nao tiver o core na pasta, mostrar botão para baixar — avisar que para
    // jogar é necessário o download do core"): the panel grows a warning
    // line plus a "Baixar núcleo" button whose label carries the live
    // progress. Done returns to the plain idle panel.
    let mut core_missing = !core_installed;
    let mut core_rx: Option<mpsc::Receiver<CoreUpdateMsg>> = None;
    let mut core_label = "Baixar núcleo snes9x".to_string();
    loop {
        if let Some(rx) = &core_rx {
            match rx.try_recv() {
                Ok(CoreUpdateMsg::Progress { downloaded, total }) => {
                    let mb = downloaded as f64 / 1_048_576.0;
                    core_label = match total {
                        Some(t) => {
                            format!("Baixando núcleo... {mb:.1}/{:.1} MB", t as f64 / 1_048_576.0)
                        }
                        None => format!("Baixando núcleo... {mb:.1} MB"),
                    };
                }
                Ok(CoreUpdateMsg::Done) => {
                    core_missing = false;
                    core_rx = None;
                    core_label.clear();
                }
                Ok(CoreUpdateMsg::Failed(e)) => {
                    core_label = format!("Falha ({e}) - clique para tentar de novo");
                    core_rx = None;
                }
                Err(std::sync::mpsc::TryRecvError::Disconnected) => core_rx = None,
                Err(std::sync::mpsc::TryRecvError::Empty) => {}
            }
        }
        cab.set_idle_core_prompt(core_missing.then_some(core_label.as_str()));
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
            if cab.hit_minimize_button(ox, oy) {
                cab.minimize();
                continue;
            }
            match cab.hit_panel_button(ox, oy) {
                Some(PanelButton::Insert) => return Ok(IdleExit::OpenShelf),
                Some(PanelButton::Settings) => return Ok(IdleExit::OpenSettings),
                Some(PanelButton::CoreDownload) => {
                    // Already downloading? The click is ignored until the
                    // worker reports Done/Failed.
                    if core_rx.is_none() {
                        if let Some(url) = core_update::core_download_url() {
                            let (tx, rx) = mpsc::channel();
                            let dest = crate::dirs::core_dir();
                            std::thread::spawn(move || {
                                core_update::download_and_install(url, &dest, &tx)
                            });
                            core_rx = Some(rx);
                        } else {
                            core_label =
                                "Sem build automática - use configurações".to_string();
                        }
                    }
                }
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
        crate::runner::pace_frame(&mut next, frame);
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
pub fn capture_preview(
    cab: &mut Cabinet,
    static_level: f32,
    core_installed: bool,
    path: &Path,
) -> Result<()> {
    cab.clear_panel();
    if !core_installed {
        cab.set_idle_core_prompt(Some("Baixar núcleo snes9x"));
    }
    crate::console_art::load_brand_images(cab);
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
