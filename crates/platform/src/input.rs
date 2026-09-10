//! Keyboard + gamepad, folded into one SNES-shaped button matrix.
//!
//! Gamepad navigation is the primary path (plan §3.1); the keyboard map is the
//! fallback and is fixed for Phase 0.

/// SNES / libretro joypad buttons, in libretro id order.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(usize)]
pub enum PadButton {
    B = 0,
    Y = 1,
    Select = 2,
    Start = 3,
    Up = 4,
    Down = 5,
    Left = 6,
    Right = 7,
    A = 8,
    X = 9,
    L = 10,
    R = 11,
}

impl PadButton {
    pub const ALL: [PadButton; 12] = [
        PadButton::B,
        PadButton::Y,
        PadButton::Select,
        PadButton::Start,
        PadButton::Up,
        PadButton::Down,
        PadButton::Left,
        PadButton::Right,
        PadButton::A,
        PadButton::X,
        PadButton::L,
        PadButton::R,
    ];
}

/// High-level events the app acts on. Meaning (e.g. "power off") is decided a
/// layer up; the platform only reports the intent.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UiEvent {
    Quit,
    ToggleFullscreen,
    Reset,
    TogglePause,
}

/// Per-frame button state. Filled by `Platform::poll`.
#[derive(Default)]
pub struct Input {
    keys: [bool; 12],
    pad: [bool; 12],
}

impl Input {
    pub fn new() -> Self {
        Self::default()
    }

    /// Held on keyboard or gamepad this frame.
    pub fn held(&self, b: PadButton) -> bool {
        let i = b as usize;
        self.keys[i] || self.pad[i]
    }

    pub(crate) fn set_key(&mut self, b: PadButton, down: bool) {
        self.keys[b as usize] = down;
    }

    pub(crate) fn set_pad(&mut self, b: PadButton, down: bool) {
        self.pad[b as usize] = down;
    }

    pub(crate) fn clear_pad(&mut self) {
        self.pad = [false; 12];
    }
}
