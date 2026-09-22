//! The idle/root screen: TV off, no game loaded — the app's home state, and
//! also literally the "cartridge ejected" screen (plan revision: one screen,
//! not two — startup, backing out of the shelf, and ejecting a game all land
//! here). In place of a game's logo/cartridge art there's the console's own
//! brand logo (`assets/console.png`, optional) and an "Inserir cartucho"
//! button; "Configurações" sits in the panel's footer. This is the outermost
//! screen now: a gamepad's Back button here, same as closing the window,
//! ends the app (plan revision: mouse/gamepad only, no keyboard shortcuts).
//!
//! First-run setup (plan revision: "ao abrir pela primeira vez e não ter o
//! DAT, o snes9x — abrir dentro da tv o aviso para baixar os dois arquivos,
//! com o botão para baixar"): when either the snes9x core or the
//! `nointro.dat` is missing, the TV shows a full setup screen with a
//! download button for each, before the usual idle panel. Updates (plan
//! revision: "quando tiver update do app ou do snes9x não mostrar mais a
//! tela cheia e sim um icone verde no nameplate do lado de cada um") are no
//! longer a modal — the startup check's result lights a green dot in the
//! chin, next to the app's and the core's version lines.

use std::path::Path;
use std::sync::mpsc::{Receiver, TryRecvError};
use std::time::{Duration, Instant};

use std::sync::mpsc;

use anyhow::Result;
use xperience_platform::{Cabinet, MenuMode, MenuNav, PanelButton, Platform, Screen};

use crate::core_update::{self, CoreUpdateMsg};
use crate::update_check::UpdateNotice;

const SETUP_BG: (u8, u8, u8) = (18, 18, 20);
const SETUP_TEXT: (u8, u8, u8) = (232, 232, 232);
const SETUP_DIM: (u8, u8, u8) = (150, 150, 158);
const SETUP_GREEN: (u8, u8, u8) = (60, 230, 70);

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

/// One clickable region of the setup screen, in output coordinates —
/// `(x, y, w, h)` in the same space `Cabinet::window_to_output` maps clicks
/// into, so `hit_setup_button` and the draw closure stay in sync by both
/// coming from `setup_rects`.
type RectT = (i32, i32, u32, u32);

/// What the player can click on the setup screen.
enum SetupButton {
    Core,
    Dat,
    Continue,
}

/// Button/status geometry for the setup screen, derived from the output
/// size — the single source both `draw_setup` and `hit_setup_button` use.
fn setup_rects(w: u32, h: u32) -> (RectT, RectT, RectT) {
    let bw = 640.min(w.saturating_sub(160)).max(280);
    let bh = 64u32;
    let x = (w as i32 - bw as i32) / 2;
    let core_y = h as i32 * 34 / 100;
    let dat_y = core_y + bh as i32 + 44;
    let cw = 280u32;
    let ch = 56u32;
    let cont = (
        (w as i32 - cw as i32) / 2,
        h as i32 - ch as i32 - 72,
        cw,
        ch,
    );
    ((x, core_y, bw, bh), (x, dat_y, bw, bh), cont)
}

fn hit_setup_button(w: u32, h: u32, x: i32, y: i32) -> Option<SetupButton> {
    let (core, dat, cont) = setup_rects(w, h);
    let inside =
        |r: &RectT| x >= r.0 && y >= r.1 && (x - r.0) < r.2 as i32 && (y - r.1) < r.3 as i32;
    if inside(&core) {
        Some(SetupButton::Core)
    } else if inside(&dat) {
        Some(SetupButton::Dat)
    } else if inside(&cont) {
        Some(SetupButton::Continue)
    } else {
        None
    }
}

/// Download state for one setup button — `label` carries the live progress
/// /failure text, `rx` is `Some` while the worker thread runs.
struct Download {
    label: String,
    rx: Option<mpsc::Receiver<CoreUpdateMsg>>,
    done: bool,
}

impl Download {
    fn idle(label: &str) -> Self {
        Self {
            label: label.to_string(),
            rx: None,
            done: false,
        }
    }

    /// Drain the worker's channel, if one is wired up — the same pump
    /// `idle::run` used to inline for the core alone.
    fn pump(&mut self) {
        let Some(rx) = &self.rx else { return };
        match rx.try_recv() {
            Ok(CoreUpdateMsg::Progress { downloaded, total }) => {
                let mb = downloaded as f64 / 1_048_576.0;
                self.label = match total {
                    Some(t) => format!("baixando... {mb:.1}/{:.1} MB", t as f64 / 1_048_576.0),
                    None => format!("baixando... {mb:.1} MB"),
                };
            }
            Ok(CoreUpdateMsg::Done) => {
                self.done = true;
                self.rx = None;
                self.label = "instalado".to_string();
            }
            Ok(CoreUpdateMsg::Failed(e)) => {
                self.label = format!("falha ({e}) - clique para tentar de novo");
                self.rx = None;
            }
            Err(TryRecvError::Disconnected) => self.rx = None,
            Err(TryRecvError::Empty) => {}
        }
    }
}

/// Run the idle screen until the player opens the shelf or quits.
/// `static_level` is the signal-off snow to show — the steady dim hiss at
/// startup or after Esc on the shelf, or whatever level a just-ejected game
/// settled on (already steady by the time Eject fires, so no extra fade is
/// needed here either way). `notice_rx` is the startup update-check's
/// channel — `None` once the check has reported (its result went straight
/// to the nameplate's green dots) or found nothing.
pub fn run(
    plat: &mut Platform,
    cab: &mut Cabinet,
    static_level: f32,
    notice_rx: &mut Option<Receiver<UpdateNotice>>,
    core_installed: bool,
    dat_installed: bool,
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

    // First-run setup (plan revision: see module docs): the TV shows the
    // two download buttons until the player hits "continuar". After that
    // the plain idle panel takes over (with the old core prompt if the
    // core is still missing).
    let mut setup = core_missing_at_start(core_installed, dat_installed);
    let mut core = Download::idle("baixar núcleo snes9x");
    if core_installed {
        core.done = true;
        core.label = "instalado".to_string();
    }
    let mut dat = Download::idle("baixar nointro.dat");
    if dat_installed {
        dat.done = true;
        dat.label = "instalado".to_string();
    }
    let mut core_missing = !core_installed;

    loop {
        let core_was_done = core.done;
        core.pump();
        dat.pump();
        // A core download just finished (setup screen or panel button) —
        // rebuild the nameplate right away, so the snes9x line appears
        // without waiting for the player to leave this screen.
        if core.done && !core_was_done {
            let p = core_update::default_core_path();
            cab.set_nameplate(&core_update::nameplate_text(p.as_deref()));
        }
        if core.rx.is_none() && core.done {
            core_missing = false;
        }
        if let Some(rx) = notice_rx.as_ref() {
            match rx.try_recv() {
                // Updates aren't a modal anymore (plan revision) — the
                // result just lights the green dots in the nameplate: line
                // 0 for the app, line 1 for the core.
                Ok(n) => {
                    cab.set_nameplate_updates(n.app_update.is_some(), n.core_stale);
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

        if setup {
            let (w, h) = cab.screen_size();
            let mut dismiss = false;
            if let Some((x, y)) = m.click {
                // The close/minimize pair lives in cabinet-canvas space; the
                // setup buttons in the screen buffer's own space (the screen
                // is inset by the bezel) — each with its own mapping.
                let (ox, oy) = cab.window_to_output(x, y);
                if cab.hit_close_button(ox, oy) {
                    return Ok(IdleExit::Quit);
                }
                if cab.hit_minimize_button(ox, oy) {
                    cab.minimize();
                    continue;
                }
                let (sx, sy) = cab.window_to_screen(x, y);
                match hit_setup_button(w, h, sx, sy) {
                    Some(SetupButton::Core) if core.rx.is_none() && !core.done => {
                        if let Some(url) = core_update::core_download_url() {
                            let (tx, rx) = mpsc::channel();
                            let dest = crate::dirs::core_dir();
                            std::thread::spawn(move || {
                                core_update::download_and_install(url, &dest, &tx)
                            });
                            core.rx = Some(rx);
                        } else {
                            core.label = "sem build automática - use configurações".to_string();
                        }
                    }
                    Some(SetupButton::Dat) if dat.rx.is_none() && !dat.done => {
                        let (tx, rx) = mpsc::channel();
                        let dest = crate::dat_update::dat_path();
                        std::thread::spawn(move || {
                            crate::dat_update::download_and_install(&dest, &tx)
                        });
                        dat.rx = Some(rx);
                    }
                    Some(SetupButton::Continue) => dismiss = true,
                    _ => {}
                }
            }
            // Confirm (Enter/gamepad A) on the setup screen means
            // "continuar" — Back still quits.
            for nav in &m.nav {
                match nav {
                    MenuNav::Confirm => dismiss = true,
                    MenuNav::Back => return Ok(IdleExit::Quit),
                    _ => {}
                }
            }
            if dismiss {
                setup = false;
            } else {
                let core_label = core.label.clone();
                let dat_label = dat.label.clone();
                let core_done = core.done;
                let dat_done = dat.done;
                let core_busy = core.rx.is_some();
                let dat_busy = dat.rx.is_some();
                let render = |d: &mut Screen| {
                    draw_setup(
                        d,
                        &core_label,
                        core_done,
                        core_busy,
                        &dat_label,
                        dat_done,
                        dat_busy,
                    )
                };
                cab.frame_idle_2d(SETUP_BG, render);
                crate::runner::pace_frame(&mut next, frame);
                continue;
            }
            // Just dismissed: the same click/Confirm that dismissed the
            // setup screen must not also land on the idle panel below —
            // give it a frame of its own.
            cab.set_idle_core_prompt(core_missing.then_some("Baixar núcleo snes9x"));
            crate::runner::pace_frame(&mut next, frame);
            continue;
        }

        cab.set_idle_core_prompt(core_missing.then_some("Baixar núcleo snes9x"));

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
                // Sem o snes9x instalado (plan revision: "só desative o
                // botão de config e de inserir cartucho se o usuário não
                // baixou o snes9x") — os dois ficam inertes; o painel
                // oferece o download do core no lugar.
                Some(PanelButton::Insert) if !core_missing => return Ok(IdleExit::OpenShelf),
                Some(PanelButton::Settings) if !core_missing => return Ok(IdleExit::OpenSettings),
                Some(PanelButton::CoreDownload) if core.rx.is_none() => {
                    match core_update::core_download_url() {
                        Some(url) => {
                            let (tx, rx) = mpsc::channel();
                            let dest = crate::dirs::core_dir();
                            std::thread::spawn(move || {
                                core_update::download_and_install(url, &dest, &tx)
                            });
                            core.rx = Some(rx);
                            core.done = false;
                            core.label = "baixando...".to_string();
                        }
                        None => {
                            core.label = "Sem build automática - use configurações".to_string();
                        }
                    }
                }
                // Already downloading? The click is ignored until the
                // worker reports Done/Failed.
                Some(PanelButton::CoreDownload) => {}
                _ => {}
            }
        }
        for nav in m.nav {
            match nav {
                // Same gate as the panel buttons above: without the core
                // there's nowhere to go but out — Confirm is inert, Back
                // still quits.
                MenuNav::Confirm if !core_missing => return Ok(IdleExit::OpenShelf),
                MenuNav::Back => return Ok(IdleExit::Quit),
                _ => {}
            }
        }
        cab.present_static(static_level);
        crate::runner::pace_frame(&mut next, frame);
    }
}

/// Whether the setup screen should come up — either file the player needs
/// is missing (plan revision: "não tiver o DAT, o snes9x").
fn core_missing_at_start(core_installed: bool, dat_installed: bool) -> bool {
    !core_installed || !dat_installed
}

/// The first-run setup screen, drawn inside the TV: a download button per
/// missing file, each with its own live status line underneath, plus
/// "continuar" to get to the regular console. Done buttons restyle to a
/// green "instalado" instead of disappearing, so the layout never jumps.
#[allow(clippy::too_many_arguments)]
fn draw_setup(
    d: &mut Screen,
    core_label: &str,
    core_done: bool,
    core_busy: bool,
    dat_label: &str,
    dat_done: bool,
    dat_busy: bool,
) {
    /// Same glyph metrics the platform font uses — `Screen::text` advances
    /// `GLYPH_W * scale` per char, `GLYPH_H * scale` per line.
    const CELL: i32 = 9;
    let (w, h) = d.size();
    let (w, h) = (w as i32, h as i32);
    let (core_r, dat_r, cont_r) = setup_rects(w as u32, h as u32);
    let center_x = |s: &str, scale: u32| (w - s.chars().count() as i32 * CELL * scale as i32) / 2;

    d.text(
        center_x("bem-vindo ao snes xperience", 3),
        h * 12 / 100,
        3,
        SETUP_TEXT,
        "bem-vindo ao snes xperience",
    );
    d.text(
        center_x("para jogar, faltam dois downloads", 1),
        h * 12 / 100 + 74,
        1,
        SETUP_DIM,
        "para jogar, faltam dois downloads",
    );

    fn draw_button(d: &mut Screen, r: &RectT, label: &str, color: (u8, u8, u8)) {
        const CELL2: i32 = 18; // CELL * scale 2, for centering the label
        let (cr, cg, cb) = color;
        d.outline(r.0, r.1, r.2, r.3, 2, (cr, cg, cb, 255));
        d.text(
            r.0 + (r.2 as i32 - label.chars().count() as i32 * CELL2) / 2,
            r.1 + (r.3 as i32 - 40) / 2,
            2,
            color,
            label,
        );
    }
    let (mut core_color, mut dat_color) = (SETUP_TEXT, SETUP_TEXT);
    if core_busy {
        core_color = SETUP_DIM;
    } else if core_done {
        core_color = SETUP_GREEN;
    }
    if dat_busy {
        dat_color = SETUP_DIM;
    } else if dat_done {
        dat_color = SETUP_GREEN;
    }
    draw_button(d, &core_r, core_label, core_color);
    draw_button(d, &dat_r, dat_label, dat_color);
    d.text(
        core_r.0 + 4,
        core_r.1 + core_r.3 as i32 + 8,
        1,
        SETUP_DIM,
        "necessário para rodar os jogos",
    );
    d.text(
        dat_r.0 + 4,
        dat_r.1 + dat_r.3 as i32 + 8,
        1,
        SETUP_DIM,
        "opcional: nomes canônicos, ano e editora dos jogos",
    );

    draw_button(d, &cont_r, "continuar", SETUP_TEXT);
    d.text(
        center_x("enter também continua", 1),
        cont_r.1 + cont_r.3 as i32 + 10,
        1,
        SETUP_DIM,
        "enter também continua",
    );
}

/// Headless preview of the idle screen (dev/testing) — same setup as `run`,
/// one frame captured through the tube instead of a live loop. Shows the
/// first-run setup screen when either download is missing, the plain idle
/// panel otherwise.
pub fn capture_preview(
    cab: &mut Cabinet,
    static_level: f32,
    core_installed: bool,
    dat_installed: bool,
    path: &Path,
) -> Result<()> {
    cab.clear_panel();
    if core_missing_at_start(core_installed, dat_installed) {
        let render = |d: &mut Screen| {
            draw_setup(
                d,
                if core_installed {
                    "instalado"
                } else {
                    "baixar núcleo snes9x"
                },
                core_installed,
                false,
                if dat_installed {
                    "instalado"
                } else {
                    "baixar nointro.dat"
                },
                dat_installed,
                false,
            )
        };
        cab.capture_idle_2d(SETUP_BG, render, path)
            .map_err(|e| anyhow::anyhow!(e.to_string()))
    } else {
        crate::console_art::load_brand_images(cab);
        cab.capture_static_bmp(static_level, path)
            .map_err(|e| anyhow::anyhow!(e.to_string()))
    }
}
