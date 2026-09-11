//! Platform layer: everything OS- and SDL-specific. Nothing above this line in
//! the architecture knows SDL exists.

mod audio;
mod cabinet;
mod input;

pub use audio::AudioOut;
pub use cabinet::{Cabinet, FrameRef, PixelFormat, Screen};
pub use input::{Input, KeyMap, PadButton, UiEvent, MAX_PORTS};

use sdl3::event::Event;
use sdl3::gamepad::{Button as PadBtn, Gamepad};
use thiserror::Error;

/// What `poll_menu` should do with keydowns this call.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MenuMode {
    /// Grid/list browsing: arrows and paging are nav, `F`/`O` are hotkeys,
    /// anything else printable is `typed` (the shelf's search box).
    Nav,
    /// Editing one text field: every printable key types (including `f`/`o`
    /// — no hotkeys), Return commits (`Confirm`), Escape cancels (`Back`).
    TextEntry,
    /// Rebinding a control: the next key pressed comes back raw in
    /// `captured_key`; Escape cancels instead of being captured.
    CaptureKey,
}

/// A directional / confirm / back intent from the selector's controls.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MenuNav {
    Up,
    Down,
    Left,
    Right,
    Confirm,
    Back,
    PageUp,
    PageDown,
    Home,
    End,
}

/// One frame's worth of menu input.
#[derive(Default)]
pub struct MenuInput {
    pub quit: bool,
    pub nav: Vec<MenuNav>,
    /// Characters typed this frame (search box, or a settings text field).
    pub typed: String,
    pub backspace: bool,
    pub clear_search: bool,
    pub toggle_fullscreen: bool,
    /// `O` on the shelf — opens the settings screen.
    pub open_settings: bool,
    /// Set only when `poll_menu` was called with `capture_key: true` and a
    /// key went down this frame: its raw SDL name, for rebinding a control.
    pub captured_key: Option<String>,
    /// Set only in capture mode: Escape cancels the capture instead of being
    /// captured as the new binding.
    pub capture_cancelled: bool,
}

#[derive(Debug, Error)]
pub enum PlatformError {
    #[error("SDL error: {0}")]
    Sdl(String),
}

impl From<sdl3::Error> for PlatformError {
    fn from(e: sdl3::Error) -> Self {
        PlatformError::Sdl(e.to_string())
    }
}

impl From<String> for PlatformError {
    fn from(e: String) -> Self {
        PlatformError::Sdl(e)
    }
}

/// Owns the SDL context and its subsystems. Create one, keep it alive for the
/// whole process.
pub struct Platform {
    pub sdl: sdl3::Sdl,
    pub video_subsystem: sdl3::VideoSubsystem,
    pub audio_subsystem: sdl3::AudioSubsystem,
    event_pump: sdl3::EventPump,
    gamepad_subsystem: sdl3::GamepadSubsystem,
    /// Open gamepads, in order — index n drives port n.
    gamepads: Vec<Gamepad>,
    /// Rising-edge tracking for `poll_menu`'s gamepad buttons.
    menu_prev: [bool; MENU_PAD_MAP.len()],
}

const MENU_PAD_MAP: [(PadBtn, MenuNav); 8] = [
    (PadBtn::DPadUp, MenuNav::Up),
    (PadBtn::DPadDown, MenuNav::Down),
    (PadBtn::DPadLeft, MenuNav::Left),
    (PadBtn::DPadRight, MenuNav::Right),
    (PadBtn::South, MenuNav::Confirm),
    (PadBtn::East, MenuNav::Back),
    (PadBtn::LeftShoulder, MenuNav::PageUp),
    (PadBtn::RightShoulder, MenuNav::PageDown),
];

impl Platform {
    pub fn new() -> Result<Self, PlatformError> {
        let sdl = sdl3::init()?;
        let video_subsystem = sdl.video()?;
        let audio_subsystem = sdl.audio()?;
        let gamepad_subsystem = sdl.gamepad()?;
        let event_pump = sdl.event_pump()?;
        let mut me = Self {
            sdl,
            video_subsystem,
            audio_subsystem,
            event_pump,
            gamepad_subsystem,
            gamepads: Vec::new(),
            menu_prev: [false; MENU_PAD_MAP.len()],
        };
        me.refresh_gamepads();
        Ok(me)
    }

    /// Re-open the connected gamepads (up to `MAX_PORTS`). Cheap enough to run
    /// on every add/remove event.
    fn refresh_gamepads(&mut self) {
        self.gamepads.clear();
        let Ok(ids) = self.gamepad_subsystem.gamepads() else {
            return;
        };
        for id in ids.into_iter().take(input::MAX_PORTS) {
            if let Ok(pad) = self.gamepad_subsystem.open(id) {
                log::info!(
                    "gamepad port {}: {}",
                    self.gamepads.len(),
                    pad.name().unwrap_or_default()
                );
                self.gamepads.push(pad);
            }
        }
    }

    /// The one window: a dark cabinet with the screen recessed into it. Both the
    /// game and the selector draw into that screen area; nothing recreates it.
    pub fn create_cabinet(
        &self,
        title: &str,
        width: u32,
        height: u32,
    ) -> Result<Cabinet, PlatformError> {
        Cabinet::new(&self.video_subsystem, title, width, height)
    }

    /// Drain events for a menu screen. See [`MenuMode`] for what each mode
    /// does with a keydown; gamepad d-pad/buttons (rising edge only) always
    /// feed `nav` regardless of mode.
    pub fn poll_menu(&mut self, mode: MenuMode) -> MenuInput {
        use sdl3::keyboard::Keycode;
        let mut out = MenuInput::default();
        let mut devices_changed = false;
        for event in self.event_pump.poll_iter() {
            match event {
                Event::Quit { .. } => out.quit = true,
                Event::GamepadAdded { .. } | Event::GamepadRemoved { .. } => devices_changed = true,
                Event::KeyDown {
                    keycode: Some(k),
                    keymod,
                    repeat,
                    ..
                } => match mode {
                    MenuMode::CaptureKey => {
                        if repeat {
                            continue;
                        }
                        if k == Keycode::Escape {
                            out.capture_cancelled = true;
                        } else if out.captured_key.is_none() {
                            out.captured_key = Some(k.name());
                        }
                    }
                    MenuMode::Nav => match k {
                        Keycode::Up => out.nav.push(MenuNav::Up),
                        Keycode::Down => out.nav.push(MenuNav::Down),
                        Keycode::Left => out.nav.push(MenuNav::Left),
                        Keycode::Right => out.nav.push(MenuNav::Right),
                        Keycode::Return | Keycode::KpEnter => out.nav.push(MenuNav::Confirm),
                        Keycode::Escape => out.nav.push(MenuNav::Back),
                        Keycode::PageUp => out.nav.push(MenuNav::PageUp),
                        Keycode::PageDown => out.nav.push(MenuNav::PageDown),
                        Keycode::Home => out.nav.push(MenuNav::Home),
                        Keycode::End => out.nav.push(MenuNav::End),
                        Keycode::Backspace => out.backspace = true,
                        Keycode::F if !repeat => out.toggle_fullscreen = true,
                        Keycode::O if !repeat => out.open_settings = true,
                        _ => {
                            if let Some(c) = char_for_key(k, keymod) {
                                out.typed.push(c);
                            }
                        }
                    },
                    MenuMode::TextEntry => match k {
                        Keycode::Return | Keycode::KpEnter => out.nav.push(MenuNav::Confirm),
                        Keycode::Escape => out.nav.push(MenuNav::Back),
                        Keycode::Backspace => out.backspace = true,
                        _ => {
                            if let Some(c) = char_for_key(k, keymod) {
                                out.typed.push(c);
                            }
                        }
                    },
                },
                _ => {}
            }
        }
        if devices_changed {
            self.refresh_gamepads();
        }
        // Gamepad: rising edges only.
        let pad = self.gamepads.first();
        for (i, (btn, nav)) in MENU_PAD_MAP.iter().enumerate() {
            let down = pad.map(|p| p.button(*btn)).unwrap_or(false);
            if down && !self.menu_prev[i] {
                out.nav.push(*nav);
            }
            self.menu_prev[i] = down;
        }
        out
    }

    pub fn open_audio(&self, sample_rate: u32) -> Result<AudioOut, PlatformError> {
        AudioOut::new(&self.audio_subsystem, sample_rate)
    }

    pub fn new_input(&self) -> Input {
        Input::new()
    }

    /// Drain the event queue, update `input` via `keymap`, and return the UI
    /// intents that fired this frame. Esc emits [`UiEvent::Quit`] ("leave this
    /// screen"); an OS close request emits [`UiEvent::CloseRequested`].
    pub fn poll(&mut self, input: &mut Input, keymap: &KeyMap) -> Vec<UiEvent> {
        use sdl3::keyboard::Keycode;
        let mut out = Vec::new();
        let mut devices_changed = false;
        for event in self.event_pump.poll_iter() {
            match event {
                Event::Quit { .. } => out.push(UiEvent::CloseRequested),
                Event::GamepadAdded { .. } | Event::GamepadRemoved { .. } => devices_changed = true,
                Event::KeyDown {
                    keycode: Some(k),
                    repeat: false,
                    ..
                } => {
                    if let Some(b) = keymap.pad_for(k) {
                        input.set_key(b, true);
                    }
                    if k == Keycode::Escape {
                        out.push(UiEvent::Quit);
                    } else if let Some(e) = keymap.ui_for(k) {
                        match e {
                            // Held state, not an edge.
                            UiEvent::FastForward => input.set_fast_forward(true),
                            _ => out.push(e),
                        }
                    }
                }
                Event::KeyUp {
                    keycode: Some(k), ..
                } => {
                    if let Some(b) = keymap.pad_for(k) {
                        input.set_key(b, false);
                    }
                    if keymap.ui_for(k) == Some(UiEvent::FastForward) {
                        input.set_fast_forward(false);
                    }
                }
                _ => {}
            }
        }
        if devices_changed {
            self.refresh_gamepads();
        }
        self.sample_gamepads(input);
        out
    }

    fn sample_gamepads(&self, input: &mut Input) {
        input.clear_pads();
        for (port, pad) in self.gamepads.iter().enumerate() {
            for (btn, mapped) in GAMEPAD_MAP {
                if pad.button(btn) {
                    input.set_pad(port, mapped, true);
                }
            }
        }
    }
}

/// A key's printable character, respecting Shift — for search boxes and
/// settings text fields (credentials need more than the old lowercase-only
/// search alphabet). `Keycode` names the *unshifted* glyph (SDL's own
/// convention), so shifting is done here, not trusted from the OS.
fn char_for_key(k: sdl3::keyboard::Keycode, keymod: sdl3::keyboard::Mod) -> Option<char> {
    use sdl3::keyboard::Mod;
    let shift = keymod.intersects(Mod::LSHIFTMOD | Mod::RSHIFTMOD);
    let name = k.name();
    if name == "Space" {
        return Some(' ');
    }
    let mut chars = name.chars();
    let c = chars.next()?;
    if chars.next().is_some() {
        return None; // multi-char name ("Backspace", "F1", ...): not printable
    }
    Some(shift_char(c, shift))
}

fn shift_char(c: char, shift: bool) -> char {
    if c.is_ascii_alphabetic() {
        return if shift {
            c.to_ascii_uppercase()
        } else {
            c.to_ascii_lowercase()
        };
    }
    if !shift {
        return c;
    }
    match c {
        '1' => '!',
        '2' => '@',
        '3' => '#',
        '4' => '$',
        '5' => '%',
        '6' => '^',
        '7' => '&',
        '8' => '*',
        '9' => '(',
        '0' => ')',
        '-' => '_',
        '=' => '+',
        '[' => '{',
        ']' => '}',
        '\\' => '|',
        ';' => ':',
        '\'' => '"',
        ',' => '<',
        '.' => '>',
        '/' => '?',
        '`' => '~',
        other => other,
    }
}

const GAMEPAD_MAP: [(PadBtn, PadButton); 12] = [
    (PadBtn::DPadUp, PadButton::Up),
    (PadBtn::DPadDown, PadButton::Down),
    (PadBtn::DPadLeft, PadButton::Left),
    (PadBtn::DPadRight, PadButton::Right),
    (PadBtn::South, PadButton::B),
    (PadBtn::East, PadButton::A),
    (PadBtn::West, PadButton::Y),
    (PadBtn::North, PadButton::X),
    (PadBtn::LeftShoulder, PadButton::L),
    (PadBtn::RightShoulder, PadButton::R),
    (PadBtn::Back, PadButton::Select),
    (PadBtn::Start, PadButton::Start),
];
