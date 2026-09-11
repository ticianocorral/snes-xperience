//! The settings screen — `O` on the shelf. Three flat lists (main, controls,
//! ScreenScraper), no nesting deeper than that: pick a row, `Confirm` acts on
//! it, `Back` goes up a level. Edits save to `config.toml` immediately, not
//! on some later "apply" step — there's nothing to lose by backing out.

use std::time::{Duration, Instant};

use anyhow::Result;
use xperience_platform::{Cabinet, MenuMode, MenuNav, PadButton, Platform, Screen, UiEvent};

use crate::config::Config;

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
    ScreenScraper,
}

/// Which field is being typed into right now, if any (ScreenScraper mode
/// only — Controls captures a raw key instead, see `awaiting_key`).
#[derive(Clone, Copy, PartialEq, Eq)]
enum Field {
    DevId,
    DevPassword,
}

/// Run the settings screen until the player backs all the way out. Returns
/// `true` if the whole app should quit (window closed / Cmd-Q) instead of
/// returning to the shelf.
pub fn run(plat: &mut Platform, cab: &mut Cabinet, cfg: &mut Config) -> Result<bool> {
    let mut mode = Mode::Main;
    let mut main_sel: usize = 0;
    let mut controls_sel: usize = 0;
    let mut controls_top: usize = 0;
    let mut ss_sel: usize = 0;
    let mut awaiting_key: Option<usize> = None; // index into keymap.describe()
    let mut editing: Option<Field> = None;
    let mut edit_buf = String::new();
    let frame_time = Duration::from_millis(16);

    loop {
        let next = Instant::now() + frame_time;

        let poll_mode = if awaiting_key.is_some() {
            MenuMode::CaptureKey
        } else if editing.is_some() {
            MenuMode::TextEntry
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
        } else if let Some(field) = editing {
            if !m.typed.is_empty() {
                edit_buf.push_str(&m.typed);
            }
            if m.backspace {
                edit_buf.pop();
            }
            for nav in &m.nav {
                match nav {
                    MenuNav::Confirm => {
                        match field {
                            Field::DevId => cfg.screenscraper.dev_id = edit_buf.clone(),
                            Field::DevPassword => cfg.screenscraper.dev_password = edit_buf.clone(),
                        }
                        let _ = cfg.save();
                        editing = None;
                    }
                    MenuNav::Back => editing = None, // discard edit_buf
                    _ => {}
                }
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
                                1 => mode = Mode::ScreenScraper,
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
                Mode::ScreenScraper => {
                    for nav in &m.nav {
                        match nav {
                            MenuNav::Up => ss_sel = ss_sel.saturating_sub(1),
                            MenuNav::Down => ss_sel = (ss_sel + 1).min(SS_ROWS - 1),
                            MenuNav::Back => mode = Mode::Main,
                            MenuNav::Confirm => match ss_sel {
                                0 => {
                                    cfg.screenscraper.enabled = !cfg.screenscraper.enabled;
                                    let _ = cfg.save();
                                }
                                1 => {
                                    edit_buf = cfg.screenscraper.dev_id.clone();
                                    editing = Some(Field::DevId);
                                }
                                2 => {
                                    edit_buf = cfg.screenscraper.dev_password.clone();
                                    editing = Some(Field::DevPassword);
                                }
                                3 => mode = Mode::Main,
                                _ => {}
                            },
                            MenuNav::Left | MenuNav::Right if ss_sel == 0 => {
                                cfg.screenscraper.enabled = !cfg.screenscraper.enabled;
                                let _ = cfg.save();
                            }
                            _ => {}
                        }
                    }
                }
            }
        }

        let render = |d: &mut Screen| match mode {
            Mode::Main => draw_main(d, cfg, main_sel),
            Mode::Controls => draw_controls(d, cfg, controls_sel, controls_top, awaiting_key),
            Mode::ScreenScraper => draw_screenscraper(d, cfg, ss_sel, editing, &edit_buf),
        };
        cab.frame_2d(BG, render);

        let now = Instant::now();
        if next > now {
            std::thread::sleep(next - now);
        }
    }
}

const MAIN_ROWS: usize = 5;
const SS_ROWS: usize = 4;

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

fn draw_main(d: &mut Screen, cfg: &Config, sel: usize) {
    let x = MARGIN;
    let mut y = MARGIN;
    d.text(x, y, 2, TEXT, "configuracoes");
    y += 40;

    let rows = [
        "Controles".to_string(),
        "ScreenScraper".to_string(),
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

fn draw_screenscraper(d: &mut Screen, cfg: &Config, sel: usize, editing: Option<Field>, buf: &str) {
    let x = MARGIN;
    let mut y = MARGIN;
    d.text(x, y, 2, TEXT, "screenscraper");
    y += 40;

    let masked: String = "*".repeat(cfg.screenscraper.dev_password.chars().count());
    let dev_id_label = match editing {
        Some(Field::DevId) => format!("Dev ID: {buf}_"),
        _ => format!(
            "Dev ID: {}",
            if cfg.screenscraper.dev_id.is_empty() {
                "(vazio)"
            } else {
                &cfg.screenscraper.dev_id
            }
        ),
    };
    let dev_pw_label = match editing {
        Some(Field::DevPassword) => format!("Dev Password: {}_", "*".repeat(buf.chars().count())),
        _ => format!(
            "Dev Password: {}",
            if cfg.screenscraper.dev_password.is_empty() {
                "(vazio)".to_string()
            } else {
                masked
            }
        ),
    };
    let rows = [
        format!(
            "Ativado: {}",
            if cfg.screenscraper.enabled {
                "sim"
            } else {
                "nao"
            }
        ),
        dev_id_label,
        dev_pw_label,
        "Voltar".to_string(),
    ];
    for (i, row) in rows.iter().enumerate() {
        draw_row(d, x, y, row, i == sel);
        y += ROW_H;
    }

    y += 12;
    d.text_wrapped(
        x,
        y,
        d.size().0.saturating_sub(MARGIN as u32 * 2),
        1,
        DIM,
        "Conta gratuita em screenscraper.fr. Sem essas duas linhas preenchidas e Ativado, \
         a estante busca fichas e capas por SS_DEVID/SS_DEVPASSWORD do ambiente, se existirem.",
    );

    d.text(
        x,
        d.size().1 as i32 - MARGIN,
        1,
        HINT,
        if editing.is_some() {
            "enter: salva  esc: cancela"
        } else {
            "enter: liga/edita  esc: volta"
        },
    );
}

/// Headless preview of one screen (`"main"` | `"controls"` | `"screenscraper"`),
/// for verification — not part of the interactive `run` loop.
pub fn capture_preview(
    cab: &mut Cabinet,
    cfg: &Config,
    screen: &str,
    path: &std::path::Path,
) -> Result<()> {
    let render = |d: &mut Screen| match screen {
        "controls" => draw_controls(d, cfg, 0, 0, None),
        "screenscraper" => draw_screenscraper(d, cfg, 0, None, ""),
        _ => draw_main(d, cfg, 0),
    };
    cab.capture_2d(BG, render, path)
        .map_err(|e| anyhow::anyhow!(e.to_string()))
}

fn draw_row(d: &mut Screen, x: i32, y: i32, label: &str, selected: bool) {
    let (cursor, color) = if selected { ("> ", TEXT) } else { ("  ", DIM) };
    d.text(x, y, 1, color, &format!("{cursor}{label}"));
}
