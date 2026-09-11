//! The one window: a static dark cabinet with the screen recessed into it. Both
//! the running game and the selector draw into that screen area — the game as a
//! frame through a barrel-distorted CRT mesh, the selector as flat 2D (rects,
//! 8x8 bitmap text, letterboxed images). The cabinet furniture (the chamfer ring
//! from the window edge down to the glass) is redrawn every frame so nothing
//! ever recreates the window. NTSC colour bleed is applied upstream
//! (`xperience-ntsc`).

use std::collections::HashMap;
use std::time::Duration;

use sdl3::pixels::{Color, FColor, PixelFormat as SdlFormat};
use sdl3::rect::Rect;
use sdl3::render::{
    BlendMode, ClippingRect, ScaleMode as SdlScaleMode, Texture, Vertex, WindowCanvas,
};
use sdl3::VideoSubsystem;

use crate::PlatformError;

/// CRT tube shape. `WARP` is how hard the edges bow (0 = flat); `VIGNETTE` is
/// how much the corners darken; `GRID` is the mesh resolution.
const CRT_WARP: f32 = 0.06;
const CRT_VIGNETTE: f32 = 0.22;
const CRT_GRID: usize = 32;

/// Cabinet around the tube. The screen is inset from the window by these
/// fractions (a bit more at the bottom for the "chin"); everything outside is
/// the cabinet face, chamfered down to a near-black recess at the screen edge.
const BEZEL_SIDE: f32 = 0.070;
const BEZEL_TOP: f32 = 0.070;
const BEZEL_CHIN: f32 = 0.110;
/// Cabinet face — dark warm-grey plastic. Reads as a surface, still far darker
/// than a lit game screen (plan §3.2).
const CABINET: (u8, u8, u8) = (40, 37, 33);
/// The lip right against the glass, in shadow.
const RECESS: (u8, u8, u8) = (4, 4, 5);
/// Cartridge shell / rim — a touch warmer than the cabinet so it reads as its
/// own object sitting in the slot, not part of the cabinet face (plan §3.2).
const CART_SHELL: (u8, u8, u8) = (54, 46, 40);
const CART_RIM: (u8, u8, u8) = (96, 86, 72);
/// Reserved image-cache key for the current cartridge's label art. Distinct
/// from any `Screen::set_image` id a caller might use.
const CARTRIDGE_IMG: u64 = u64::MAX;
/// Reserved image-cache key for the side panel's logo art.
const PANEL_LOGO_IMG: u64 = u64::MAX - 1;
/// Reserved image-cache key for the panel's most-recent note thumbnail.
const PANEL_NOTE_IMG: u64 = u64::MAX - 2;

/// The side panel: a column of plain widgets beside the tube during play —
/// not warped, drawn straight on the window (plan §2's presentation order,
/// §3.2). A fixed fraction of the window, clamped so it neither disappears on
/// a small window nor swallows a huge one.
const PANEL_FRAC: f32 = 0.25;
const PANEL_MIN: u32 = 260;
const PANEL_MAX: u32 = 520;
const PANEL_BG: (u8, u8, u8) = (16, 15, 14);
const PANEL_TEXT: (u8, u8, u8) = (225, 220, 210);
const PANEL_DIM: (u8, u8, u8) = (140, 134, 124);

/// The set's own nameplate: a small wordmark printed into the chin, left of
/// the cartridge — a touch lighter than the cabinet plastic, like an embossed
/// badge rather than a lit label.
const BRAND: &str = "SNES Xperience";
const BRAND_TEXT: (u8, u8, u8) = (92, 86, 78);

/// 8x8 glyph cell, before scaling.
const GLYPH: u32 = 8;

/// Pixel layout of a core framebuffer. Mirrors `xperience_emulation::PixelFormat`
/// so the platform layer stays independent of the emulation crate.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PixelFormat {
    Rgb1555,
    Xrgb8888,
    Rgb565,
}

impl PixelFormat {
    fn sdl(self) -> SdlFormat {
        match self {
            PixelFormat::Rgb1555 => SdlFormat::XRGB1555,
            PixelFormat::Xrgb8888 => SdlFormat::XRGB8888,
            PixelFormat::Rgb565 => SdlFormat::RGB565,
        }
    }
    fn bytes_per_pixel(self) -> usize {
        match self {
            PixelFormat::Rgb1555 | PixelFormat::Rgb565 => 2,
            PixelFormat::Xrgb8888 => 4,
        }
    }
}

/// A borrowed core frame ready to upload.
pub struct FrameRef<'a> {
    pub width: u32,
    pub height: u32,
    pub pitch: usize,
    pub format: PixelFormat,
    pub pixels: &'a [u8],
}

pub struct Cabinet {
    canvas: WindowCanvas,
    /// Streaming texture at the incoming frame's resolution (game path).
    src: Option<SrcTexture>,
    /// Cached CRT mesh; rebuilt only when the game rect resizes.
    mesh: Option<CrtMesh>,
    /// Cached cabinet mesh; rebuilt only when the window or screen rect changes.
    bezel: Option<BezelMesh>,
    /// Render target the selector draws its flat 2D into, then composited
    /// through the tube like a game frame.
    screen_tex: Option<SizedTex>,
    /// Small streaming texture for the signal-off snow.
    noise_tex: Option<SizedTex>,
    noise: Vec<u8>,
    rng: u32,
    /// 128 glyphs laid out horizontally, white on transparent (2D path).
    font: Texture,
    images: HashMap<u64, ImgTex>,
    /// The current inner-screen rect (the tube opening).
    screen: Rect,
    /// The cartridge "inserted" in the slot (game path only). `None` = no game
    /// running right now, so nothing is drawn.
    cartridge: Option<CartridgeSlot>,
    /// The side panel content (game path only, plan §3.2). `None` = nothing
    /// drawn — the tube fills the whole window, as on the shelf.
    panel: Option<PanelInfo>,
    /// Elapsed time to show at the bottom of the panel; the caller updates
    /// this once a frame (`set_session_time`).
    session: Duration,
    fullscreen: bool,
}

/// What to draw in the cartridge slot: the label art if we have it, else just
/// the ROM's name.
struct CartridgeSlot {
    has_image: bool,
    name: String,
}

/// What to draw at the top of the side panel: the `wheel` logo if we have it,
/// else the ROM's title. `commands` is the button legend (plan §3.2, item 3):
/// `(label, key)` pairs, in display order. `cheats` is the interruptor list
/// (item 4, plan §4.4): `(description, on)` pairs, with `cheat_sel` marking
/// which one the cursor is on.
struct PanelInfo {
    has_logo: bool,
    title: String,
    commands: Vec<(String, String)>,
    cheats: Vec<(String, bool)>,
    cheat_sel: usize,
    /// Notebook block (item 5, plan §3.4): absent entirely when this is 0 —
    /// no "no notes" filler.
    note_count: usize,
    has_note_thumb: bool,
}

struct SrcTexture {
    tex: Texture,
    w: u32,
    h: u32,
    format: PixelFormat,
}

/// A plain RGBA texture kept at a known size.
struct SizedTex {
    tex: Texture,
    w: u32,
    h: u32,
}

struct CrtMesh {
    verts: Vec<Vertex>,
    indices: Vec<i32>,
    w: u32,
    h: u32,
}

/// The chamfered ring from the window edge to the recessed screen.
struct BezelMesh {
    verts: Vec<Vertex>,
    indices: Vec<i32>,
    /// Cache key: (window w, window h, screen w, screen h).
    key: (u32, u32, u32, u32),
}

struct ImgTex {
    tex: Texture,
    w: u32,
    h: u32,
}

impl Cabinet {
    pub(crate) fn new(
        video: &VideoSubsystem,
        title: &str,
        width: u32,
        height: u32,
    ) -> Result<Self, PlatformError> {
        let window = video
            .window(title, width, height)
            .position_centered()
            .resizable()
            .build()
            .map_err(|e| PlatformError::Sdl(e.to_string()))?;
        let mut canvas = window.into_canvas();
        canvas.set_blend_mode(BlendMode::Blend);
        canvas.set_draw_color(Color::RGB(RECESS.0, RECESS.1, RECESS.2));
        canvas.clear();
        canvas.present();

        let font = build_font_atlas(&mut canvas)?;
        let (w, h) = canvas.output_size().unwrap_or((width, height));
        Ok(Self {
            canvas,
            src: None,
            mesh: None,
            bezel: None,
            screen_tex: None,
            noise_tex: None,
            noise: Vec::new(),
            rng: 0x9E37_79B9,
            font,
            images: HashMap::new(),
            screen: screen_area(w, h),
            cartridge: None,
            panel: None,
            session: Duration::ZERO,
            fullscreen: false,
        })
    }

    pub fn toggle_fullscreen(&mut self) {
        self.fullscreen = !self.fullscreen;
        let _ = self.canvas.window_mut().set_fullscreen(self.fullscreen);
    }

    /// Show the cartridge in its slot on the cabinet during play: `label`
    /// (width, height, RGBA) is the scraped `texture` art if there is one,
    /// else the slot falls back to `name` in text. Call once per game; the
    /// cabinet keeps showing it until the next `set_cartridge` call.
    pub fn set_cartridge(&mut self, label: Option<(u32, u32, &[u8])>, name: &str) {
        let has_image = if let Some((w, h, rgba)) = label {
            self.set_image(CARTRIDGE_IMG, w, h, rgba);
            true
        } else {
            false
        };
        self.cartridge = Some(CartridgeSlot {
            has_image,
            name: name.to_string(),
        });
    }

    /// Empty the cartridge slot (plan §3.3: ejecting pulls it out — the slot
    /// stays visibly empty until the next `set_cartridge`).
    pub fn clear_cartridge(&mut self) {
        self.cartridge = None;
    }

    /// Show the side panel during play: `logo` (width, height, RGBA) is the
    /// scraped `wheel` art if there is one, else the panel falls back to
    /// `title` in text (plan §3.2, item 1). `commands` is the button legend
    /// (item 3) — `(label, key)` pairs, e.g. `("Reset", "Backspace")`. Call
    /// once per game.
    pub fn set_panel(
        &mut self,
        logo: Option<(u32, u32, &[u8])>,
        title: &str,
        commands: &[(String, String)],
    ) {
        let has_logo = if let Some((w, h, rgba)) = logo {
            self.set_image(PANEL_LOGO_IMG, w, h, rgba);
            true
        } else {
            false
        };
        self.panel = Some(PanelInfo {
            has_logo,
            title: title.to_string(),
            commands: commands.to_vec(),
            cheats: Vec::new(),
            cheat_sel: 0,
            note_count: 0,
            has_note_thumb: false,
        });
    }

    /// Update the panel's notebook block (plan §3.4, item 5): `count` pages
    /// captured so far for this ROM, `thumb` the most recent one (width,
    /// height, RGBA) if there is one. Call once at game start and again
    /// after every capture. A no-op before `set_panel`.
    pub fn set_notes(&mut self, count: usize, thumb: Option<(u32, u32, &[u8])>) {
        let has_thumb = if let Some((w, h, rgba)) = thumb {
            self.set_image(PANEL_NOTE_IMG, w, h, rgba);
            true
        } else {
            false
        };
        if let Some(panel) = &mut self.panel {
            panel.note_count = count;
            panel.has_note_thumb = has_thumb;
        }
    }

    /// Update the side panel's cheat list (plan §4.4): `cheats` is
    /// `(description, on)` pairs in the curated order, `selected` the index
    /// the cursor is currently on. Call once at game start and again on every
    /// navigate/toggle — the list is always tiny. A no-op before `set_panel`.
    pub fn set_cheats(&mut self, cheats: &[(String, bool)], selected: usize) {
        if let Some(panel) = &mut self.panel {
            panel.cheats = cheats.to_vec();
            panel.cheat_sel = selected;
        }
    }

    /// Update the session clock shown at the bottom of the panel. Call once a
    /// frame; it just stores the value for the next present/capture.
    pub fn set_session_time(&mut self, elapsed: Duration) {
        self.session = elapsed;
    }

    /// Size of the recessed screen area — what the selector lays itself out in.
    pub fn screen_size(&self) -> (u32, u32) {
        let (w, h) = self.canvas.output_size().unwrap_or((1280, 720));
        let s = screen_area(w, h);
        (s.width(), s.height())
    }

    // --- game path -------------------------------------------------------

    /// Draw one frame to the window. `aspect_ratio <= 0` means "use 4:3".
    pub fn present_frame(&mut self, frame: &FrameRef, aspect_ratio: f32) {
        self.ensure_src(frame.width, frame.height, frame.format);
        self.upload(frame);

        let (out_w, out_h) = self
            .canvas
            .output_size()
            .unwrap_or((frame.width, frame.height));
        let panel = panel_rect(out_w, out_h);
        let cab_w = out_w.saturating_sub(panel.width());
        let screen = screen_area(cab_w, out_h);
        self.screen = screen;
        let dst = fit_aspect_in(screen, resolve_aspect(aspect_ratio));
        self.ensure_mesh(dst);
        self.ensure_bezel(cab_w, out_h, dst);

        self.canvas
            .set_draw_color(Color::RGB(RECESS.0, RECESS.1, RECESS.2));
        self.canvas.clear();

        let src = self.src.take().unwrap();
        let mesh = self.mesh.take().unwrap();
        let bezel = self.bezel.take().unwrap();
        let _ = self
            .canvas
            .render_geometry(&mesh.verts, Some(&src.tex), &mesh.indices[..]);
        let _ = self
            .canvas
            .render_geometry(&bezel.verts, None, &bezel.indices[..]);
        self.src = Some(src);
        self.mesh = Some(mesh);
        self.bezel = Some(bezel);

        draw_brand(&mut self.canvas, &mut self.font, self.screen, out_h);
        if let (Some(cart), Some(rect)) = (
            &self.cartridge,
            cartridge_slot_rect(self.screen, cab_w, out_h),
        ) {
            draw_cartridge_slot(&mut self.canvas, &mut self.font, &self.images, cart, rect);
        }
        draw_panel(
            &mut self.canvas,
            &mut self.font,
            &self.images,
            self.panel.as_ref(),
            panel,
            self.session,
        );
        self.canvas.present();
    }

    /// Render one frame into an offscreen target and save it as a BMP. Works
    /// headless (a background window never composites on macOS), so the real
    /// output — cabinet, CRT warp and all — can be eyeballed without a window.
    pub fn capture_bmp(
        &mut self,
        frame: &FrameRef,
        aspect_ratio: f32,
        path: &std::path::Path,
    ) -> Result<(), PlatformError> {
        self.ensure_src(frame.width, frame.height, frame.format);
        self.upload(frame);

        let (out_w, out_h) = self
            .canvas
            .output_size()
            .unwrap_or((frame.width, frame.height));
        let panel = panel_rect(out_w, out_h);
        let cab_w = out_w.saturating_sub(panel.width());
        let screen = screen_area(cab_w, out_h);
        self.screen = screen;
        let dst = fit_aspect_in(screen, resolve_aspect(aspect_ratio));
        self.ensure_mesh(dst);
        self.ensure_bezel(cab_w, out_h, dst);

        let mut target = self
            .canvas
            .create_texture_target(SdlFormat::RGBA32, out_w, out_h)
            .map_err(|e| PlatformError::Sdl(e.to_string()))?;

        let src = self.src.take().unwrap();
        let mesh = self.mesh.take().unwrap();
        let bezel = self.bezel.take().unwrap();
        let cart_draw = self
            .cartridge
            .as_ref()
            .zip(cartridge_slot_rect(self.screen, cab_w, out_h));
        let panel_info = self.panel.as_ref();
        let session = self.session;
        let font = &mut self.font;
        let images = &self.images;
        let mut saved: Result<(), PlatformError> = Ok(());
        let outcome = self.canvas.with_texture_canvas(&mut target, |c| {
            c.set_draw_color(Color::RGB(RECESS.0, RECESS.1, RECESS.2));
            c.clear();
            let _ = c.render_geometry(&mesh.verts, Some(&src.tex), &mesh.indices[..]);
            let _ = c.render_geometry(&bezel.verts, None, &bezel.indices[..]);
            draw_brand(c, font, screen, out_h);
            if let Some((cart, rect)) = cart_draw {
                draw_cartridge_slot(c, font, images, cart, rect);
            }
            draw_panel(c, font, images, panel_info, panel, session);
            saved = c
                .read_pixels(None::<Rect>)
                .and_then(|s| s.save_bmp(path))
                .map_err(|e| PlatformError::Sdl(e.to_string()));
        });
        self.src = Some(src);
        self.mesh = Some(mesh);
        self.bezel = Some(bezel);
        outcome.map_err(|e| PlatformError::Sdl(e.to_string()))?;
        saved
    }

    fn upload(&mut self, frame: &FrameRef) {
        let expected_pitch = frame.width as usize * frame.format.bytes_per_pixel();
        let src = self.src.as_mut().unwrap();
        let pitch = if frame.pitch == 0 {
            expected_pitch
        } else {
            frame.pitch
        };
        src.tex
            .update(None, frame.pixels, pitch)
            .expect("upload frame");
    }

    fn ensure_src(&mut self, w: u32, h: u32, format: PixelFormat) {
        let stale = match &self.src {
            Some(s) => s.w != w || s.h != h || s.format != format,
            None => true,
        };
        if stale {
            let mut tex = self
                .canvas
                .create_texture_streaming(format.sdl(), w, h)
                .expect("create streaming texture");
            tex.set_scale_mode(SdlScaleMode::Linear);
            self.src = Some(SrcTexture { tex, w, h, format });
        }
    }

    fn ensure_mesh(&mut self, dst: Rect) {
        let (w, h) = (dst.width(), dst.height());
        if matches!(&self.mesh, Some(m) if m.w == w && m.h == h) {
            return;
        }
        self.mesh = Some(build_crt_mesh(dst, 1.0));
    }

    fn ensure_bezel(&mut self, out_w: u32, out_h: u32, screen: Rect) {
        let key = (out_w, out_h, screen.width(), screen.height());
        if matches!(&self.bezel, Some(b) if b.key == key) {
            return;
        }
        self.bezel = Some(build_bezel_mesh(out_w, out_h, screen, key));
    }

    // --- 2D path (selector) --------------------------------------------
    //
    // The selector draws flat 2D into an offscreen buffer the size of the tube
    // opening, which is then composited through the CRT mesh just like a game
    // frame — so the shelf bulges with the same tube.

    /// Register/replace an image from tightly-packed RGBA8 (call between frames).
    pub fn set_image(&mut self, id: u64, w: u32, h: u32, rgba: &[u8]) {
        if w == 0 || h == 0 || rgba.len() < (w * h * 4) as usize {
            return;
        }
        let mut tex = match self.canvas.create_texture_static(SdlFormat::RGBA32, w, h) {
            Ok(t) => t,
            Err(e) => {
                log::warn!("cabinet: texture {w}x{h}: {e}");
                return;
            }
        };
        if tex.update(None, rgba, (w * 4) as usize).is_err() {
            return;
        }
        tex.set_blend_mode(BlendMode::Blend);
        tex.set_scale_mode(SdlScaleMode::Linear);
        self.images.insert(id, ImgTex { tex, w, h });
    }

    /// Draw a 2D frame: `draw` renders into a screen-sized buffer (coords
    /// 0..screen), which is then warped through the tube, framed and presented.
    pub fn frame_2d<F: FnOnce(&mut Screen)>(&mut self, bg: (u8, u8, u8), draw: F) {
        self.paint_2d(bg, draw);
        self.composite_screen();
        self.canvas.present();
    }

    /// Like [`Cabinet::frame_2d`] but composited into an offscreen target and
    /// saved as a BMP (headless — a background window never composites on macOS).
    pub fn capture_2d<F: FnOnce(&mut Screen)>(
        &mut self,
        bg: (u8, u8, u8),
        draw: F,
        path: &std::path::Path,
    ) -> Result<(), PlatformError> {
        self.paint_2d(bg, draw);

        let (ww, wh) = self.canvas.output_size().unwrap_or((1280, 720));
        let screen = self.screen;
        let mesh = build_crt_mesh(screen, 1.0);
        let mut target = self
            .canvas
            .create_texture_target(SdlFormat::RGBA32, ww, wh)
            .map_err(|e| PlatformError::Sdl(e.to_string()))?;
        let st = self.screen_tex.take().unwrap();
        let bezel = self.bezel.take().unwrap();
        let font = &mut self.font;
        let mut saved: Result<(), PlatformError> = Ok(());
        let outcome = self.canvas.with_texture_canvas(&mut target, |c| {
            c.set_draw_color(Color::RGB(RECESS.0, RECESS.1, RECESS.2));
            c.clear();
            let _ = c.render_geometry(&mesh.verts, Some(&st.tex), &mesh.indices[..]);
            let _ = c.render_geometry(&bezel.verts, None, &bezel.indices[..]);
            draw_brand(c, font, screen, wh);
            saved = c
                .read_pixels(None::<Rect>)
                .and_then(|s| s.save_bmp(path))
                .map_err(|e| PlatformError::Sdl(e.to_string()));
        });
        self.screen_tex = Some(st);
        self.bezel = Some(bezel);
        outcome.map_err(|e| PlatformError::Sdl(e.to_string()))?;
        saved
    }

    /// One frame of signal-off snow through the tube, cartridge still visible
    /// in its slot if one is set. `level` 1.0 = a full blizzard, 0.0 = a dim,
    /// near-still hiss. Never a full-screen flash.
    pub fn present_static(&mut self, level: f32) {
        let (ww, wh) = self.canvas.output_size().unwrap_or((1280, 720));
        let panel = panel_rect(ww, wh);
        let cab_w = ww.saturating_sub(panel.width());
        self.screen = screen_area(cab_w, wh);
        self.ensure_bezel(cab_w, wh, self.screen);
        self.update_noise_tex(level);

        let mesh = build_crt_mesh(self.screen, 1.0);
        self.canvas
            .set_draw_color(Color::RGB(RECESS.0, RECESS.1, RECESS.2));
        self.canvas.clear();
        let nt = self.noise_tex.take().unwrap();
        let _ = self
            .canvas
            .render_geometry(&mesh.verts, Some(&nt.tex), &mesh.indices[..]);
        self.noise_tex = Some(nt);
        let bezel = self.bezel.take().unwrap();
        let _ = self
            .canvas
            .render_geometry(&bezel.verts, None, &bezel.indices[..]);
        self.bezel = Some(bezel);
        draw_brand(&mut self.canvas, &mut self.font, self.screen, wh);
        if let (Some(cart), Some(rect)) =
            (&self.cartridge, cartridge_slot_rect(self.screen, cab_w, wh))
        {
            draw_cartridge_slot(&mut self.canvas, &mut self.font, &self.images, cart, rect);
        }
        draw_panel(
            &mut self.canvas,
            &mut self.font,
            &self.images,
            self.panel.as_ref(),
            panel,
            self.session,
        );
        self.canvas.present();
    }

    /// Like [`Cabinet::present_static`] but composited into an offscreen
    /// target and saved as a BMP (headless — eyeball the power-off / idle-off
    /// screen, cartridge and all, without a window).
    pub fn capture_static_bmp(
        &mut self,
        level: f32,
        path: &std::path::Path,
    ) -> Result<(), PlatformError> {
        let (ww, wh) = self.canvas.output_size().unwrap_or((1280, 720));
        let panel = panel_rect(ww, wh);
        let cab_w = ww.saturating_sub(panel.width());
        self.screen = screen_area(cab_w, wh);
        self.ensure_bezel(cab_w, wh, self.screen);
        self.update_noise_tex(level);
        let screen = self.screen;

        let mesh = build_crt_mesh(screen, 1.0);
        let mut target = self
            .canvas
            .create_texture_target(SdlFormat::RGBA32, ww, wh)
            .map_err(|e| PlatformError::Sdl(e.to_string()))?;
        let nt = self.noise_tex.take().unwrap();
        let bezel = self.bezel.take().unwrap();
        let cart_draw = self
            .cartridge
            .as_ref()
            .zip(cartridge_slot_rect(self.screen, cab_w, wh));
        let panel_info = self.panel.as_ref();
        let session = self.session;
        let font = &mut self.font;
        let images = &self.images;
        let mut saved: Result<(), PlatformError> = Ok(());
        let outcome = self.canvas.with_texture_canvas(&mut target, |c| {
            c.set_draw_color(Color::RGB(RECESS.0, RECESS.1, RECESS.2));
            c.clear();
            let _ = c.render_geometry(&mesh.verts, Some(&nt.tex), &mesh.indices[..]);
            let _ = c.render_geometry(&bezel.verts, None, &bezel.indices[..]);
            draw_brand(c, font, screen, wh);
            if let Some((cart, rect)) = cart_draw {
                draw_cartridge_slot(c, font, images, cart, rect);
            }
            draw_panel(c, font, images, panel_info, panel, session);
            saved = c
                .read_pixels(None::<Rect>)
                .and_then(|s| s.save_bmp(path))
                .map_err(|e| PlatformError::Sdl(e.to_string()));
        });
        self.noise_tex = Some(nt);
        self.bezel = Some(bezel);
        outcome.map_err(|e| PlatformError::Sdl(e.to_string()))?;
        saved
    }

    /// Like [`Cabinet::frame_2d`], but blended up from residual signal-off snow
    /// instead of cutting in cold: `static_level` is the snow still showing
    /// behind it, `shelf_alpha` (0..1) how much of the drawn frame shows on top
    /// (plan §3.3, "a estante entra por cima"). Call with `shelf_alpha` ramping
    /// 0.0 -> 1.0 over the first handful of frames after a game closes.
    pub fn frame_2d_fade_in<F: FnOnce(&mut Screen)>(
        &mut self,
        bg: (u8, u8, u8),
        draw: F,
        static_level: f32,
        shelf_alpha: f32,
    ) {
        self.paint_2d(bg, draw);
        self.update_noise_tex(static_level);
        let (_, out_h) = self.canvas.output_size().unwrap_or((1280, 720));

        let mesh_static = build_crt_mesh(self.screen, 1.0);
        let mesh_shelf = build_crt_mesh(self.screen, shelf_alpha.clamp(0.0, 1.0));

        self.canvas
            .set_draw_color(Color::RGB(RECESS.0, RECESS.1, RECESS.2));
        self.canvas.clear();

        let nt = self.noise_tex.take().unwrap();
        let _ = self.canvas.render_geometry(
            &mesh_static.verts,
            Some(&nt.tex),
            &mesh_static.indices[..],
        );
        self.noise_tex = Some(nt);

        let st = self.screen_tex.take().unwrap();
        let _ =
            self.canvas
                .render_geometry(&mesh_shelf.verts, Some(&st.tex), &mesh_shelf.indices[..]);
        self.screen_tex = Some(st);

        let bezel = self.bezel.take().unwrap();
        let _ = self
            .canvas
            .render_geometry(&bezel.verts, None, &bezel.indices[..]);
        self.bezel = Some(bezel);
        draw_brand(&mut self.canvas, &mut self.font, self.screen, out_h);

        self.canvas.present();
    }

    /// Fill the noise texture for the current window size at `level` (1.0 =
    /// full blizzard, 0.0 = a dim near-still hiss); leaves it in `noise_tex`.
    fn update_noise_tex(&mut self, level: f32) {
        const NW: u32 = 320;
        const NH: u32 = 240;
        self.ensure_noise_tex(NW, NH);
        let k = level.clamp(0.0, 1.0);
        let hi = (26.0 + 150.0 * k) as u32; // cap well under white
        let lo = (6.0 * k) as u32;
        let span = hi - lo + 1;
        {
            let Self { noise, rng, .. } = &mut *self;
            if noise.len() != (NW * NH * 4) as usize {
                *noise = vec![0u8; (NW * NH * 4) as usize];
            }
            for px in noise.as_chunks_mut::<4>().0 {
                *rng ^= *rng << 13;
                *rng ^= *rng >> 17;
                *rng ^= *rng << 5;
                let v = (lo + *rng % span) as u8;
                *px = [v, v, v, 255];
            }
        }
        let nt = self.noise_tex.as_mut().unwrap();
        let _ = nt.tex.update(None, &self.noise, (NW * 4) as usize);
    }

    /// Render `draw` into the screen buffer. Shared by `frame_2d` / `capture_2d`.
    fn paint_2d<F: FnOnce(&mut Screen)>(&mut self, bg: (u8, u8, u8), draw: F) {
        let (ww, wh) = self.canvas.output_size().unwrap_or((1280, 720));
        self.screen = screen_area(ww, wh);
        let (sw, sh) = (self.screen.width(), self.screen.height());
        self.ensure_screen_tex(sw, sh);
        self.ensure_bezel(ww, wh, self.screen);

        let Self {
            canvas,
            screen_tex,
            font,
            images,
            ..
        } = self;
        let images = &*images;
        let st = screen_tex.as_mut().unwrap();
        let _ = canvas.with_texture_canvas(&mut st.tex, |c| {
            c.set_draw_color(Color::RGB(bg.0, bg.1, bg.2));
            c.clear();
            let mut s = Screen {
                canvas: c,
                font,
                images,
                w: sw,
                h: sh,
            };
            draw(&mut s);
        });
        // The clip lives on the shared underlying renderer; clear it.
        self.canvas.set_clip_rect(ClippingRect::None);
    }

    /// Warp the screen buffer through the tube into the live window, then frame.
    fn composite_screen(&mut self) {
        let mesh = build_crt_mesh(self.screen, 1.0);
        self.canvas
            .set_draw_color(Color::RGB(RECESS.0, RECESS.1, RECESS.2));
        self.canvas.clear();
        let st = self.screen_tex.take().unwrap();
        let _ = self
            .canvas
            .render_geometry(&mesh.verts, Some(&st.tex), &mesh.indices[..]);
        self.screen_tex = Some(st);
        let bezel = self.bezel.take().unwrap();
        let _ = self
            .canvas
            .render_geometry(&bezel.verts, None, &bezel.indices[..]);
        self.bezel = Some(bezel);
        let (_, out_h) = self.canvas.output_size().unwrap_or((1280, 720));
        draw_brand(&mut self.canvas, &mut self.font, self.screen, out_h);
    }

    fn ensure_screen_tex(&mut self, w: u32, h: u32) {
        if matches!(&self.screen_tex, Some(s) if s.w == w && s.h == h) {
            return;
        }
        let mut tex = self
            .canvas
            .create_texture_target(SdlFormat::RGBA32, w, h)
            .expect("create screen target");
        tex.set_scale_mode(SdlScaleMode::Linear);
        // So `frame_2d_fade_in`'s vertex alpha can blend it over the snow.
        tex.set_blend_mode(BlendMode::Blend);
        self.screen_tex = Some(SizedTex { tex, w, h });
    }

    fn ensure_noise_tex(&mut self, w: u32, h: u32) {
        if matches!(&self.noise_tex, Some(s) if s.w == w && s.h == h) {
            return;
        }
        let mut tex = self
            .canvas
            .create_texture_streaming(SdlFormat::RGBA32, w, h)
            .expect("create noise texture");
        tex.set_scale_mode(SdlScaleMode::Linear);
        self.noise_tex = Some(SizedTex { tex, w, h });
    }
}

/// A 2D drawing surface, screen-local coordinates (0,0 = top-left of the tube
/// opening). Handed to the `frame_2d` / `capture_2d` closure.
pub struct Screen<'a> {
    canvas: &'a mut WindowCanvas,
    font: &'a mut Texture,
    images: &'a HashMap<u64, ImgTex>,
    w: u32,
    h: u32,
}

impl Screen<'_> {
    /// Size of the drawing surface.
    pub fn size(&self) -> (u32, u32) {
        (self.w, self.h)
    }

    pub fn fill(&mut self, x: i32, y: i32, w: u32, h: u32, c: (u8, u8, u8, u8)) {
        self.canvas.set_draw_color(Color::RGBA(c.0, c.1, c.2, c.3));
        let _ = self.canvas.fill_rect(Rect::new(x, y, w, h));
    }

    pub fn outline(&mut self, x: i32, y: i32, w: u32, h: u32, thick: u32, c: (u8, u8, u8, u8)) {
        let t = thick as i32;
        self.fill(x, y, w, thick, c);
        self.fill(x, y + h as i32 - t, w, thick, c);
        self.fill(x, y, thick, h, c);
        self.fill(x + w as i32 - t, y, thick, h, c);
    }

    /// Draw `s` at `(x, y)`, `scale`x the 8px cell. Returns the advance width.
    pub fn text(&mut self, x: i32, y: i32, scale: u32, c: (u8, u8, u8), s: &str) -> i32 {
        self.font.set_color_mod(c.0, c.1, c.2);
        let cell = (GLYPH * scale) as i32;
        let mut pen = x;
        for ch in s.chars() {
            let idx = if (ch as u32) < 128 {
                ch as u32
            } else {
                b'?' as u32
            };
            if ch != ' ' {
                let src = Rect::new(idx as i32 * GLYPH as i32, 0, GLYPH, GLYPH);
                let dst = Rect::new(pen, y, GLYPH * scale, GLYPH * scale);
                let _ = self.canvas.copy(self.font, src, dst);
            }
            pen += cell;
        }
        pen - x
    }

    /// Word-wrap `s` into `max_w`, returning the y past the last line.
    pub fn text_wrapped(
        &mut self,
        x: i32,
        y: i32,
        max_w: u32,
        scale: u32,
        c: (u8, u8, u8),
        s: &str,
    ) -> i32 {
        let cell = (GLYPH * scale) as i32;
        let cols = (max_w / (GLYPH * scale)).max(1) as usize;
        let mut line = String::new();
        let mut cy = y;
        for word in s.split_whitespace() {
            if !line.is_empty() && line.len() + 1 + word.len() > cols {
                self.text(x, cy, scale, c, &line);
                cy += cell + 2;
                line.clear();
            }
            if !line.is_empty() {
                line.push(' ');
            }
            line.push_str(word);
            while line.len() > cols {
                let (head, tail) = line.split_at(cols);
                self.text(x, cy, scale, c, head);
                cy += cell + 2;
                line = tail.to_string();
            }
        }
        if !line.is_empty() {
            self.text(x, cy, scale, c, &line);
            cy += cell + 2;
        }
        cy
    }

    /// Height [`Screen::text_wrapped`] would take for `s`, without drawing.
    pub fn wrapped_height(&self, max_w: u32, scale: u32, s: &str) -> i32 {
        wrapped_height(max_w, scale, s)
    }

    /// Clip drawing to `rect`; `None` clears the clip.
    pub fn clip(&mut self, rect: Option<(i32, i32, u32, u32)>) {
        self.canvas.set_clip_rect(match rect {
            Some((x, y, w, h)) => ClippingRect::Some(Rect::new(x, y, w, h)),
            None => ClippingRect::None,
        });
    }

    pub fn has_image(&self, id: u64) -> bool {
        self.images.contains_key(&id)
    }

    /// Draw image `id` letterboxed inside the box, centered. No-op if unknown.
    pub fn image_fit(&mut self, id: u64, x: i32, y: i32, bw: u32, bh: u32) {
        let Some(img) = self.images.get(&id) else {
            return;
        };
        let (iw, ih) = (img.w as f32, img.h as f32);
        let scale = (bw as f32 / iw).min(bh as f32 / ih);
        let dw = (iw * scale).round() as i32;
        let dh = (ih * scale).round() as i32;
        let dx = x + (bw as i32 - dw) / 2;
        let dy = y + (bh as i32 - dh) / 2;
        let _ = self.canvas.copy(
            &img.tex,
            None::<sdl3::render::FRect>,
            Rect::new(dx, dy, dw.max(1) as u32, dh.max(1) as u32),
        );
    }
}

fn resolve_aspect(a: f32) -> f32 {
    if a > 0.0 {
        a
    } else {
        4.0 / 3.0
    }
}

fn centered_in(area: Rect, w: u32, h: u32) -> Rect {
    let x = area.x() + (area.width() as i32 - w as i32) / 2;
    let y = area.y() + (area.height() as i32 - h as i32) / 2;
    Rect::new(x, y, w, h)
}

/// Largest `aspect`-shaped rect that fits inside `area`, centered in it. Fills
/// the height unless that would overflow the width, then fills the width.
fn fit_aspect_in(area: Rect, aspect: f32) -> Rect {
    let mut h = area.height();
    let mut w = (h as f32 * aspect).round() as u32;
    if w > area.width() {
        w = area.width();
        h = (w as f32 / aspect).round() as u32;
    }
    centered_in(area, w, h)
}

/// The cabinet opening: the window inset by the bezel fractions (a wider chin).
fn screen_area(out_w: u32, out_h: u32) -> Rect {
    let sx = (out_w as f32 * BEZEL_SIDE).round() as i32;
    let ty = (out_h as f32 * BEZEL_TOP).round() as i32;
    let by = (out_h as f32 * BEZEL_CHIN).round() as i32;
    let w = (out_w as i32 - 2 * sx).max(16) as u32;
    let h = (out_h as i32 - ty - by).max(16) as u32;
    Rect::new(sx, ty, w, h)
}

/// A four-quad ring from the window edge (cabinet colour) to the recessed
/// screen edge (near-black), so the screen sits in a shadowed well.
fn build_bezel_mesh(out_w: u32, out_h: u32, screen: Rect, key: (u32, u32, u32, u32)) -> BezelMesh {
    let norm = |c: (u8, u8, u8)| {
        FColor::RGBA(
            c.0 as f32 / 255.0,
            c.1 as f32 / 255.0,
            c.2 as f32 / 255.0,
            1.0,
        )
    };
    let (cab, rec) = (norm(CABINET), norm(RECESS));
    let z = sdl3::render::FPoint::new(0.0, 0.0);
    let vtx = |x: i32, y: i32, c: FColor| Vertex {
        position: sdl3::render::FPoint::new(x as f32, y as f32),
        color: c,
        tex_coord: z,
    };

    let (or, ob) = (out_w as i32, out_h as i32);
    let (il, it, ir, ib) = (screen.left(), screen.top(), screen.right(), screen.bottom());

    let verts = vec![
        vtx(0, 0, cab),
        vtx(or, 0, cab),
        vtx(or, ob, cab),
        vtx(0, ob, cab), // 0..3 outer
        vtx(il, it, rec),
        vtx(ir, it, rec),
        vtx(ir, ib, rec),
        vtx(il, ib, rec), // 4..7 inner
    ];
    #[rustfmt::skip]
    let indices = vec![
        0, 1, 5, 0, 5, 4, // top
        1, 2, 6, 1, 6, 5, // right
        2, 3, 7, 2, 7, 6, // bottom
        3, 0, 4, 3, 4, 7, // left
    ];
    BezelMesh {
        verts,
        indices,
        key,
    }
}

/// Where the cartridge slot sits: the cabinet's chin, right-aligned. `None` if
/// the window is too short for the chin to hold anything.
fn cartridge_slot_rect(screen: Rect, out_w: u32, out_h: u32) -> Option<Rect> {
    let chin_top = screen.bottom();
    let chin_h = out_h as i32 - chin_top;
    if chin_h < 24 {
        return None;
    }
    let h = ((chin_h as f32) * 0.62) as u32;
    let w = ((h as f32) * 1.35) as u32;
    let margin = 16i32;
    let x = out_w as i32 - margin - w as i32;
    let y = chin_top + (chin_h - h as i32) / 2;
    Some(Rect::new(x, y, w, h))
}

/// The set's nameplate, printed into the chin left of the tube — part of the
/// cabinet itself, so unlike the cartridge/panel it's drawn in every context
/// (shelf, game, idle-off) and never disappears. `None` if the chin is too
/// short to hold it (mirrors `cartridge_slot_rect`'s own guard).
fn draw_brand(canvas: &mut WindowCanvas, font: &mut Texture, screen: Rect, out_h: u32) {
    let chin_top = screen.bottom();
    let chin_h = out_h as i32 - chin_top;
    if chin_h < 24 {
        return;
    }
    let y = chin_top + (chin_h - GLYPH as i32) / 2;
    draw_text_absolute(
        canvas,
        font,
        screen.left(),
        y,
        TextStyle::new(1, BRAND_TEXT),
        BRAND,
        usize::MAX,
    );
}

/// Draw the cartridge slot — a small shell with the label art or, failing
/// that, the ROM's name. Cabinet furniture: not warped by the tube, drawn
/// straight on whatever `canvas` currently targets (live window or an
/// offscreen capture target — same call either way).
fn draw_cartridge_slot(
    canvas: &mut WindowCanvas,
    font: &mut Texture,
    images: &HashMap<u64, ImgTex>,
    cart: &CartridgeSlot,
    rect: Rect,
) {
    canvas.set_draw_color(Color::RGB(CART_RIM.0, CART_RIM.1, CART_RIM.2));
    let _ = canvas.fill_rect(Rect::new(
        rect.x() - 3,
        rect.y() - 3,
        rect.width() + 6,
        rect.height() + 6,
    ));
    canvas.set_draw_color(Color::RGB(CART_SHELL.0, CART_SHELL.1, CART_SHELL.2));
    let _ = canvas.fill_rect(rect);

    if cart.has_image {
        draw_image_absolute(
            canvas,
            images,
            CARTRIDGE_IMG,
            rect.x() + 4,
            rect.y() + 4,
            rect.width().saturating_sub(8),
            rect.height().saturating_sub(8),
        );
    } else {
        let max_chars = (rect.width().saturating_sub(10) / GLYPH).max(1) as usize;
        draw_text_absolute(
            canvas,
            font,
            rect.x() + 5,
            rect.y() + rect.height() as i32 / 2 - 4,
            TextStyle::new(1, (220, 210, 190)),
            &cart.name,
            max_chars,
        );
    }
}

/// Like `Screen::image_fit`, but at absolute window coordinates instead of
/// offset into the 2D screen buffer — for cabinet furniture like the cartridge.
fn draw_image_absolute(
    canvas: &mut WindowCanvas,
    images: &HashMap<u64, ImgTex>,
    id: u64,
    x: i32,
    y: i32,
    bw: u32,
    bh: u32,
) {
    let Some(img) = images.get(&id) else {
        return;
    };
    let (iw, ih) = (img.w as f32, img.h as f32);
    let scale = (bw as f32 / iw).min(bh as f32 / ih);
    let dw = (iw * scale).round() as i32;
    let dh = (ih * scale).round() as i32;
    let dx = x + (bw as i32 - dw) / 2;
    let dy = y + (bh as i32 - dh) / 2;
    let _ = canvas.copy(
        &img.tex,
        None::<sdl3::render::FRect>,
        Rect::new(dx, dy, dw.max(1) as u32, dh.max(1) as u32),
    );
}

/// Scale + colour for one of the absolute-coordinate text helpers below —
/// bundled so those functions stay under clippy's argument-count limit.
#[derive(Clone, Copy)]
struct TextStyle {
    scale: u32,
    color: (u8, u8, u8),
}

impl TextStyle {
    fn new(scale: u32, color: (u8, u8, u8)) -> Self {
        Self { scale, color }
    }
}

/// A single line of text at absolute window coordinates, truncated to
/// `max_chars` (`usize::MAX` for no truncation) — for cabinet furniture that
/// isn't inside a `Screen`.
fn draw_text_absolute(
    canvas: &mut WindowCanvas,
    font: &mut Texture,
    x: i32,
    y: i32,
    style: TextStyle,
    s: &str,
    max_chars: usize,
) {
    let (r, g, b) = style.color;
    font.set_color_mod(r, g, b);
    let cell = (GLYPH * style.scale) as i32;
    let mut pen = x;
    for ch in s.chars().take(max_chars) {
        let idx = if (ch as u32) < 128 {
            ch as u32
        } else {
            b'?' as u32
        };
        if ch != ' ' {
            let src = Rect::new(idx as i32 * GLYPH as i32, 0, GLYPH, GLYPH);
            let dst = Rect::new(pen, y, GLYPH * style.scale, GLYPH * style.scale);
            let _ = canvas.copy(font, src, dst);
        }
        pen += cell;
    }
}

/// Word-wrapped text at absolute window coordinates, mirroring
/// `Screen::text_wrapped`'s layout. Returns the y past the last line.
fn draw_text_wrapped_absolute(
    canvas: &mut WindowCanvas,
    font: &mut Texture,
    x: i32,
    y: i32,
    max_w: u32,
    style: TextStyle,
    s: &str,
) -> i32 {
    let cell = (GLYPH * style.scale) as i32;
    let cols = (max_w / (GLYPH * style.scale)).max(1) as usize;
    let mut line = String::new();
    let mut cy = y;
    for word in s.split_whitespace() {
        if !line.is_empty() && line.len() + 1 + word.len() > cols {
            draw_text_absolute(canvas, font, x, cy, style, &line, usize::MAX);
            cy += cell + 2;
            line.clear();
        }
        if !line.is_empty() {
            line.push(' ');
        }
        line.push_str(word);
        while line.len() > cols {
            let (head, tail) = line.split_at(cols);
            draw_text_absolute(canvas, font, x, cy, style, head, usize::MAX);
            cy += cell + 2;
            line = tail.to_string();
        }
    }
    if !line.is_empty() {
        draw_text_absolute(canvas, font, x, cy, style, &line, usize::MAX);
        cy += cell + 2;
    }
    cy
}

/// Where the side panel sits: a column on the right, `PANEL_FRAC` of the
/// window, clamped so it neither collapses nor swallows a small window.
fn panel_rect(out_w: u32, out_h: u32) -> Rect {
    let w = ((out_w as f32) * PANEL_FRAC)
        .round()
        .clamp(PANEL_MIN as f32, PANEL_MAX as f32) as u32;
    let w = w.min(out_w.saturating_sub(64));
    Rect::new((out_w - w) as i32, 0, w, out_h)
}

/// Draw the side panel: background, logo (or title) at top, session timer at
/// the bottom. Plain widgets, not warped by the tube (plan §2, §3.2). `None`
/// (no game running) draws nothing.
fn draw_panel(
    canvas: &mut WindowCanvas,
    font: &mut Texture,
    images: &HashMap<u64, ImgTex>,
    panel: Option<&PanelInfo>,
    rect: Rect,
    session: Duration,
) {
    let Some(panel) = panel else { return };
    if rect.width() == 0 {
        return;
    }
    canvas.set_draw_color(Color::RGB(PANEL_BG.0, PANEL_BG.1, PANEL_BG.2));
    let _ = canvas.fill_rect(rect);

    let pad = 20i32;
    let inner_w = rect.width().saturating_sub(pad as u32 * 2);
    let x = rect.x() + pad;
    let y = rect.y() + pad;

    // 1. Logo, or the title if there isn't one (plan §3.2, item 1).
    let mut cy = if panel.has_logo {
        draw_image_absolute(canvas, images, PANEL_LOGO_IMG, x, y, inner_w, 110);
        y + 110
    } else {
        draw_text_wrapped_absolute(
            canvas,
            font,
            x,
            y,
            inner_w,
            TextStyle::new(2, PANEL_TEXT),
            &panel.title,
        )
    };

    // 3. Commands — the console's own buttons, not the emulator's extras
    // (plan §3.2, item 3). Label left, key right, one line each.
    if !panel.commands.is_empty() {
        cy += 16;
        draw_text_absolute(
            canvas,
            font,
            x,
            cy,
            TextStyle::new(1, PANEL_DIM),
            "comandos",
            usize::MAX,
        );
        cy += GLYPH as i32 + 6;
        for (label, key) in &panel.commands {
            draw_text_absolute(
                canvas,
                font,
                x,
                cy,
                TextStyle::new(1, PANEL_TEXT),
                label,
                usize::MAX,
            );
            let key_w = (GLYPH as i32) * key.chars().count() as i32;
            draw_text_absolute(
                canvas,
                font,
                rect.right() - pad - key_w,
                cy,
                TextStyle::new(1, PANEL_DIM),
                key,
                usize::MAX,
            );
            cy += GLYPH as i32 + 4;
        }
    }

    // 4. Cheats — the interruptor list (plan §4.4), not the string of raw
    // addresses: selected row gets a cursor and full brightness, the rest
    // dim; on/off shown as `[x]`/`[ ]` since the font is ASCII-only.
    if !panel.cheats.is_empty() {
        cy += 16;
        draw_text_absolute(
            canvas,
            font,
            x,
            cy,
            TextStyle::new(1, PANEL_DIM),
            "cheats",
            usize::MAX,
        );
        cy += GLYPH as i32 + 6;
        for (i, (desc, on)) in panel.cheats.iter().enumerate() {
            let selected = i == panel.cheat_sel;
            let cursor = if selected { "> " } else { "  " };
            let mark = if *on { "[x] " } else { "[ ] " };
            let color = if selected { PANEL_TEXT } else { PANEL_DIM };
            cy = draw_text_wrapped_absolute(
                canvas,
                font,
                x,
                cy,
                inner_w,
                TextStyle::new(1, color),
                &format!("{cursor}{mark}{desc}"),
            );
        }
    }

    // 5. Notes — most-recent capture + counter (plan §3.2, item 5; §3.4).
    // Absent entirely with nothing captured yet, not a "no notes" filler.
    if panel.note_count > 0 {
        cy += 16;
        draw_text_absolute(
            canvas,
            font,
            x,
            cy,
            TextStyle::new(1, PANEL_DIM),
            "notas",
            usize::MAX,
        );
        cy += GLYPH as i32 + 6;
        if panel.has_note_thumb {
            draw_image_absolute(canvas, images, PANEL_NOTE_IMG, x, cy, inner_w, 70);
            cy += 70 + 6;
        }
        let label = format!(
            "{} captura{}",
            panel.note_count,
            if panel.note_count == 1 { "" } else { "s" }
        );
        draw_text_absolute(
            canvas,
            font,
            x,
            cy,
            TextStyle::new(1, PANEL_TEXT),
            &label,
            usize::MAX,
        );
    }

    // 6. Session clock, pinned to the bottom (plan §3.2, item 6).
    let secs = session.as_secs();
    let stamp = if secs >= 3600 {
        format!("{}:{:02}:{:02}", secs / 3600, (secs / 60) % 60, secs % 60)
    } else {
        format!("{:02}:{:02}", secs / 60, secs % 60)
    };
    let ty = rect.bottom() - pad - GLYPH as i32;
    draw_text_absolute(
        canvas,
        font,
        x,
        ty,
        TextStyle::new(1, PANEL_DIM),
        "session",
        usize::MAX,
    );
    let label_w = (GLYPH as i32) * "session ".len() as i32;
    draw_text_absolute(
        canvas,
        font,
        x + label_w,
        ty,
        TextStyle::new(1, PANEL_TEXT),
        &stamp,
        usize::MAX,
    );
}

/// Build a textured grid over `dst` whose vertex positions are barrel-distorted
/// (edges bow out, corners pull in) with an edge vignette baked into the vertex
/// colours. Texture coordinates stay a plain grid, so the picture — not just the
/// outline — curves. `alpha` (1.0 normally) lets a caller fade the whole tube in.
fn build_crt_mesh(dst: Rect, alpha: f32) -> CrtMesh {
    let n = CRT_GRID;
    let (ox, oy) = (dst.x() as f32, dst.y() as f32);
    let (dw, dh) = (dst.width() as f32, dst.height() as f32);

    let mut verts = Vec::with_capacity((n + 1) * (n + 1));
    for j in 0..=n {
        for i in 0..=n {
            let u = i as f32 / n as f32; // 0..1 texture / grid coord
            let v = j as f32 / n as f32;
            let cx = u * 2.0 - 1.0; // -1..1
            let cy = v * 2.0 - 1.0;

            // Barrel: corners move toward the centre, mid-edges stay put.
            let dx = cx * (1.0 - CRT_WARP * cy * cy);
            let dy = cy * (1.0 - CRT_WARP * cx * cx);

            let px = ox + (dx * 0.5 + 0.5) * dw;
            let py = oy + (dy * 0.5 + 0.5) * dh;

            let r2 = (cx * cx + cy * cy).min(2.0) / 2.0;
            let shade = (1.0 - CRT_VIGNETTE * r2 * r2).clamp(0.0, 1.0);

            verts.push(Vertex {
                position: sdl3::render::FPoint::new(px, py),
                color: FColor::RGBA(shade, shade, shade, alpha),
                tex_coord: sdl3::render::FPoint::new(u, v),
            });
        }
    }

    let stride = (n + 1) as i32;
    let mut indices = Vec::with_capacity(n * n * 6);
    for j in 0..n as i32 {
        for i in 0..n as i32 {
            let a = j * stride + i;
            let b = a + 1;
            let c = a + stride;
            let d = c + 1;
            indices.extend_from_slice(&[a, c, b, b, c, d]);
        }
    }

    CrtMesh {
        verts,
        indices,
        w: dst.width(),
        h: dst.height(),
    }
}

/// Line-count math shared by [`Cabinet::text_wrapped`]'s layout and
/// [`Cabinet::wrapped_height`]. Mirrors the wrap loop: greedy word packing into
/// `cols` chars, hard-splitting any word longer than a line.
fn wrapped_height(max_w: u32, scale: u32, s: &str) -> i32 {
    let cell = (GLYPH * scale) as i32;
    let cols = (max_w / (GLYPH * scale)).max(1) as usize;
    let mut lines = 0i32;
    let mut len = 0usize;
    let mut open = false;
    for word in s.split_whitespace() {
        if open && len + 1 + word.len() > cols {
            lines += 1;
            len = 0;
            open = false;
        }
        if open {
            len += 1;
        }
        len += word.len();
        open = true;
        while len > cols {
            lines += 1;
            len -= cols;
        }
    }
    if open {
        lines += 1;
    }
    lines * (cell + 2)
}

fn build_font_atlas(canvas: &mut WindowCanvas) -> Result<Texture, PlatformError> {
    let cols = 128u32;
    let w = cols * GLYPH;
    let h = GLYPH;
    let mut rgba = vec![0u8; (w * h * 4) as usize];
    for (i, glyph) in font8x8::legacy::BASIC_LEGACY.iter().enumerate() {
        for (row, bits) in glyph.iter().enumerate() {
            for col in 0..8u32 {
                if bits & (1 << col) != 0 {
                    let px = i as u32 * GLYPH + col;
                    let o = ((row as u32 * w + px) * 4) as usize;
                    rgba[o..o + 4].copy_from_slice(&[255, 255, 255, 255]);
                }
            }
        }
    }
    let mut tex = canvas
        .create_texture_static(SdlFormat::RGBA32, w, h)
        .map_err(|e| PlatformError::Sdl(e.to_string()))?;
    tex.update(None, &rgba, (w * 4) as usize)
        .map_err(|e| PlatformError::Sdl(e.to_string()))?;
    tex.set_blend_mode(BlendMode::Blend);
    tex.set_scale_mode(SdlScaleMode::Nearest);
    Ok(tex)
}

#[cfg(test)]
mod tests {
    use super::{fit_aspect_in, screen_area, wrapped_height, GLYPH};
    use sdl3::rect::Rect;

    #[test]
    fn screen_area_insets_with_a_wider_chin() {
        let s = screen_area(1000, 1000);
        assert_eq!(s.x(), 70); // 7% side
        assert_eq!(s.y(), 70); // 7% top
        assert_eq!(s.width(), 860); // 1000 - 2*70
        assert_eq!(s.height(), 820); // 1000 - 70 top - 110 chin
                                     // Never collapses to nothing on a tiny window.
        assert!(screen_area(4, 4).width() >= 16);
    }

    #[test]
    fn fit_aspect_in_centers_a_43_rect() {
        // A wide area: 4:3 fills the height, centered horizontally.
        let r = fit_aspect_in(Rect::new(0, 0, 800, 300), 4.0 / 3.0);
        assert_eq!(r.height(), 300);
        assert_eq!(r.width(), 400);
        assert_eq!(r.x(), 200);
        assert_eq!(r.y(), 0);
        // A tall area: 4:3 fills the width instead.
        let r = fit_aspect_in(Rect::new(0, 0, 400, 900), 4.0 / 3.0);
        assert_eq!(r.width(), 400);
        assert_eq!(r.height(), 300);
        assert_eq!(r.y(), 300);
    }

    #[test]
    fn wrapped_height_counts_lines() {
        let row = GLYPH as i32 + 2; // one line's advance at scale 1
        assert_eq!(wrapped_height(80, 1, ""), 0);
        // 80px / 8px = 10 cols. "hello world" -> "hello" + " world" = 11 > 10,
        // so two lines.
        assert_eq!(wrapped_height(80, 1, "hello world"), 2 * row);
        // Fits on one line.
        assert_eq!(wrapped_height(80, 1, "hello you"), row);
        // A single word longer than the line is hard-split.
        assert_eq!(wrapped_height(80, 1, &"x".repeat(25)), 3 * row);
    }
}
