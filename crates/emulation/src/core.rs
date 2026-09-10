//! Safe-ish wrapper around a dynamically loaded libretro core.
//!
//! libretro's callbacks are bare C function pointers with no user-data slot, so
//! the frontend state they need is reached through a thread-local pointer that
//! is only non-null for the duration of a `retro_run` / `retro_load_game` call.

use std::cell::Cell;
use std::ffi::{c_void, CStr, CString};
use std::os::raw::{c_char, c_uint};
use std::path::{Path, PathBuf};
use std::ptr;

use libloading::{Library, Symbol};

use crate::sys;
use crate::sys::*;

/// libretro joypad button, in libretro id order. `Frame`-independent.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum Button {
    B = RETRO_DEVICE_ID_JOYPAD_B,
    Y = RETRO_DEVICE_ID_JOYPAD_Y,
    Select = RETRO_DEVICE_ID_JOYPAD_SELECT,
    Start = RETRO_DEVICE_ID_JOYPAD_START,
    Up = RETRO_DEVICE_ID_JOYPAD_UP,
    Down = RETRO_DEVICE_ID_JOYPAD_DOWN,
    Left = RETRO_DEVICE_ID_JOYPAD_LEFT,
    Right = RETRO_DEVICE_ID_JOYPAD_RIGHT,
    A = RETRO_DEVICE_ID_JOYPAD_A,
    X = RETRO_DEVICE_ID_JOYPAD_X,
    L = RETRO_DEVICE_ID_JOYPAD_L,
    R = RETRO_DEVICE_ID_JOYPAD_R,
}

impl Button {
    pub const ALL: [Button; 12] = [
        Button::B,
        Button::Y,
        Button::Select,
        Button::Start,
        Button::Up,
        Button::Down,
        Button::Left,
        Button::Right,
        Button::A,
        Button::X,
        Button::L,
        Button::R,
    ];
}

pub const MAX_PORTS: usize = 2;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PixelFormat {
    Rgb1555,
    Xrgb8888,
    Rgb565,
}

impl PixelFormat {
    pub fn bytes_per_pixel(self) -> usize {
        match self {
            PixelFormat::Rgb1555 | PixelFormat::Rgb565 => 2,
            PixelFormat::Xrgb8888 => 4,
        }
    }
}

/// One video frame handed up by the core. Owns its pixels.
#[derive(Debug, Clone)]
pub struct Frame {
    pub width: u32,
    pub height: u32,
    /// Row stride in bytes, as reported by the core (may exceed width * bpp).
    pub pitch: usize,
    pub format: PixelFormat,
    pub pixels: Vec<u8>,
}

#[derive(Debug, Clone, Copy)]
pub struct AvInfo {
    pub base_width: u32,
    pub base_height: u32,
    pub max_width: u32,
    pub max_height: u32,
    pub aspect_ratio: f32,
    pub fps: f64,
    pub sample_rate: f64,
}

#[derive(Debug, thiserror::Error)]
pub enum CoreError {
    #[error("could not open core library at {path}: {source}")]
    Open {
        path: PathBuf,
        #[source]
        source: libloading::Error,
    },
    #[error("core is missing symbol `{0}`")]
    MissingSymbol(&'static str),
    #[error("core reports libretro API version {found}, frontend speaks {expected}")]
    ApiMismatch { found: u32, expected: u32 },
    #[error("core rejected the ROM (retro_load_game returned false)")]
    LoadRejected,
    #[error("core requires a real file path for this ROM but none was given")]
    NeedsFullPath,
    #[error(transparent)]
    Io(#[from] std::io::Error),
}

/// Frontend state the C callbacks read and write. Boxed so its address is stable.
struct CallbackState {
    pixel_format: PixelFormat,
    system_directory: CString,
    save_directory: CString,
    /// Button matrix the frontend fills before each `run()`.
    input: [[bool; 16]; MAX_PORTS],
    /// Latest frame, taken out by the frontend after `run()`.
    frame: Option<Frame>,
    /// `true` when the core signalled a duped frame (no new video this run).
    frame_duped: bool,
    /// Interleaved S16 stereo, accumulated across the run, drained by frontend.
    audio: Vec<i16>,
    /// Legacy core-option variables: name -> chosen value (first listed option).
    variables: Vec<(CString, CString)>,
    variables_dirty: bool,
    av_info: AvInfo,
    av_info_dirty: bool,
}

thread_local! {
    static CB: Cell<*mut CallbackState> = const { Cell::new(ptr::null_mut()) };
}

fn with_cb<R>(f: impl FnOnce(&mut CallbackState) -> R) -> Option<R> {
    CB.with(|c| {
        let p = c.get();
        if p.is_null() {
            None
        } else {
            // Safety: set_scope guarantees the pointer outlives the closure and
            // that no other &mut alias exists on this thread while it is set.
            Some(f(unsafe { &mut *p }))
        }
    })
}

pub struct Core {
    // `lib` must outlive every symbol; declared last so it drops last.
    api: CoreApi,
    state: Box<CallbackState>,
    system_name: String,
    system_version: String,
    needs_fullpath: bool,
    valid_extensions: Vec<String>,
    loaded: bool,
    #[allow(dead_code)]
    lib: Library,
}

impl Core {
    /// dlopen the core and resolve its entry points. Does not init it yet.
    pub fn load(path: impl AsRef<Path>) -> Result<Self, CoreError> {
        let path = path.as_ref().to_path_buf();
        // Safety: loading arbitrary native code. The path comes from the user.
        let lib = unsafe { Library::new(&path) }.map_err(|source| CoreError::Open {
            path: path.clone(),
            source,
        })?;

        macro_rules! sym {
            ($name:literal, $t:ty) => {{
                let s: Symbol<$t> = unsafe { lib.get(concat!($name, "\0").as_bytes()) }
                    .map_err(|_| CoreError::MissingSymbol($name))?;
                *s
            }};
        }

        let api = CoreApi {
            retro_api_version: sym!("retro_api_version", sys::FnU32),
            retro_get_system_info: sym!("retro_get_system_info", sys::FnGetSystemInfo),
            retro_get_system_av_info: sym!("retro_get_system_av_info", sys::FnGetSystemAvInfo),
            retro_set_environment: sym!("retro_set_environment", sys::FnSetEnvironment),
            retro_set_video_refresh: sym!("retro_set_video_refresh", sys::FnSetVideoRefresh),
            retro_set_audio_sample: sym!("retro_set_audio_sample", sys::FnSetAudioSample),
            retro_set_audio_sample_batch: sym!(
                "retro_set_audio_sample_batch",
                sys::FnSetAudioSampleBatch
            ),
            retro_set_input_poll: sym!("retro_set_input_poll", sys::FnSetInputPoll),
            retro_set_input_state: sym!("retro_set_input_state", sys::FnSetInputState),
            retro_set_controller_port_device: sym!(
                "retro_set_controller_port_device",
                sys::FnSetControllerPortDevice
            ),
            retro_init: sym!("retro_init", sys::FnVoid),
            retro_deinit: sym!("retro_deinit", sys::FnVoid),
            retro_load_game: sym!("retro_load_game", sys::FnLoadGame),
            retro_unload_game: sym!("retro_unload_game", sys::FnVoid),
            retro_run: sym!("retro_run", sys::FnVoid),
            retro_reset: sym!("retro_reset", sys::FnVoid),
            retro_get_region: sym!("retro_get_region", sys::FnU32),
            retro_serialize_size: sym!("retro_serialize_size", sys::FnUsize),
            retro_serialize: sym!("retro_serialize", sys::FnSerialize),
            retro_unserialize: sym!("retro_unserialize", sys::FnUnserialize),
        };

        let found = unsafe { (api.retro_api_version)() };
        if found != RETRO_API_VERSION {
            return Err(CoreError::ApiMismatch {
                found,
                expected: RETRO_API_VERSION,
            });
        }

        let mut info: retro_system_info = unsafe { std::mem::zeroed() };
        unsafe { (api.retro_get_system_info)(&mut info) };
        let system_name = cstr_to_string(info.library_name);
        let system_version = cstr_to_string(info.library_version);
        let valid_extensions = cstr_to_string(info.valid_extensions)
            .split('|')
            .filter(|s| !s.is_empty())
            .map(|s| s.to_ascii_lowercase())
            .collect();

        let state = Box::new(CallbackState {
            pixel_format: PixelFormat::Rgb565,
            system_directory: CString::new(".").unwrap(),
            save_directory: CString::new(".").unwrap(),
            input: [[false; 16]; MAX_PORTS],
            frame: None,
            frame_duped: false,
            audio: Vec::with_capacity(4096),
            variables: Vec::new(),
            variables_dirty: false,
            av_info: AvInfo {
                base_width: 256,
                base_height: 224,
                max_width: 512,
                max_height: 512,
                aspect_ratio: 0.0,
                fps: 60.0,
                sample_rate: 32040.0,
            },
            av_info_dirty: false,
        });

        Ok(Core {
            api,
            state,
            system_name,
            system_version,
            needs_fullpath: info.need_fullpath,
            valid_extensions,
            loaded: false,
            lib,
        })
    }

    pub fn system_name(&self) -> &str {
        &self.system_name
    }
    pub fn system_version(&self) -> &str {
        &self.system_version
    }
    pub fn valid_extensions(&self) -> &[String] {
        &self.valid_extensions
    }

    /// Point the core at the folders it uses for BIOS / SRAM / configs.
    pub fn set_directories(&mut self, system: &Path, save: &Path) {
        self.state.system_directory =
            CString::new(system.to_string_lossy().into_owned()).unwrap_or_default();
        self.state.save_directory =
            CString::new(save.to_string_lossy().into_owned()).unwrap_or_default();
    }

    /// Override a core option by key (e.g. `snes9x_blargg` = `composite`).
    /// Upserts into the variable table and flags it dirty so the core re-reads
    /// it on the next `run()`. Call after `init()` — `retro_init` repopulates the
    /// table from the core's own defaults.
    pub fn set_variable(&mut self, key: &str, value: &str) {
        let (Ok(k), Ok(v)) = (CString::new(key), CString::new(value)) else {
            return;
        };
        if let Some(slot) = self.state.variables.iter_mut().find(|(ek, _)| *ek == k) {
            slot.1 = v;
        } else {
            self.state.variables.push((k, v));
        }
        self.state.variables_dirty = true;
    }

    /// Current value of a core option, if set.
    pub fn variable(&self, key: &str) -> Option<&str> {
        let k = CString::new(key).ok()?;
        self.state
            .variables
            .iter()
            .find(|(ek, _)| *ek == k)
            .and_then(|(_, v)| v.to_str().ok())
    }

    /// Register callbacks and call `retro_init`. Idempotent-unsafe: call once.
    pub fn init(&mut self) {
        self.enter(|api| unsafe {
            (api.retro_set_environment)(environment_cb);
            (api.retro_set_video_refresh)(video_refresh_cb);
            (api.retro_set_audio_sample)(audio_sample_cb);
            (api.retro_set_audio_sample_batch)(audio_sample_batch_cb);
            (api.retro_set_input_poll)(input_poll_cb);
            (api.retro_set_input_state)(input_state_cb);
            (api.retro_init)();
        });
    }

    /// Load a ROM. `data` is the ROM bytes; `path` is its on-disk location,
    /// required by cores whose `need_fullpath` is set.
    pub fn load_game(&mut self, path: &Path, data: &[u8]) -> Result<(), CoreError> {
        let c_path = CString::new(path.to_string_lossy().into_owned()).unwrap_or_default();
        let info = if self.needs_fullpath {
            retro_game_info {
                path: c_path.as_ptr(),
                data: ptr::null(),
                size: 0,
                meta: ptr::null(),
            }
        } else {
            retro_game_info {
                path: c_path.as_ptr(),
                data: data.as_ptr() as *const c_void,
                size: data.len(),
                meta: ptr::null(),
            }
        };

        let ok = self.enter(|api| unsafe { (api.retro_load_game)(&info) });
        if !ok {
            return Err(CoreError::LoadRejected);
        }
        self.loaded = true;

        // Default both ports to a standard pad.
        self.enter(|api| unsafe {
            (api.retro_set_controller_port_device)(0, RETRO_DEVICE_JOYPAD);
            (api.retro_set_controller_port_device)(1, RETRO_DEVICE_JOYPAD);
        });
        self.refresh_av_info();
        Ok(())
    }

    pub fn refresh_av_info(&mut self) {
        let mut av: retro_system_av_info = unsafe { std::mem::zeroed() };
        self.enter(|api| unsafe { (api.retro_get_system_av_info)(&mut av) });
        self.state.av_info = AvInfo {
            base_width: av.geometry.base_width,
            base_height: av.geometry.base_height,
            max_width: av.geometry.max_width.max(av.geometry.base_width),
            max_height: av.geometry.max_height.max(av.geometry.base_height),
            aspect_ratio: av.geometry.aspect_ratio,
            fps: av.timing.fps,
            sample_rate: av.timing.sample_rate,
        };
        self.state.av_info_dirty = false;
    }

    pub fn av_info(&self) -> AvInfo {
        self.state.av_info
    }

    pub fn av_info_dirty(&self) -> bool {
        self.state.av_info_dirty
    }

    pub fn region(&mut self) -> c_uint {
        self.enter(|api| unsafe { (api.retro_get_region)() })
    }

    /// Set one button on one port for the frames that follow.
    pub fn set_button(&mut self, port: usize, button: Button, pressed: bool) {
        if port < MAX_PORTS {
            self.state.input[port][button as usize] = pressed;
        }
    }

    /// Advance one frame. Drains previous video/audio first.
    pub fn run(&mut self) {
        self.state.frame_duped = false;
        self.state.audio.clear();
        self.enter(|api| unsafe { (api.retro_run)() });
        if self.state.av_info_dirty {
            self.refresh_av_info();
        }
    }

    /// The most recent frame, if the last `run()` produced a fresh one.
    pub fn take_frame(&mut self) -> Option<Frame> {
        self.state.frame.take()
    }

    pub fn frame_duped(&self) -> bool {
        self.state.frame_duped
    }

    /// Interleaved S16LE stereo produced by the last `run()`.
    pub fn audio(&self) -> &[i16] {
        &self.state.audio
    }

    // --- run-ahead scaffolding (used from Phase 1 on) ----------------------
    pub fn serialize_size(&mut self) -> usize {
        self.enter(|api| unsafe { (api.retro_serialize_size)() })
    }

    pub fn save_state(&mut self) -> Option<Vec<u8>> {
        let size = self.serialize_size();
        if size == 0 {
            return None;
        }
        let mut buf = vec![0u8; size];
        let ok = self
            .enter(|api| unsafe { (api.retro_serialize)(buf.as_mut_ptr() as *mut c_void, size) });
        ok.then_some(buf)
    }

    pub fn load_state(&mut self, buf: &[u8]) -> bool {
        self.enter(|api| unsafe {
            (api.retro_unserialize)(buf.as_ptr() as *const c_void, buf.len())
        })
    }

    pub fn reset(&mut self) {
        self.enter(|api| unsafe { (api.retro_reset)() });
    }

    /// Run `body` with the thread-local callback pointer set to our state.
    fn enter<R>(&mut self, body: impl FnOnce(&CoreApi) -> R) -> R {
        let ptr: *mut CallbackState = &mut *self.state;
        let prev = CB.with(|c| c.replace(ptr));
        let out = body(&self.api);
        CB.with(|c| c.set(prev));
        out
    }
}

impl Drop for Core {
    fn drop(&mut self) {
        if self.loaded {
            self.enter(|api| unsafe { (api.retro_unload_game)() });
        }
        self.enter(|api| unsafe { (api.retro_deinit)() });
    }
}

fn cstr_to_string(p: *const c_char) -> String {
    if p.is_null() {
        String::new()
    } else {
        unsafe { CStr::from_ptr(p) }.to_string_lossy().into_owned()
    }
}

// --- C callbacks -----------------------------------------------------------

unsafe extern "C" fn environment_cb(cmd: c_uint, data: *mut c_void) -> bool {
    match cmd {
        RETRO_ENVIRONMENT_GET_CAN_DUPE => {
            if !data.is_null() {
                *(data as *mut bool) = true;
            }
            true
        }
        RETRO_ENVIRONMENT_GET_OVERSCAN => {
            if !data.is_null() {
                *(data as *mut bool) = false;
            }
            true
        }
        RETRO_ENVIRONMENT_SET_SUPPORT_NO_GAME => true,
        RETRO_ENVIRONMENT_SET_PIXEL_FORMAT => {
            if data.is_null() {
                return false;
            }
            let fmt = *(data as *const c_uint);
            let mapped = match fmt {
                RETRO_PIXEL_FORMAT_0RGB1555 => Some(PixelFormat::Rgb1555),
                RETRO_PIXEL_FORMAT_XRGB8888 => Some(PixelFormat::Xrgb8888),
                RETRO_PIXEL_FORMAT_RGB565 => Some(PixelFormat::Rgb565),
                _ => None,
            };
            match mapped {
                Some(f) => {
                    with_cb(|s| s.pixel_format = f);
                    true
                }
                None => false,
            }
        }
        RETRO_ENVIRONMENT_GET_SYSTEM_DIRECTORY | RETRO_ENVIRONMENT_GET_CORE_ASSETS_DIRECTORY => {
            if data.is_null() {
                return false;
            }
            with_cb(|s| {
                *(data as *mut *const c_char) = s.system_directory.as_ptr();
            });
            true
        }
        RETRO_ENVIRONMENT_GET_SAVE_DIRECTORY => {
            if data.is_null() {
                return false;
            }
            with_cb(|s| {
                *(data as *mut *const c_char) = s.save_directory.as_ptr();
            });
            true
        }
        RETRO_ENVIRONMENT_GET_VARIABLE => {
            if data.is_null() {
                return false;
            }
            let var = &mut *(data as *mut retro_variable);
            let key = if var.key.is_null() {
                return false;
            } else {
                CStr::from_ptr(var.key)
            };
            with_cb(|s| {
                for (k, v) in &s.variables {
                    if k.as_c_str() == key {
                        var.value = v.as_ptr();
                        return true;
                    }
                }
                false
            })
            .unwrap_or(false)
        }
        RETRO_ENVIRONMENT_SET_VARIABLES => {
            if data.is_null() {
                return true;
            }
            let mut p = data as *const retro_variable;
            with_cb(|s| {
                s.variables.clear();
                while !(*p).key.is_null() {
                    let key = CStr::from_ptr((*p).key).to_owned();
                    // value looks like "Description; opt_a|opt_b|opt_c"
                    let raw = CStr::from_ptr((*p).value).to_string_lossy();
                    let default = raw
                        .split(';')
                        .nth(1)
                        .unwrap_or("")
                        .trim()
                        .split('|')
                        .next()
                        .unwrap_or("")
                        .trim()
                        .to_string();
                    if let Ok(v) = CString::new(default) {
                        s.variables.push((key, v));
                    }
                    p = p.add(1);
                }
                s.variables_dirty = true;
            });
            true
        }
        RETRO_ENVIRONMENT_GET_VARIABLE_UPDATE => {
            with_cb(|s| {
                let was = s.variables_dirty;
                s.variables_dirty = false;
                if !data.is_null() {
                    *(data as *mut bool) = was;
                }
            });
            true
        }
        RETRO_ENVIRONMENT_GET_CORE_OPTIONS_VERSION => {
            // Report legacy: pushes the core down the SET_VARIABLES path we parse.
            if !data.is_null() {
                *(data as *mut c_uint) = 0;
            }
            true
        }
        RETRO_ENVIRONMENT_SET_SYSTEM_AV_INFO | RETRO_ENVIRONMENT_SET_GEOMETRY => {
            with_cb(|s| s.av_info_dirty = true);
            true
        }
        RETRO_ENVIRONMENT_GET_INPUT_BITMASKS => true,
        RETRO_ENVIRONMENT_SHUTDOWN => true,
        _ => false,
    }
}

#[repr(C)]
struct retro_variable {
    key: *const c_char,
    value: *const c_char,
}

unsafe extern "C" fn video_refresh_cb(
    data: *const c_void,
    width: c_uint,
    height: c_uint,
    pitch: usize,
) {
    if data.is_null() {
        // Duped frame: core is telling us to repeat the previous one.
        with_cb(|s| s.frame_duped = true);
        return;
    }
    with_cb(|s| {
        let bpp = s.pixel_format.bytes_per_pixel();
        let row_bytes = width as usize * bpp;
        let mut pixels = Vec::with_capacity(row_bytes * height as usize);
        let src = data as *const u8;
        for row in 0..height as usize {
            let start = row * pitch;
            let line = std::slice::from_raw_parts(src.add(start), row_bytes);
            pixels.extend_from_slice(line);
        }
        s.frame = Some(Frame {
            width,
            height,
            pitch: row_bytes,
            format: s.pixel_format,
            pixels,
        });
    });
}

unsafe extern "C" fn audio_sample_cb(left: i16, right: i16) {
    with_cb(|s| {
        s.audio.push(left);
        s.audio.push(right);
    });
}

unsafe extern "C" fn audio_sample_batch_cb(data: *const i16, frames: usize) -> usize {
    if !data.is_null() {
        let slice = std::slice::from_raw_parts(data, frames * 2);
        with_cb(|s| s.audio.extend_from_slice(slice));
    }
    frames
}

unsafe extern "C" fn input_poll_cb() {
    // Frontend refreshes its matrix before run(); nothing to do here.
}

unsafe extern "C" fn input_state_cb(
    port: c_uint,
    device: c_uint,
    _index: c_uint,
    id: c_uint,
) -> i16 {
    if device != RETRO_DEVICE_JOYPAD {
        return 0;
    }
    let port = port as usize;
    with_cb(|s| {
        if port >= MAX_PORTS {
            return 0;
        }
        if id == RETRO_DEVICE_ID_JOYPAD_MASK {
            let mut mask: i16 = 0;
            for (i, held) in s.input[port].iter().enumerate() {
                if *held {
                    mask |= 1 << i;
                }
            }
            mask
        } else if (id as usize) < 16 && s.input[port][id as usize] {
            1
        } else {
            0
        }
    })
    .unwrap_or(0)
}
