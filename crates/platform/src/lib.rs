//! Platform layer: everything OS- and SDL-specific. Nothing above this line in
//! the architecture knows SDL exists.

mod audio;
mod cabinet;
mod input;

pub use audio::AudioOut;
pub use cabinet::{Cabinet, FrameRef, PixelFormat};
pub use input::{Input, KeyMap, PadButton, UiEvent, MAX_PORTS};

use sdl3::event::Event;
use sdl3::gamepad::{Button as PadBtn, Gamepad};
use thiserror::Error;

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
    /// Characters typed this frame (search box).
    pub typed: String,
    pub backspace: bool,
    pub clear_search: bool,
    pub toggle_fullscreen: bool,
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

    /// Drain events for a menu screen: directional nav (keyboard arrows repeat;
    /// gamepad d-pad/buttons on rising edge), confirm/back, and typed search
    /// characters.
    pub fn poll_menu(&mut self) -> MenuInput {
        use sdl3::keyboard::Keycode;
        let mut out = MenuInput::default();
        let mut devices_changed = false;
        for event in self.event_pump.poll_iter() {
            match event {
                Event::Quit { .. } => out.quit = true,
                Event::GamepadAdded { .. } | Event::GamepadRemoved { .. } => devices_changed = true,
                Event::KeyDown {
                    keycode: Some(k),
                    repeat,
                    ..
                } => match k {
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
                    _ => {
                        let name = k.name();
                        if name == "Space" {
                            out.typed.push(' ');
                        } else if name.len() == 1 {
                            let c = name.chars().next().unwrap();
                            if c.is_ascii_alphanumeric() {
                                out.typed.push(c.to_ascii_lowercase());
                            }
                        }
                    }
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
