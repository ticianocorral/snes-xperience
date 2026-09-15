//! The idle/root screen: TV off, no game loaded — the app's home state, and
//! also literally the "cartridge ejected" screen (plan revision: one screen,
//! not two — startup, backing out of the shelf, and ejecting a game all land
//! here). In place of a game's logo/cartridge art there's the console's own
//! brand logo (`assets/console.png`, optional) and an "Inserir cartucho"
//! button; "Configuracoes" sits in the panel's footer. This is the outermost
//! screen now: a gamepad's Back button here, same as closing the window,
//! ends the app (plan revision: mouse/gamepad only, no keyboard shortcuts).

use std::path::Path;

use anyhow::Result;
use xperience_platform::{Cabinet, MenuMode, MenuNav, PanelButton, Platform};

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
    /// "Configuracoes" clicked — the only way into settings from here (no
    /// keyboard shortcut, mouse/gamepad only).
    OpenSettings,
}

/// Run the idle screen until the player opens the shelf or quits.
/// `static_level` is the signal-off snow to show — the steady dim hiss at
/// startup or after Esc on the shelf, or whatever level a just-ejected game
/// settled on (already steady by the time Eject fires, so no extra fade is
/// needed here either way).
pub fn run(plat: &mut Platform, cab: &mut Cabinet, static_level: f32) -> Result<IdleExit> {
    // Whatever game was loaded before (if any) is gone now — without this the
    // panel keeps showing its stale logo/commands instead of the "Inserir
    // cartucho" button, and there'd be nothing to click.
    cab.clear_panel();
    // The console's own brand logo (plan revision), same local-file
    // convention as per-game art — optional, so a missing file just falls
    // back to plain text (`draw_panel`'s idle branch handles that part).
    let console_png = crate::dirs::assets_dir().join("console.png");
    let console_logo = console_png.exists().then(|| decode_art(&console_png, 640));
    let console_logo = match console_logo {
        Some(Ok(img)) => Some(img),
        Some(Err(e)) => {
            log::warn!("console logo {}: {e}", console_png.display());
            None
        }
        None => None,
    };
    cab.set_console_logo(
        console_logo
            .as_ref()
            .map(|(w, h, d)| (*w, *h, d.as_slice())),
    );
    loop {
        let m = plat.poll_menu(MenuMode::Nav);
        if m.quit {
            return Ok(IdleExit::Quit);
        }
        if let Some((x, y)) = m.click {
            let (ox, oy) = cab.window_to_output(x, y);
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

/// Decode `assets/console.png` to tightly-packed RGBA, downscaled for memory
/// — same helper shape as `runner::decode_art`/`shelf::decode_art`, just for
/// this screen's one fixed (not per-game) image.
fn decode_art(path: &Path, max: u32) -> anyhow::Result<(u32, u32, Vec<u8>)> {
    let img = image::open(path)?.thumbnail(max, max).to_rgba8();
    let (w, h) = img.dimensions();
    Ok((w, h, img.into_raw()))
}
