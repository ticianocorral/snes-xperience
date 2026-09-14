//! The idle/root screen: TV off, no game loaded — the app's home state.
//! Shown at startup, after Esc on the shelf, and after ejecting a game.
//! In place of the panel's logo/title (plan §3.2, item 1) there's a single
//! "Inserir cartucho" button that opens the shelf. This is the outermost
//! screen now: Esc here, same as closing the window, ends the app.

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
    loop {
        let m = plat.poll_menu(MenuMode::Nav);
        if m.quit {
            return Ok(IdleExit::Quit);
        }
        if m.toggle_fullscreen {
            cab.toggle_fullscreen();
        }
        if let Some((x, y)) = m.click {
            let (ox, oy) = cab.window_to_output(x, y);
            if cab.hit_panel_button(ox, oy) == Some(PanelButton::Insert) {
                return Ok(IdleExit::OpenShelf);
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
