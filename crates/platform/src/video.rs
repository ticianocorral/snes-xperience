//! Window plus scaling modes (plan §4.7, trimmed).
//!
//! Two modes: `PixelPerfect` (integer nearest, letterboxed) and `Bilinear`
//! (RetroArch-style linear stretch to 4:3). `Bilinear` also gets rounded
//! corners — a nod to CRT-tube geometry, without scanlines or barrel warp.

use sdl3::pixels::{Color, PixelFormat as SdlFormat};
use sdl3::rect::Rect;
use sdl3::render::{BlendMode, FRect, ScaleMode as SdlScaleMode, Texture, WindowCanvas};
use sdl3::VideoSubsystem;

use crate::PlatformError;

/// Corner radius as a fraction of the shorter side of the game rect. Tuned to
/// read as a CRT-tube corner without eating picture.
const CORNER_FRAC: f32 = 0.06;

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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScaleMode {
    PixelPerfect,
    Bilinear,
}

impl ScaleMode {
    pub fn next(self) -> Self {
        match self {
            ScaleMode::PixelPerfect => ScaleMode::Bilinear,
            ScaleMode::Bilinear => ScaleMode::PixelPerfect,
        }
    }
    pub fn label(self) -> &'static str {
        match self {
            ScaleMode::PixelPerfect => "pixel perfect",
            ScaleMode::Bilinear => "bilinear (rounded corners)",
        }
    }
}

pub struct Video {
    canvas: WindowCanvas,
    /// Streaming texture at native core resolution.
    src: Option<SrcTexture>,
    /// Black corner overlay for the bilinear mode, sized to the game rect.
    corners: Option<CornerMask>,
    fullscreen: bool,
}

struct SrcTexture {
    tex: Texture,
    w: u32,
    h: u32,
    format: PixelFormat,
}

struct CornerMask {
    tex: Texture,
    w: u32,
    h: u32,
}

impl Video {
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
        canvas.set_draw_color(Color::RGB(0, 0, 0));
        canvas.clear();
        canvas.present();
        Ok(Self {
            canvas,
            src: None,
            corners: None,
            fullscreen: false,
        })
    }

    pub fn toggle_fullscreen(&mut self) {
        self.fullscreen = !self.fullscreen;
        let _ = self.canvas.window_mut().set_fullscreen(self.fullscreen);
    }

    /// Read back the composited window and save it as a BMP. Used by the
    /// headless self-check so the real output (rounded corners included) can be
    /// eyeballed without watching the window.
    pub fn capture_bmp(&self, path: &std::path::Path) -> Result<(), PlatformError> {
        let surface = self
            .canvas
            .read_pixels(None::<Rect>)
            .map_err(|e| PlatformError::Sdl(e.to_string()))?;
        surface
            .save_bmp(path)
            .map_err(|e| PlatformError::Sdl(e.to_string()))?;
        Ok(())
    }

    fn ensure_src(&mut self, w: u32, h: u32, format: PixelFormat) {
        let stale = match &self.src {
            Some(s) => s.w != w || s.h != h || s.format != format,
            None => true,
        };
        if stale {
            let tex = self
                .canvas
                .create_texture_streaming(format.sdl(), w, h)
                .expect("create streaming texture");
            self.src = Some(SrcTexture { tex, w, h, format });
        }
    }

    /// (Re)build the rounded-corner overlay to match a `w`x`h` game rect.
    fn ensure_corners(&mut self, w: u32, h: u32) {
        if matches!(&self.corners, Some(c) if c.w == w && c.h == h) {
            return;
        }
        let radius = (w.min(h) as f32 * CORNER_FRAC).round().max(6.0);
        let pixels = corner_mask_rgba(w, h, radius);
        let mut tex = self
            .canvas
            .create_texture_streaming(SdlFormat::RGBA32, w, h)
            .expect("create corner mask texture");
        tex.update(None, &pixels, w as usize * 4)
            .expect("fill mask");
        tex.set_blend_mode(BlendMode::Blend);
        tex.set_scale_mode(SdlScaleMode::Linear);
        self.corners = Some(CornerMask { tex, w, h });
    }

    /// Draw one frame. `aspect_ratio <= 0` means "use 4:3".
    pub fn present(&mut self, frame: &FrameRef, aspect_ratio: f32, mode: ScaleMode) {
        self.ensure_src(frame.width, frame.height, frame.format);

        let expected_pitch = frame.width as usize * frame.format.bytes_per_pixel();
        {
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

        let (out_w, out_h) = self
            .canvas
            .output_size()
            .unwrap_or((frame.width, frame.height));
        let aspect = if aspect_ratio > 0.0 {
            aspect_ratio
        } else {
            4.0 / 3.0
        };

        // Nearest for pixel-perfect, linear for the bilinear stretch.
        {
            let want = match mode {
                ScaleMode::PixelPerfect => SdlScaleMode::Nearest,
                ScaleMode::Bilinear => SdlScaleMode::Linear,
            };
            self.src.as_mut().unwrap().tex.set_scale_mode(want);
        }

        self.canvas.set_draw_color(Color::RGB(0, 0, 0));
        self.canvas.clear();

        match mode {
            ScaleMode::PixelPerfect => {
                let scale = ((out_w / frame.width).min(out_h / frame.height)).max(1);
                let dst = centered(out_w, out_h, frame.width * scale, frame.height * scale);
                let src = self.src.as_ref().unwrap();
                let _ = self.canvas.copy(&src.tex, None::<FRect>, dst);
            }
            ScaleMode::Bilinear => {
                let dst = fit_aspect(out_w, out_h, aspect);
                self.ensure_corners(dst.width(), dst.height());
                let src = self.src.take().unwrap();
                let corners = self.corners.take().unwrap();
                let _ = self.canvas.copy(&src.tex, None::<FRect>, dst);
                let _ = self.canvas.copy(&corners.tex, None::<FRect>, dst);
                self.src = Some(src);
                self.corners = Some(corners);
            }
        }

        self.canvas.present();
    }
}

fn centered(out_w: u32, out_h: u32, w: u32, h: u32) -> Rect {
    let x = (out_w as i32 - w as i32) / 2;
    let y = (out_h as i32 - h as i32) / 2;
    Rect::new(x, y, w, h)
}

/// Largest `aspect`-shaped rect that fits in the output, centered. Fills the
/// height unless that would overflow the width, then fills the width.
fn fit_aspect(out_w: u32, out_h: u32, aspect: f32) -> Rect {
    let mut h = out_h;
    let mut w = (h as f32 * aspect).round() as u32;
    if w > out_w {
        w = out_w;
        h = (w as f32 / aspect).round() as u32;
    }
    centered(out_w, out_h, w, h)
}

/// An RGBA buffer that is transparent inside a rounded rectangle and opaque
/// black in the four corners outside it, with a 1px soft edge.
fn corner_mask_rgba(w: u32, h: u32, radius: f32) -> Vec<u8> {
    let (wf, hf) = (w as f32, h as f32);
    let r = radius.min(wf / 2.0).min(hf / 2.0);
    let mut buf = vec![0u8; w as usize * h as usize * 4];

    for y in 0..h {
        for x in 0..w {
            let px = x as f32 + 0.5;
            let py = y as f32 + 0.5;

            // Circle centre for whichever corner this pixel sits in.
            let cx = if px < r {
                r
            } else if px > wf - r {
                wf - r
            } else {
                px
            };
            let cy = if py < r {
                r
            } else if py > hf - r {
                hf - r
            } else {
                py
            };

            // Only the corner boxes can be outside the rounded rect.
            let in_corner_box = (px < r || px > wf - r) && (py < r || py > hf - r);
            let alpha = if in_corner_box {
                let d = ((px - cx).powi(2) + (py - cy).powi(2)).sqrt();
                (d - r + 0.5).clamp(0.0, 1.0)
            } else {
                0.0
            };

            let i = (y as usize * w as usize + x as usize) * 4;
            buf[i + 3] = (alpha * 255.0).round() as u8; // R,G,B stay 0 (black)
        }
    }
    buf
}
