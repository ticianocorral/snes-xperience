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
/// layer up; the platform only reports the intent. None of these are
/// keyboard-bindable any more (plan revision: mouse/gamepad only for every
/// console/UI command) — they're only ever produced by a panel-button
/// `Click` being resolved by the caller, or (`CloseRequested`) the OS.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UiEvent {
    /// Power toggle — off saves/signals-off (console dark, cartridge still in
    /// the slot, plan §3.3), on again resumes exactly where it was. Always
    /// mouse-driven now (the panel's Power button); a click while already
    /// mid-transition does nothing new.
    Quit,
    /// The OS asked the window to close (red button, Cmd-Q, `SIGTERM`). Always
    /// means "tear the whole app down", never "go back".
    CloseRequested,
    /// **Ejetar** — only takes effect once the console is off; while it's
    /// still on the lock resists (plan §3.3).
    Eject,
    Reset,
    TogglePause,
    SaveState,
    LoadState,
    NextSlot,
    /// Flip cheat `usize` on/off directly — a panel click addresses its row,
    /// so there's no separate cursor-move step any more.
    CheatToggle(usize),
    /// Capture straight into the current note slot (plan §3.4, plan
    /// revision: a fixed 1..=15 slot, not an open-ended timestamped list) —
    /// not a keepsake of the cabinet, a page for the password/map/progress
    /// screen on-screen right now.
    NoteCapture,
    /// Cycle which of the 15 note slots `NoteCapture` targets (plan
    /// revision) — the same slot the pause book's right page shows.
    NoteSlotNext,
    /// Advance a single frame — only offered (and only acts) while paused,
    /// via the pause book's own "Avancar quadro" button.
    FrameStep,
    /// Turbo toggle — click-driven, not held: on until clicked again.
    ToggleFastForward,
    /// Pause book: step the right page to an earlier/later note slot.
    NotePrev,
    NoteNext,
    /// Pause book: open the free-text note editor (the one deliberate
    /// keyboard-typing exception — see `Platform::poll_text_entry`).
    NoteWriteStart,
    /// Pause book: toggle whether the shown slot resists a future "Nota"
    /// overwrite (plan revision).
    NotePinToggle,
    /// Pause book: open the editor for the shown slot's caption (plan
    /// revision) — same keyboard-typing exception as `NoteWriteStart`.
    NoteNameStart,
    /// Left mouse button went down, in **window** coordinates — the caller
    /// (which owns the `Cabinet`) converts to output/canvas space via
    /// `Cabinet::window_to_output` before hit-testing anything.
    Click(i32, i32),
}

/// Keyboard bindings for actual SNES gameplay input (D-pad/face buttons) —
/// the one thing that's still keyboard by default, since a gamepad isn't
/// guaranteed to be plugged in. Built from [`KeyMap::default`] then
/// overridden per the app's config. Gamepad bindings stay fixed (SDL's
/// controller DB already normalises devices). Every console/UI command
/// (eject, reset, pause, save state, ...) used to have a rebindable key here
/// too; it doesn't any more — those are mouse/gamepad-menu only (plan
/// revision), so there's nothing left to bind for them.
#[derive(Debug, Clone, Default)]
pub struct KeyMap {
    pad: Vec<(Keycode, PadButton)>,
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
        let mut m = KeyMap::default();
        for (name, b) in pad {
            m.bind_pad(name, b).expect("valid default key name");
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

    /// `(action token, SDL key name)` for every current bind — for writing
    /// out a config file.
    pub fn describe(&self) -> Vec<(String, String)> {
        let mut v = Vec::new();
        for b in PadButton::ALL {
            if let Some(&(k, _)) = self.pad.iter().find(|&&(_, bb)| bb == b) {
                v.push((b.token().to_string(), k.name()));
            }
        }
        v
    }

    pub(crate) fn pad_for(&self, k: Keycode) -> Option<PadButton> {
        self.pad.iter().find(|&&(kk, _)| kk == k).map(|&(_, b)| b)
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

    /// Turbo is a click-to-toggle now (no held key) — the caller (`runner`)
    /// owns the on/off state and just tells `Input` about it each frame.
    pub fn set_fast_forward(&mut self, on: bool) {
        self.fast_forward = on;
    }

    pub(crate) fn clear_pads(&mut self) {
        self.pads = [[false; 12]; MAX_PORTS];
    }
}
