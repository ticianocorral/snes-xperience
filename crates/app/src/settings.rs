//! The settings screen — `O` on the shelf. Two flat lists (main, controls),
//! no nesting deeper than that: pick a row, `Confirm` acts on it, `Back` goes
//! up a level. Edits save to `xperience.cfg` immediately, not on some later
//! "apply" step — there's nothing to lose by backing out.

use std::sync::mpsc::{self, Receiver};
use std::time::{Duration, Instant};

use anyhow::Result;
use xperience_platform::{Cabinet, MenuMode, MenuNav, PadButton, Platform, Screen, UiEvent};

use crate::config::Config;
use crate::core_update::{self, CoreUpdateMsg};

const BG: (u8, u8, u8) = (18, 18, 20);
const TEXT: (u8, u8, u8) = (232, 232, 232);
const DIM: (u8, u8, u8) = (150, 150, 158);
const HINT: (u8, u8, u8) = (120, 116, 108);

const MARGIN: i32 = 40;
const ROW_H: i32 = 22;
const RUNAHEAD_MAX: u32 = 4;

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
    let frame_time = Duration::from_millis(16);

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
        if m.toggle_fullscreen {
            cab.toggle_fullscreen();
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
                                4 => return Ok(false),
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
            Mode::Main => draw_main(d, cfg, main_sel, &core_status),
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
/// flight. Same thread + `mpsc` shape `shelf.rs` used for the old
/// ScreenScraper worker — `settings::run`'s loop drains it with `try_recv()`
/// every frame, same as there.
fn start_core_download(status: &mut CoreStatus, worker: &mut Option<Receiver<CoreUpdateMsg>>) {
    if matches!(status, CoreStatus::Downloading { .. }) {
        return;
    }
    let Some(url) = core_update::core_download_url() else {
        *status = CoreStatus::Failed("sem build automatica pra esta plataforma".to_string());
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
        1 => "instalado ha 1 dia".to_string(),
        n => format!("instalado ha {n} dias"),
    })
}

fn core_row_label(status: &CoreStatus) -> String {
    match status {
        CoreStatus::Downloading { downloaded, total } => {
            let mb = *downloaded as f64 / 1_048_576.0;
            match total {
                Some(t) => format!(
                    "Nucleo: baixando... {mb:.1}/{:.1} MB",
                    *t as f64 / 1_048_576.0
                ),
                None => format!("Nucleo: baixando... {mb:.1} MB"),
            }
        }
        CoreStatus::Done => "Nucleo: atualizado com sucesso".to_string(),
        CoreStatus::Failed(e) => format!("Nucleo: falha - {e}"),
        CoreStatus::Idle => match core_installed_label() {
            Some(installed) => format!("Nucleo: atualizar ({installed})"),
            None => "Nucleo: baixar".to_string(),
        },
    }
}

const MAIN_ROWS: usize = 5;

fn bind_row(cfg: &mut Config, row: usize, key_name: &str) {
    let Some((action, _)) = cfg.keymap.describe().into_iter().nth(row) else {
        return;
    };
    let result = if let Ok(b) = action.parse::<PadButton>() {
        cfg.keymap.bind_pad(key_name, b)
    } else if let Some(e) = UiEvent::BINDABLE
        .into_iter()
        .find(|e| e.token() == Some(action.as_str()))
    {
        cfg.keymap.bind_ui(key_name, e)
    } else {
        return;
    };
    if let Err(e) = result {
        log::warn!("rebind {action} -> {key_name}: {e}");
    }
}

fn draw_main(d: &mut Screen, cfg: &Config, sel: usize, core_status: &CoreStatus) {
    let x = MARGIN;
    let mut y = MARGIN;
    d.text(x, y, 2, TEXT, "configuracoes");
    y += 40;

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
        "cima/baixo: navega  esquerda/direita: muda  enter: abre/liga  esc: volta pra estante",
    );
}

fn draw_controls(d: &mut Screen, cfg: &Config, sel: usize, top: usize, awaiting: Option<usize>) {
    let x = MARGIN;
    let mut y = MARGIN;
    d.text(x, y, 2, TEXT, "controles");
    y += 40;

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
        "enter: rebind  esc: volta",
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
        _ => draw_main(d, cfg, 0, &CoreStatus::Idle),
    };
    cab.capture_2d(BG, render, path)
        .map_err(|e| anyhow::anyhow!(e.to_string()))
}

fn draw_row(d: &mut Screen, x: i32, y: i32, label: &str, selected: bool) {
    let (cursor, color) = if selected { ("> ", TEXT) } else { ("  ", DIM) };
    d.text(x, y, 1, color, &format!("{cursor}{label}"));
}
