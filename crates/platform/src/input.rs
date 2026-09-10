//! Keyboard + gamepad, folded into one SNES-shaped button matrix.
//!
//! Gamepad navigation is the primary path (plan §3.1); the keyboard binds come
//! from a [`KeyMap`] the app builds (defaults plus a config file).

use std::str::FromStr;

use sdl3::keyboard::Keycode;

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

    /// Config token, e.g. `"start"`, `"l"`.
    pub fn token(self) -> &'static str {
        match self {
            PadButton::B => "b",
            PadButton::Y => "y",
            PadButton::Select => "select",
            PadButton::Start => "start",
            PadButton::Up => "up",
            PadButton::Down => "down",
            PadButton::Left => "left",
            PadButton::Right => "right",
            PadButton::A => "a",
            PadButton::X => "x",
            PadButton::L => "l",
            PadButton::R => "r",
        }
    }
}

impl FromStr for PadButton {
    type Err = ();
    fn from_str(s: &str) -> Result<Self, ()> {
        PadButton::ALL
            .into_iter()
            .find(|b| b.token() == s)
            .ok_or(())
    }
}

/// Controller ports. The SNES has more with a multitap; two covers the plan.
pub const MAX_PORTS: usize = 2;

/// High-level events the app acts on. Meaning (e.g. "power off") is decided a
/// layer up; the platform only reports the intent.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UiEvent {
    Quit,
    ToggleFullscreen,
    Reset,
    TogglePause,
    SaveState,
    LoadState,
    Screenshot,
    NextSlot,
    PrevSlot,
    /// Advance a single frame (only acted on while paused).
    FrameStep,
    /// Held state, not an edge — `FastForward` is filtered out of the event
    /// stream and surfaced via [`Input::fast_forward`].
    FastForward,
}

impl UiEvent {
    /// Config token for bindable events (`Quit` is fixed to Esc, not listed).
    pub fn token(self) -> Option<&'static str> {
        Some(match self {
            UiEvent::ToggleFullscreen => "fullscreen",
            UiEvent::Reset => "reset",
            UiEvent::TogglePause => "pause",
            UiEvent::SaveState => "save_state",
            UiEvent::LoadState => "load_state",
            UiEvent::Screenshot => "screenshot",
            UiEvent::NextSlot => "slot_next",
            UiEvent::PrevSlot => "slot_prev",
            UiEvent::FrameStep => "frame_step",
            UiEvent::FastForward => "fast_forward",
            UiEvent::Quit => return None,
        })
    }

    pub const BINDABLE: [UiEvent; 10] = [
        UiEvent::ToggleFullscreen,
        UiEvent::Reset,
        UiEvent::TogglePause,
        UiEvent::SaveState,
        UiEvent::LoadState,
        UiEvent::Screenshot,
        UiEvent::NextSlot,
        UiEvent::PrevSlot,
        UiEvent::FrameStep,
        UiEvent::FastForward,
    ];
}

/// Keyboard bindings. Built from [`KeyMap::default`] then overridden per the
/// app's config. Gamepad bindings stay fixed (SDL's controller DB already
/// normalises devices).
#[derive(Debug, Clone, Default)]
pub struct KeyMap {
    pad: Vec<(Keycode, PadButton)>,
    ui: Vec<(Keycode, UiEvent)>,
}

impl KeyMap {
    /// The built-in layout (arrows = dpad, Z/X = B/A, …).
    pub fn defaults() -> Self {
        let pad = [
            ("Up", PadButton::Up),
            ("Down", PadButton::Down),
            ("Left", PadButton::Left),
            ("Right", PadButton::Right),
            ("Z", PadButton::B),
            ("X", PadButton::A),
            ("A", PadButton::Y),
            ("S", PadButton::X),
            ("Q", PadButton::L),
            ("W", PadButton::R),
            ("Return", PadButton::Start),
            ("Right Shift", PadButton::Select),
        ];
        let ui = [
            ("F", UiEvent::ToggleFullscreen),
            ("Backspace", UiEvent::Reset),
            ("P", UiEvent::TogglePause),
            ("F2", UiEvent::SaveState),
            ("F4", UiEvent::LoadState),
            ("F12", UiEvent::Screenshot),
            ("]", UiEvent::NextSlot),
            ("[", UiEvent::PrevSlot),
            ("\\", UiEvent::FrameStep),
            ("Tab", UiEvent::FastForward),
        ];
        let mut m = KeyMap::default();
        for (name, b) in pad {
            m.bind_pad(name, b).expect("valid default key name");
        }
        for (name, e) in ui {
            m.bind_ui(name, e).expect("valid default key name");
        }
        m
    }

    /// Parse an SDL key name (`"Left Shift"`, `"F2"`, `"]"`). Case-sensitive as
    /// SDL defines it.
    pub fn parse_key(name: &str) -> Option<Keycode> {
        Keycode::from_name(name)
    }

    /// Rebind a pad button; the previous key(s) for it are dropped.
    pub fn bind_pad(&mut self, key_name: &str, button: PadButton) -> Result<(), String> {
        let k = Self::parse_key(key_name).ok_or_else(|| format!("unknown key {key_name:?}"))?;
        self.pad.retain(|&(kk, bb)| kk != k && bb != button);
        self.pad.push((k, button));
        Ok(())
    }

    pub fn bind_ui(&mut self, key_name: &str, event: UiEvent) -> Result<(), String> {
        let k = Self::parse_key(key_name).ok_or_else(|| format!("unknown key {key_name:?}"))?;
        self.ui.retain(|&(kk, ee)| kk != k && ee != event);
        self.ui.push((k, event));
        Ok(())
    }

    /// `(action token, SDL key name)` for every current bind, pad first — for
    /// writing out a config file.
    pub fn describe(&self) -> Vec<(String, String)> {
        let mut v = Vec::new();
        for b in PadButton::ALL {
            if let Some(&(k, _)) = self.pad.iter().find(|&&(_, bb)| bb == b) {
                v.push((b.token().to_string(), k.name()));
            }
        }
        for e in UiEvent::BINDABLE {
            if let (Some(t), Some(&(k, _))) = (e.token(), self.ui.iter().find(|&&(_, ee)| ee == e))
            {
                v.push((t.to_string(), k.name()));
            }
        }
        v
    }

    pub(crate) fn pad_for(&self, k: Keycode) -> Option<PadButton> {
        self.pad.iter().find(|&&(kk, _)| kk == k).map(|&(_, b)| b)
    }

    pub(crate) fn ui_for(&self, k: Keycode) -> Option<UiEvent> {
        self.ui.iter().find(|&&(kk, _)| kk == k).map(|&(_, e)| e)
    }
}

/// Per-frame button state. Filled by `Platform::poll`. Keyboard drives port 0;
/// gamepad *n* drives port *n*.
#[derive(Default)]
pub struct Input {
    keys: [bool; 12],
    pads: [[bool; 12]; MAX_PORTS],
    fast_forward: bool,
}

impl Input {
    pub fn new() -> Self {
        Self::default()
    }

    /// Button `b` held on `port` this frame (keyboard counts for port 0).
    pub fn held(&self, port: usize, b: PadButton) -> bool {
        let i = b as usize;
        (port == 0 && self.keys[i]) || (port < MAX_PORTS && self.pads[port][i])
    }

    /// The fast-forward key is down.
    pub fn fast_forward(&self) -> bool {
        self.fast_forward
    }

    pub(crate) fn set_key(&mut self, b: PadButton, down: bool) {
        self.keys[b as usize] = down;
    }

    pub(crate) fn set_pad(&mut self, port: usize, b: PadButton, down: bool) {
        if port < MAX_PORTS {
            self.pads[port][b as usize] = down;
        }
    }

    pub(crate) fn set_fast_forward(&mut self, down: bool) {
        self.fast_forward = down;
    }

    pub(crate) fn clear_pads(&mut self) {
        self.pads = [[false; 12]; MAX_PORTS];
    }
}
