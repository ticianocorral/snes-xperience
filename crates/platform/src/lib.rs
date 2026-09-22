//! Platform layer: everything OS- and SDL-specific. Nothing above this line in
//! the architecture knows SDL exists.

mod audio;
mod cabinet;
mod input;

pub use audio::AudioOut;
pub use cabinet::{
    Cabinet, FrameRef, PanelButton, PixelFormat, Screen, SettingsButton, SettingsPanelInfo,
    ShelfButton, ShelfPanelInfo, BRAND, DEMO_BADGE_IMG,
};
pub use input::{Input, KeyMap, PadButton, UiEvent, MAX_PORTS};

use sdl3::event::Event;
use sdl3::gamepad::{Button as PadBtn, Gamepad};
use sdl3::mouse::MouseButton;
use thiserror::Error;

/// Left-button-down position, in window coordinates, or `None` for anything
/// else. Shared between `poll` and `poll_menu` so the SDL event match isn't
/// duplicated.
fn left_click_at(event: &Event) -> Option<(i32, i32)> {
    match event {
        Event::MouseButtonDown {
            mouse_btn: MouseButton::Left,
            x,
            y,
            ..
        } => Some((*x as i32, *y as i32)),
        _ => None,
    }
}

/// What `poll_menu` should do with keydowns this call. Mouse click + gamepad
/// nav (`MENU_PAD_MAP`) drive `Nav` regardless — no keyboard shortcuts left
/// for navigation itself. `CaptureKey` is the one deliberate exception: its
/// whole job is recording a keyboard key to bind for gameplay input, so it's
/// the only mode `poll_menu` still reads keydowns for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MenuMode {
    /// Grid/list browsing: mouse click, gamepad d-pad/buttons, and the mouse
    /// wheel (mapped to `Up`/`Down`) are the only inputs.
    Nav,
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
    /// Set only when `poll_menu` was called with `capture_key: true` and a
    /// key went down this frame: its raw SDL name, for rebinding a control.
    pub captured_key: Option<String>,
    /// Set only in capture mode: Escape cancels the capture instead of being
    /// captured as the new binding.
    pub capture_cancelled: bool,
    /// Left click this frame, in window coordinates (see `UiEvent::Click`).
    pub click: Option<(i32, i32)>,
}

/// One frame's worth of input while writing a free-text note (`Platform::
/// poll_text_entry`) — the pause book's note editor, the one deliberate
/// keyboard-typing exception (plan revision: everything else is
/// mouse/gamepad only).
#[derive(Default)]
pub struct TextEntryInput {
    pub quit: bool,
    /// Composed text typed this frame — usually one character, sometimes
    /// more (IME, paste-like input methods), sometimes empty.
    pub typed: String,
    pub backspace: bool,
    /// Return/Enter — commit the draft.
    pub commit: bool,
    /// Escape — discard the draft.
    pub cancel: bool,
    /// Left click this frame, in window coordinates.
    pub click: Option<(i32, i32)>,
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
        fullscreen: bool,
    ) -> Result<Cabinet, PlatformError> {
        Cabinet::new(
            &self.video_subsystem,
            &self.audio_subsystem,
            title,
            width,
            height,
            fullscreen,
        )
    }

    /// Testing hook (example harnesses): push a synthetic left-button
    /// down+up at window coordinates into the event queue, indistinguishable
    /// from a real click to `poll`/`poll_menu`.
    pub fn push_synthetic_click(&self, x: i32, y: i32) {
        for down in [true, false] {
            let ev = if down {
                Event::MouseButtonDown {
                    timestamp: 0,
                    window_id: 0,
                    which: 0,
                    mouse_btn: MouseButton::Left,
                    clicks: 1,
                    x: x as f32,
                    y: y as f32,
                }
            } else {
                Event::MouseButtonUp {
                    timestamp: 0,
                    window_id: 0,
                    which: 0,
                    mouse_btn: MouseButton::Left,
                    clicks: 1,
                    x: x as f32,
                    y: y as f32,
                }
            };
            let _ = self.sdl.event().expect("event subsystem").push_event(ev);
        }
    }

    /// Drain events for a menu screen: mouse click, mouse wheel (-> `Up`/
    /// `Down`), and gamepad d-pad/buttons (rising edge only, `MENU_PAD_MAP`)
    /// always feed `nav`. In `CaptureKey` mode only, a keydown is captured
    /// raw instead — see [`MenuMode`].
    pub fn poll_menu(&mut self, mode: MenuMode) -> MenuInput {
        use sdl3::keyboard::Keycode;
        let mut out = MenuInput::default();
        let mut devices_changed = false;
        for event in self.event_pump.poll_iter() {
            if let Some(pos) = left_click_at(&event) {
                out.click = Some(pos);
                continue;
            }
            match event {
                Event::Quit { .. } => out.quit = true,
                Event::GamepadAdded { .. } | Event::GamepadRemoved { .. } => devices_changed = true,
                Event::MouseWheel { y, .. } => {
                    if y > 0.0 {
                        out.nav.push(MenuNav::Up);
                    } else if y < 0.0 {
                        out.nav.push(MenuNav::Down);
                    }
                }
                Event::KeyDown {
                    keycode: Some(k),
                    repeat,
                    ..
                } if mode == MenuMode::CaptureKey => {
                    if repeat {
                        continue;
                    }
                    if k == Keycode::Escape {
                        out.capture_cancelled = true;
                    } else if out.captured_key.is_none() {
                        out.captured_key = Some(k.name());
                    }
                }
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

    /// Turn on OS text composition (IME, dead keys, the works) for `cab`'s
    /// window — call before the first `poll_text_entry` of a writing session
    /// (plan revision: the pause book's free-text note is the one deliberate
    /// keyboard-typing exception to "mouse/gamepad only").
    pub fn start_text_input(&self, cab: &Cabinet) {
        self.video_subsystem.text_input().start(cab.window());
    }

    /// Turn text composition back off — call once the draft is saved or
    /// cancelled, so a stray keypress elsewhere doesn't get eaten as text.
    pub fn stop_text_input(&self, cab: &Cabinet) {
        self.video_subsystem.text_input().stop(cab.window());
    }

    /// Drain events while writing free text: composed text (`Event::
    /// TextInput`, which handles layout/IME properly — no hand-rolled
    /// shift/keycode mapping), Backspace, Return (commit), Escape (cancel),
    /// and a click (to hit "Salvar"/"Cancelar" or click away). Nothing else
    /// is read — gameplay input stays untouched while a note is open.
    pub fn poll_text_entry(&mut self) -> TextEntryInput {
        use sdl3::keyboard::Keycode;
        let mut out = TextEntryInput::default();
        for event in self.event_pump.poll_iter() {
            if let Some(pos) = left_click_at(&event) {
                out.click = Some(pos);
                continue;
            }
            match event {
                Event::Quit { .. } => out.quit = true,
                Event::TextInput { text, .. } => out.typed.push_str(&text),
                Event::KeyDown {
                    keycode: Some(k),
                    repeat,
                    ..
                } => match k {
                    Keycode::Backspace => out.backspace = true,
                    Keycode::Return | Keycode::KpEnter if !repeat => out.commit = true,
                    Keycode::Escape if !repeat => out.cancel = true,
                    _ => {}
                },
                _ => {}
            }
        }
        out
    }

    pub fn open_audio(&self, sample_rate: u32) -> Result<AudioOut, PlatformError> {
        AudioOut::new(&self.audio_subsystem, sample_rate)
    }

    pub fn new_input(&self) -> Input {
        Input::new()
    }

    /// Drain the event queue, update `input` via `keymap` (gameplay D-pad/
    /// buttons only — every console/UI command is mouse-only now, reported
    /// as [`UiEvent::Click`] for the caller to resolve via
    /// `Cabinet::hit_panel_button`), and return the click plus an OS close
    /// request ([`UiEvent::CloseRequested`]), if either happened this frame.
    pub fn poll(&mut self, input: &mut Input, keymap: &KeyMap) -> Vec<UiEvent> {
        let mut out = Vec::new();
        let mut devices_changed = false;
        for event in self.event_pump.poll_iter() {
            if let Some((x, y)) = left_click_at(&event) {
                out.push(UiEvent::Click(x, y));
                continue;
            }
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
                }
                Event::KeyUp {
                    keycode: Some(k), ..
                } => {
                    if let Some(b) = keymap.pad_for(k) {
                        input.set_key(b, false);
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
