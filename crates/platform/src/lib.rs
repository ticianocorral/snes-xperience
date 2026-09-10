//! Platform layer: everything OS- and SDL-specific. Nothing above this line in
//! the architecture knows SDL exists.

mod audio;
mod input;
mod video;

pub use audio::AudioOut;
pub use input::{Input, KeyMap, PadButton, UiEvent, MAX_PORTS};
pub use video::{FrameRef, PixelFormat, Video};

use sdl3::event::Event;
use sdl3::gamepad::{Button as PadBtn, Gamepad};
use thiserror::Error;

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
}

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

    pub fn create_window(
        &self,
        title: &str,
        width: u32,
        height: u32,
    ) -> Result<Video, PlatformError> {
        Video::new(&self.video_subsystem, title, width, height)
    }

    pub fn open_audio(&self, sample_rate: u32) -> Result<AudioOut, PlatformError> {
        AudioOut::new(&self.audio_subsystem, sample_rate)
    }

    pub fn new_input(&self) -> Input {
        Input::new()
    }

    /// Drain the event queue, update `input` via `keymap`, and return the UI
    /// intents that fired this frame. Esc always quits.
    pub fn poll(&mut self, input: &mut Input, keymap: &KeyMap) -> Vec<UiEvent> {
        use sdl3::keyboard::Keycode;
        let mut out = Vec::new();
        let mut devices_changed = false;
        for event in self.event_pump.poll_iter() {
            match event {
                Event::Quit { .. } => out.push(UiEvent::Quit),
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
