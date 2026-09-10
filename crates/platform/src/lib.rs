//! Platform layer: everything OS- and SDL-specific. Nothing above this line in
//! the architecture knows SDL exists.

mod audio;
mod input;
mod video;

pub use audio::AudioOut;
pub use input::{Input, PadButton, UiEvent};
pub use video::{FrameRef, PixelFormat, Video};

use sdl3::event::Event;
use sdl3::gamepad::{Button as PadBtn, Gamepad};
use sdl3::keyboard::Keycode;
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
    gamepad: Option<Gamepad>,
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
            gamepad: None,
        };
        me.open_first_gamepad();
        Ok(me)
    }

    fn open_first_gamepad(&mut self) {
        if self.gamepad.is_some() {
            return;
        }
        if let Ok(ids) = self.gamepad_subsystem.gamepads() {
            for id in ids {
                if let Ok(pad) = self.gamepad_subsystem.open(id) {
                    log::info!("gamepad: {}", pad.name().unwrap_or_default());
                    self.gamepad = Some(pad);
                    break;
                }
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

    /// Drain the event queue, update `input`, and return high-level UI intents.
    pub fn poll(&mut self, input: &mut Input) -> Vec<UiEvent> {
        let mut out = Vec::new();
        let mut added = false;
        for event in self.event_pump.poll_iter() {
            match event {
                Event::Quit { .. } => out.push(UiEvent::Quit),
                Event::GamepadAdded { .. } => added = true,
                Event::KeyDown {
                    keycode: Some(k),
                    repeat: false,
                    ..
                } => {
                    if let Some(b) = map_key(k) {
                        input.set_key(b, true);
                    }
                    match k {
                        Keycode::Escape => out.push(UiEvent::Quit),
                        Keycode::F => out.push(UiEvent::ToggleFullscreen),
                        Keycode::Backspace => out.push(UiEvent::Reset),
                        Keycode::P => out.push(UiEvent::TogglePause),
                        _ => {}
                    }
                }
                Event::KeyUp {
                    keycode: Some(k), ..
                } => {
                    if let Some(b) = map_key(k) {
                        input.set_key(b, false);
                    }
                }
                _ => {}
            }
        }
        if added {
            self.open_first_gamepad();
        }
        self.sample_gamepad(input);
        out
    }

    fn sample_gamepad(&self, input: &mut Input) {
        input.clear_pad();
        let Some(pad) = &self.gamepad else { return };
        for (btn, mapped) in GAMEPAD_MAP {
            if pad.button(btn) {
                input.set_pad(mapped, true);
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

fn map_key(k: Keycode) -> Option<PadButton> {
    Some(match k {
        Keycode::Up => PadButton::Up,
        Keycode::Down => PadButton::Down,
        Keycode::Left => PadButton::Left,
        Keycode::Right => PadButton::Right,
        Keycode::Z => PadButton::B,
        Keycode::X => PadButton::A,
        Keycode::A => PadButton::Y,
        Keycode::S => PadButton::X,
        Keycode::Q => PadButton::L,
        Keycode::W => PadButton::R,
        Keycode::Return => PadButton::Start,
        Keycode::RShift => PadButton::Select,
        _ => return None,
    })
}
