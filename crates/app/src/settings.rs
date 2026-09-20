//! The settings screen — the idle screen's/shelf's "Configurações" button,
//! redrawn on the shelf's own layout (plan revision: "a tela de
//! configurações não está no padrão do resto do app — faça a tv na mesma
//! proporção e coloque o painel"): rows inside the tube, exactly like the
//! shelf's grid area, and a flat side panel carrying the section buttons
//! ("jogo", "vídeo", "sistema", "controles") plus "Voltar". No nesting
//! deeper than a section: click a row to act on it (or gamepad nav +
//! Confirm/Back — plan revision: no keyboard shortcuts). Edits save to
//! `xperience.cfg` immediately, not on some later "apply" step — there's
//! nothing to lose by backing out.

use std::sync::mpsc::{self, Receiver};
use std::time::{Duration, Instant};

use anyhow::Result;
use xperience_platform::{
    Cabinet, MenuMode, MenuNav, PadButton, Platform, Screen, SettingsButton, SettingsPanelInfo,
};

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

/// The settings sections, in panel order — the tube shows one section's
/// rows at a time; the panel's `Section(i)` buttons switch between them
/// (plan revision: "se for interessante faça secoes na configuração").
const SECTION_NAMES: [&str; 4] = ["jogo", "vídeo", "sistema", "controles"];
const SEC_JOGO: usize = 0;
const SEC_VIDEO: usize = 1;
const SEC_SISTEMA: usize = 2;
const SEC_CONTROLES: usize = 3;

/// How many rows a section shows — shared by the nav clamps and the click
/// hit-test so they can't drift apart. `controles` is the key-bind list, so
/// its count comes from `cfg.keymap.describe()`.
fn row_count(sec: usize, cfg: &Config) -> usize {
    match sec {
        SEC_JOGO => 2,
        SEC_VIDEO => 2,
        SEC_SISTEMA => 3,
        SEC_CONTROLES => cfg.keymap.describe().len(),
        _ => 0,
    }
}

/// Which row (if any) a screen-local click y lands in, given how many rows
/// are actually on screen right now — settings rows span the full width, so
/// only y matters. Shared by the draw functions and the click handling in
/// `run`, so they can't drift apart.
fn row_at(y: i32, count: usize) -> Option<usize> {
    if y < LIST_TOP {
        return None;
    }
    let i = ((y - LIST_TOP) / ROW_H) as usize;
    (i < count).then_some(i)
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
    let mut sec = SEC_JOGO;
    let mut sel: usize = 0;
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
    let mut next = Instant::now();
    cab.set_close_button(true);

    loop {
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
            // Mouse click — the panel's section buttons (and "Voltar") come
            // first, then the rows inside the tube. Every action here also
            // has a gamepad-nav path below (`m.nav`), this just adds the
            // click-driven one (plan revision: no keyboard shortcuts).
            if let Some((cx, cy)) = m.click {
                let (ox, oy) = cab.window_to_output(cx, cy);
                if cab.hit_close_button(ox, oy) {
                    return Ok(true);
                }
                if cab.hit_minimize_button(ox, oy) {
                    cab.minimize();
                    continue;
                }
                match cab.hit_settings_button(ox, oy) {
                    Some(SettingsButton::Back) => return Ok(false),
                    Some(SettingsButton::Section(i)) => {
                        sec = i.min(SECTION_NAMES.len() - 1);
                        sel = 0;
                        controls_top = 0;
                    }
                    None => {
                        if let Some((_, ly)) = cab.hit_screen_point(ox, oy) {
                            if let Some(i) = row_at(ly, row_count(sec, cfg)) {
                                sel = i;
                                activate_row(
                                    cab, cfg, &mut sec, &mut sel, &mut awaiting_key, i,
                                    &mut core_status, &mut core_worker, &mut rename_status,
                                );
                            }
                        }
                    }
                }
            }
            for nav in &m.nav {
                match nav {
                    MenuNav::Up => sel = sel.saturating_sub(1),
                    MenuNav::Down => {
                        sel = (sel + 1).min(row_count(sec, cfg).saturating_sub(1));
                    }
                    // "controles" came from "jogo"'s Controles row, so its
                    // Back goes there; every other section's Back leaves
                    // settings entirely.
                    MenuNav::Back => {
                        if sec == SEC_CONTROLES {
                            sec = SEC_JOGO;
                            sel = 0;
                        } else {
                            return Ok(false);
                        }
                    }
                    MenuNav::Confirm => {
                        let cur = sel;
                        activate_row(
                            cab, cfg, &mut sec, &mut sel, &mut awaiting_key, cur,
                            &mut core_status, &mut core_worker, &mut rename_status,
                        );
                    }
                    // The toggle/slider rows keep their old left/right
                    // feel; everything else ignores them.
                    MenuNav::Left => adjust_row(cfg, sec, sel, cab, false),
                    MenuNav::Right => adjust_row(cfg, sec, sel, cab, true),
                    _ => {}
                }
            }
            // Keep the controles cursor inside its scroll window.
            if sec == SEC_CONTROLES {
                let (_, sh) = cab.shelf_screen_size();
                let visible = ((sh as i32 - MARGIN * 2 - 60) / ROW_H).max(1) as usize;
                if sel < controls_top {
                    controls_top = sel;
                } else if sel >= controls_top + visible {
                    controls_top = sel + 1 - visible;
                }
            }
        }

        // The flat side panel — section buttons + Voltar, rebuilt every
        // frame like the shelf's own panel.
        cab.set_settings_panel(SettingsPanelInfo {
            title: "configurações".to_string(),
            sections: SECTION_NAMES.iter().map(|s| s.to_string()).collect(),
            selected: sec,
        });

        let render = |d: &mut Screen| match sec {
            SEC_VIDEO => draw_video(d, cfg, sel),
            SEC_SISTEMA => draw_sistema(d, cfg, sel, &core_status, rename_status.as_deref()),
            SEC_CONTROLES => draw_controls(d, cfg, sel, controls_top, awaiting_key),
            _ => draw_jogo(d, cfg, sel),
        };
        cab.frame_settings(BG, render);

        crate::runner::pace_frame(&mut next, frame_time);
    }
}

/// Act on row `i` of the current section — the click path and the gamepad's
/// Confirm both land here, so they can't drift apart.
fn activate_row(
    cab: &mut Cabinet,
    cfg: &mut Config,
    sec: &mut usize,
    sel: &mut usize,
    awaiting_key: &mut Option<usize>,
    i: usize,
    core_status: &mut CoreStatus,
    core_worker: &mut Option<Receiver<CoreUpdateMsg>>,
    rename_status: &mut Option<String>,
) {
    match *sec {
        SEC_JOGO => match i {
            0 => {
                *sec = SEC_CONTROLES;
                *sel = 0;
            }
            _ => {
                cfg.runahead = (cfg.runahead + 1) % (RUNAHEAD_MAX + 1);
                let _ = cfg.save();
            }
        },
        SEC_VIDEO => match i {
            0 => {
                cfg.fullscreen = !cfg.fullscreen;
                cab.toggle_fullscreen();
                let _ = cfg.save();
            }
            _ => {
                cfg.hiss_on_static = !cfg.hiss_on_static;
                let _ = cfg.save();
                cab.set_static_hiss(cfg.hiss_on_static);
            }
        },
        SEC_SISTEMA => match i {
            0 => start_core_download(core_status, core_worker),
            1 => {
                cfg.check_updates_on_start = !cfg.check_updates_on_start;
                let _ = cfg.save();
            }
            _ => *rename_status = Some(run_rom_rename()),
        },
        SEC_CONTROLES => *awaiting_key = Some(*sel),
        _ => {}
    }
}

/// The left/right adjustments — run-ahead steps, fullscreen/check-updates
/// toggle. Rows without a horizontal feel ignore them.
fn adjust_row(cfg: &mut Config, sec: usize, sel: usize, cab: &mut Cabinet, right: bool) {
    match (sec, sel) {
        (SEC_JOGO, 1) => {
            cfg.runahead = if right {
                (cfg.runahead + 1).min(RUNAHEAD_MAX)
            } else {
                cfg.runahead.saturating_sub(1)
            };
            let _ = cfg.save();
        }
        (SEC_VIDEO, 0) => {
            cfg.fullscreen = !cfg.fullscreen;
            cab.toggle_fullscreen();
            let _ = cfg.save();
        }
        (SEC_VIDEO, 1) => {
            cfg.hiss_on_static = !cfg.hiss_on_static;
            let _ = cfg.save();
            cab.set_static_hiss(cfg.hiss_on_static);
        }
        (SEC_SISTEMA, 1) => {
            cfg.check_updates_on_start = !cfg.check_updates_on_start;
            let _ = cfg.save();
        }
        _ => {}
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
/// `draw_sistema`'s row wants — `None` if there's no core installed at all.
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

fn draw_jogo(d: &mut Screen, cfg: &Config, sel: usize) {
    let x = MARGIN;
    d.text(x, MARGIN, 2, TEXT, "jogo");
    let mut y = LIST_TOP;
    let rows = [
        "Controles".to_string(),
        format!("Run-ahead: {} quadro(s)", cfg.runahead),
    ];
    for (i, row) in rows.iter().enumerate() {
        draw_row(d, x, y, row, i == sel);
        y += ROW_H;
    }
    draw_hint(d, "clique numa opção pra mudar -- setas do gamepad ajustam, A confirma");
}

fn draw_video(d: &mut Screen, cfg: &Config, sel: usize) {
    let x = MARGIN;
    d.text(x, MARGIN, 2, TEXT, "vídeo");
    let mut y = LIST_TOP;
    let rows = [
        format!(
            "Tela cheia: {}",
            if cfg.fullscreen { "ligada" } else { "desligada" }
        ),
        format!(
            "Chiado da TV fora do ar: {}",
            if cfg.hiss_on_static { "ligado" } else { "desligado" }
        ),
    ];
    for (i, row) in rows.iter().enumerate() {
        draw_row(d, x, y, row, i == sel);
        y += ROW_H;
    }
    draw_hint(d, "clique numa opção pra alternar (o chiado toca na TV desligada)");
}

fn draw_sistema(
    d: &mut Screen,
    cfg: &Config,
    sel: usize,
    core_status: &CoreStatus,
    rename_status: Option<&str>,
) {
    let x = MARGIN;
    d.text(x, MARGIN, 2, TEXT, "sistema");
    let mut y = LIST_TOP;
    let rows = [
        core_row_label(core_status),
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
    ];
    for (i, row) in rows.iter().enumerate() {
        draw_row(d, x, y, row, i == sel);
        y += ROW_H;
    }
    draw_hint(d, "clique numa opção pra executar ou alternar");
}

fn draw_controls(d: &mut Screen, cfg: &Config, sel: usize, top: usize, awaiting: Option<usize>) {
    let x = MARGIN;
    d.text(x, MARGIN, 2, TEXT, "controles");
    let mut y = LIST_TOP;

    let binds = cfg.keymap.describe();
    let (_, sh) = d.size();
    let visible = ((sh as i32 - MARGIN * 2 - 60) / ROW_H).max(1) as usize;
    for (i, (action, key)) in binds.iter().enumerate().skip(top).take(visible) {
        let label = if awaiting == Some(i) {
            format!("{action}: aperte uma tecla... (esc cancela)")
        } else {
            format!("{action}: {key}")
        };
        draw_row(d, x, y, &label, i == sel);
        y += ROW_H;
    }

    draw_hint(d, "clique numa ação pra trocar a tecla -- esc volta pro jogo");
}

/// The one-line help at the bottom of the tube, inside it like the shelf's
/// own footer text (shared by every section so the sections read the same).
fn draw_hint(d: &mut Screen, s: &str) {
    d.text(MARGIN, d.size().1 as i32 - MARGIN, 1, HINT, s);
}

/// Headless preview of one section (dev/testing) — not part of the
/// interactive `run` loop. Accepts the section names, plus the old
/// "main"/"controls" aliases.
pub fn capture_preview(
    cab: &mut Cabinet,
    cfg: &Config,
    screen: &str,
    path: &std::path::Path,
) -> Result<()> {
    let sec = match screen {
        "video" => SEC_VIDEO,
        "sistema" => SEC_SISTEMA,
        "controls" => SEC_CONTROLES,
        _ => SEC_JOGO,
    };
    cab.set_settings_panel(SettingsPanelInfo {
        title: "configurações".to_string(),
        sections: SECTION_NAMES.iter().map(|s| s.to_string()).collect(),
        selected: sec,
    });
    let render = |d: &mut Screen| match sec {
        SEC_VIDEO => draw_video(d, cfg, 0),
        SEC_SISTEMA => draw_sistema(d, cfg, 0, &CoreStatus::Idle, None),
        SEC_CONTROLES => draw_controls(d, cfg, 0, 0, None),
        _ => draw_jogo(d, cfg, 0),
    };
    cab.set_close_button(true);
    cab.capture_settings(BG, render, path)
        .map_err(|e| anyhow::anyhow!(e.to_string()))
}

fn draw_row(d: &mut Screen, x: i32, y: i32, label: &str, selected: bool) {
    let (cursor, color) = if selected { ("> ", TEXT) } else { ("  ", DIM) };
    d.text(x, y, 1, color, &format!("{cursor}{label}"));
}
