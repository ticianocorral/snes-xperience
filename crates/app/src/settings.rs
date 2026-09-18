//! The settings screen — the idle screen's "Configurações" button. Two flat
//! lists (main, controls), no nesting deeper than that: click a row to act
//! on it (or gamepad nav + Confirm/Back — plan revision: no keyboard
//! shortcuts). Edits save to `xperience.cfg` immediately, not on some later
//! "apply" step — there's nothing to lose by backing out.

use std::sync::mpsc::{self, Receiver};
use std::time::{Duration, Instant};

use anyhow::Result;
use xperience_platform::{Cabinet, MenuMode, MenuNav, PadButton, Platform, Screen};

use crate::config::Config;
use crate::core_update::{self, CoreUpdateMsg};
use crate::rom_rename::{self, RenameOutcome};

const BG: (u8, u8, u8) = (18, 18, 20);
const TEXT: (u8, u8, u8) = (232, 232, 232);
const DIM: (u8, u8, u8) = (150, 150, 158);
const HINT: (u8, u8, u8) = (120, 116, 108);

const MARGIN: i32 = 40;
const ROW_H: i32 = 30;
/// Where the row list starts, vertically — right under the title (drawn at
/// scale 2, so `2 * GLYPH_H` tall) plus a little breathing room. Shared by
/// the draw functions and the click hit-test so they can't drift apart.
const LIST_TOP: i32 = MARGIN + 50;
const RUNAHEAD_MAX: u32 = 4;

/// Which row (if any) a screen-local click y lands in, given how many rows
/// are actually on screen right now — settings rows span the full width, so
/// only y matters. Shared by `draw_main`/`draw_controls`'s layout and the
/// click handling in `run`, so they can't drift apart.
fn row_at(y: i32, count: usize) -> Option<usize> {
    if y < LIST_TOP {
        return None;
    }
    let i = ((y - LIST_TOP) / ROW_H) as usize;
    (i < count).then_some(i)
}

/// Whether a screen-local click y lands in the bottom hint band — the one
/// place `draw_controls` offers a way back without a scrollable row list of
/// its own to put "Voltar" in (`draw_main`'s does, as its last row).
fn back_band_hit(y: i32, screen_h: i32) -> bool {
    y >= screen_h - MARGIN - ROW_H
}

#[derive(PartialEq, Eq)]
enum Mode {
    Main,
    Controls,
}

/// Where the "Núcleo" row's background download stands right now — drives
/// both its label and whether `Confirm` on it starts a new one.
enum CoreStatus {
    Idle,
    Downloading { downloaded: u64, total: Option<u64> },
    Done,
    Failed(String),
}

/// Run the settings screen until the player backs all the way out. Returns
/// `true` if the whole app should quit (window closed / Cmd-Q) instead of
/// returning to the shelf.
pub fn run(plat: &mut Platform, cab: &mut Cabinet, cfg: &mut Config) -> Result<bool> {
    let mut mode = Mode::Main;
    let mut main_sel: usize = 0;
    let mut controls_sel: usize = 0;
    let mut controls_top: usize = 0;
    let mut awaiting_key: Option<usize> = None; // index into keymap.describe()
    let mut core_status = CoreStatus::Idle;
    let mut core_worker: Option<Receiver<CoreUpdateMsg>> = None;
    // The last "Renomear ROMs" result, shown as that row's own label until
    // the next click (plan revision) — synchronous (plain disk renames, no
    // network), so unlike the core download there's no worker/progress to
    // track, just a one-shot summary.
    let mut rename_status: Option<String> = None;
    let frame_time = Duration::from_millis(16);
    cab.set_close_button(true);

    loop {
        let next = Instant::now() + frame_time;

        if let Some(rx) = &core_worker {
            while let Ok(msg) = rx.try_recv() {
                match msg {
                    CoreUpdateMsg::Progress { downloaded, total } => {
                        core_status = CoreStatus::Downloading { downloaded, total };
                    }
                    CoreUpdateMsg::Done => {
                        core_status = CoreStatus::Done;
                        core_worker = None;
                        break;
                    }
                    CoreUpdateMsg::Failed(e) => {
                        core_status = CoreStatus::Failed(e);
                        core_worker = None;
                        break;
                    }
                }
            }
        }

        let poll_mode = if awaiting_key.is_some() {
            MenuMode::CaptureKey
        } else {
            MenuMode::Nav
        };
        let m = plat.poll_menu(poll_mode);
        if m.quit {
            return Ok(true);
        }

        if let Some(row) = awaiting_key {
            if let Some(name) = m.captured_key {
                bind_row(cfg, row, &name);
                let _ = cfg.save();
                awaiting_key = None;
            } else if m.capture_cancelled {
                awaiting_key = None;
            }
        } else {
            // Mouse click — every action here also has a gamepad-nav path
            // below (`m.nav`, fed by d-pad/buttons regardless of keyboard),
            // this just adds the click-driven one (plan revision: no
            // keyboard shortcuts left for either screen).
            if let Some((cx, cy)) = m.click {
                let (ox, oy) = cab.window_to_output(cx, cy);
                if cab.hit_close_button(ox, oy) {
                    return Ok(true);
                }
                if let Some((_, ly)) = cab.hit_screen_point(ox, oy) {
                    match mode {
                        Mode::Main => {
                            if let Some(i) = row_at(ly, MAIN_ROWS) {
                                main_sel = i;
                                match i {
                                    0 => mode = Mode::Controls,
                                    1 => start_core_download(&mut core_status, &mut core_worker),
                                    2 => {
                                        cfg.runahead = (cfg.runahead + 1) % (RUNAHEAD_MAX + 1);
                                        let _ = cfg.save();
                                    }
                                    3 => {
                                        cfg.fullscreen = !cfg.fullscreen;
                                        cab.toggle_fullscreen();
                                        let _ = cfg.save();
                                    }
                                    4 => {
                                        cfg.check_updates_on_start = !cfg.check_updates_on_start;
                                        let _ = cfg.save();
                                    }
                                    5 => rename_status = Some(run_rom_rename()),
                                    6 => return Ok(false),
                                    _ => {}
                                }
                            }
                        }
                        Mode::Controls => {
                            let rows = cfg.keymap.describe().len();
                            let visible = ((cab.screen_size().1 as i32 - MARGIN * 2 - 60) / ROW_H)
                                .max(1) as usize;
                            let shown = visible.min(rows.saturating_sub(controls_top));
                            if let Some(i) = row_at(ly, shown) {
                                controls_sel = controls_top + i;
                                awaiting_key = Some(controls_sel);
                            } else if back_band_hit(ly, cab.screen_size().1 as i32) {
                                mode = Mode::Main;
                            }
                        }
                    }
                }
            }
            match mode {
                Mode::Main => {
                    for nav in &m.nav {
                        match nav {
                            MenuNav::Up => main_sel = main_sel.saturating_sub(1),
                            MenuNav::Down => main_sel = (main_sel + 1).min(MAIN_ROWS - 1),
                            MenuNav::Back => return Ok(false),
                            MenuNav::Confirm => match main_sel {
                                0 => mode = Mode::Controls,
                                1 => start_core_download(&mut core_status, &mut core_worker),
                                3 => {
                                    cfg.fullscreen = !cfg.fullscreen;
                                    cab.toggle_fullscreen();
                                    let _ = cfg.save();
                                }
                                4 => {
                                    cfg.check_updates_on_start = !cfg.check_updates_on_start;
                                    let _ = cfg.save();
                                }
                                5 => rename_status = Some(run_rom_rename()),
                                6 => return Ok(false),
                                _ => {}
                            },
                            MenuNav::Left if main_sel == 2 => {
                                cfg.runahead = cfg.runahead.saturating_sub(1);
                                let _ = cfg.save();
                            }
                            MenuNav::Right if main_sel == 2 => {
                                cfg.runahead = (cfg.runahead + 1).min(RUNAHEAD_MAX);
                                let _ = cfg.save();
                            }
                            MenuNav::Left | MenuNav::Right if main_sel == 3 => {
                                cfg.fullscreen = !cfg.fullscreen;
                                cab.toggle_fullscreen();
                                let _ = cfg.save();
                            }
                            MenuNav::Left | MenuNav::Right if main_sel == 4 => {
                                cfg.check_updates_on_start = !cfg.check_updates_on_start;
                                let _ = cfg.save();
                            }
                            _ => {}
                        }
                    }
                }
                Mode::Controls => {
                    let rows = cfg.keymap.describe().len();
                    for nav in &m.nav {
                        match nav {
                            MenuNav::Up => controls_sel = controls_sel.saturating_sub(1),
                            MenuNav::Down => controls_sel = (controls_sel + 1).min(rows - 1),
                            MenuNav::Back => mode = Mode::Main,
                            MenuNav::Confirm => awaiting_key = Some(controls_sel),
                            _ => {}
                        }
                    }
                    let visible =
                        ((cab.screen_size().1 as i32 - MARGIN * 2 - 60) / ROW_H).max(1) as usize;
                    if controls_sel < controls_top {
                        controls_top = controls_sel;
                    } else if controls_sel >= controls_top + visible {
                        controls_top = controls_sel + 1 - visible;
                    }
                }
            }
        }

        let render = |d: &mut Screen| match mode {
            Mode::Main => draw_main(d, cfg, main_sel, &core_status, rename_status.as_deref()),
            Mode::Controls => draw_controls(d, cfg, controls_sel, controls_top, awaiting_key),
        };
        cab.frame_2d(BG, render);

        let now = Instant::now();
        if next > now {
            std::thread::sleep(next - now);
        }
    }
}

/// Kick off a background download of the snes9x core (plan: "opção pra
/// baixar o snes9x... e opção de update do núcleo" — one action serves both,
/// the buildbot only ever serves "latest"). No-op while one is already in
/// flight. The standard background-thread + `mpsc` shape of this app —
/// `settings::run`'s loop drains it with `try_recv()` every frame.
fn start_core_download(status: &mut CoreStatus, worker: &mut Option<Receiver<CoreUpdateMsg>>) {
    if matches!(status, CoreStatus::Downloading { .. }) {
        return;
    }
    let Some(url) = core_update::core_download_url() else {
        *status = CoreStatus::Failed("sem build automática pra esta plataforma".to_string());
        return;
    };
    let (tx, rx) = mpsc::channel();
    let dest = crate::dirs::core_dir();
    std::thread::spawn(move || core_update::download_and_install(url, &dest, &tx));
    *worker = Some(rx);
    *status = CoreStatus::Downloading {
        downloaded: 0,
        total: None,
    };
}

/// How long since the installed core was written, in the coarse terms
/// `draw_main`'s row wants — `None` if there's no core installed at all.
fn core_installed_label() -> Option<String> {
    let path = crate::dirs::core_dir().join(core_update::core_file_name());
    let modified = std::fs::metadata(&path).ok()?.modified().ok()?;
    let days = modified.elapsed().unwrap_or_default().as_secs() / 86_400;
    Some(match days {
        0 => "instalado hoje".to_string(),
        1 => "instalado há 1 dia".to_string(),
        n => format!("instalado há {n} dias"),
    })
}

/// Run the rename synchronously (plain disk renames — fast enough even for
/// a few hundred ROMs that there's no need for `start_core_download`'s
/// background-thread treatment) and turn the result into the row's next
/// label (plan: "renomear automaticamente no padrao no-intro").
fn run_rom_rename() -> String {
    let outcome = rom_rename::rename_to_nointro(
        &crate::dirs::roms_dir(),
        &crate::dirs::nointro_dat_path(),
        &crate::dirs::saves_dir(),
        &crate::dirs::notes_dir(),
        &crate::dirs::assets_dir(),
    );
    match outcome {
        RenameOutcome::Renamed(list) => {
            for r in &list {
                log::info!("rom rename: {} -> {}", r.old, r.new);
            }
            match list.len() {
                1 => "Renomear ROMs: 1 rom renomeada".to_string(),
                n => format!("Renomear ROMs: {n} roms renomeadas"),
            }
        }
        RenameOutcome::NothingToDo => "Renomear ROMs: nenhuma precisava de nome novo".to_string(),
        RenameOutcome::NoDat => "Renomear ROMs: nointro.dat não encontrado".to_string(),
    }
}

fn core_row_label(status: &CoreStatus) -> String {
    match status {
        CoreStatus::Downloading { downloaded, total } => {
            let mb = *downloaded as f64 / 1_048_576.0;
            match total {
                Some(t) => format!(
                    "Núcleo: baixando... {mb:.1}/{:.1} MB",
                    *t as f64 / 1_048_576.0
                ),
                None => format!("Núcleo: baixando... {mb:.1} MB"),
            }
        }
        CoreStatus::Done => "Núcleo: atualizado com sucesso".to_string(),
        CoreStatus::Failed(e) => format!("Núcleo: falha - {e}"),
        CoreStatus::Idle => match core_installed_label() {
            Some(installed) => format!("Núcleo: atualizar ({installed})"),
            None => "Núcleo: baixar".to_string(),
        },
    }
}

const MAIN_ROWS: usize = 7;

fn bind_row(cfg: &mut Config, row: usize, key_name: &str) {
    let Some((action, _)) = cfg.keymap.describe().into_iter().nth(row) else {
        return;
    };
    // Every row here is a gameplay button now — console/UI commands aren't
    // rebindable any more (plan revision: mouse/gamepad only).
    let Ok(b) = action.parse::<PadButton>() else {
        return;
    };
    if let Err(e) = cfg.keymap.bind_pad(key_name, b) {
        log::warn!("rebind {action} -> {key_name}: {e}");
    }
}

fn draw_main(
    d: &mut Screen,
    cfg: &Config,
    sel: usize,
    core_status: &CoreStatus,
    rename_status: Option<&str>,
) {
    let x = MARGIN;
    d.text(x, MARGIN, 2, TEXT, "configurações");
    let mut y = LIST_TOP;

    let rows = [
        "Controles".to_string(),
        core_row_label(core_status),
        format!("Run-ahead: {} quadro(s)", cfg.runahead),
        format!(
            "Tela cheia: {}",
            if cfg.fullscreen {
                "ligada"
            } else {
                "desligada"
            }
        ),
        format!(
            "Verificar atualizações ao abrir: {}",
            if cfg.check_updates_on_start {
                "sim"
            } else {
                "não"
            }
        ),
        rename_status
            .unwrap_or("Renomear ROMs para o padrão No-Intro")
            .to_string(),
        "Voltar".to_string(),
    ];
    for (i, row) in rows.iter().enumerate() {
        draw_row(d, x, y, row, i == sel);
        y += ROW_H;
    }

    d.text(
        x,
        d.size().1 as i32 - MARGIN,
        1,
        HINT,
        "clique numa opção (role a lista com o mouse ou d-pad, botão/gamepad confirma)",
    );
}

fn draw_controls(d: &mut Screen, cfg: &Config, sel: usize, top: usize, awaiting: Option<usize>) {
    let x = MARGIN;
    d.text(x, MARGIN, 2, TEXT, "controles");
    let mut y = LIST_TOP;

    let binds = cfg.keymap.describe();
    let visible = ((d.size().1 as i32 - MARGIN * 2 - 60) / ROW_H).max(1) as usize;
    for (i, (action, key)) in binds.iter().enumerate().skip(top).take(visible) {
        let label = if awaiting == Some(i) {
            format!("{action}: aperte uma tecla... (esc cancela)")
        } else {
            format!("{action}: {key}")
        };
        draw_row(d, x, y, &label, i == sel);
        y += ROW_H;
    }

    d.text(
        x,
        d.size().1 as i32 - MARGIN,
        1,
        HINT,
        "clique numa ação pra trocar a tecla -- clique aqui embaixo pra voltar",
    );
}

/// Headless preview of one screen (`"main"` | `"controls"`), for
/// verification — not part of the interactive `run` loop.
pub fn capture_preview(
    cab: &mut Cabinet,
    cfg: &Config,
    screen: &str,
    path: &std::path::Path,
) -> Result<()> {
    let render = |d: &mut Screen| match screen {
        "controls" => draw_controls(d, cfg, 0, 0, None),
        _ => draw_main(d, cfg, 0, &CoreStatus::Idle, None),
    };
    cab.set_close_button(true);
    cab.capture_2d(BG, render, path)
        .map_err(|e| anyhow::anyhow!(e.to_string()))
}

fn draw_row(d: &mut Screen, x: i32, y: i32, label: &str, selected: bool) {
    let (cursor, color) = if selected { ("> ", TEXT) } else { ("  ", DIM) };
    d.text(x, y, 1, color, &format!("{cursor}{label}"));
}
