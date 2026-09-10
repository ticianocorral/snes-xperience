//! Window plus the three scaling modes from section 4.7 of the plan.

use sdl3::pixels::{Color, PixelFormat as SdlFormat};
use sdl3::rect::Rect;
use sdl3::render::{FRect, ScaleMode as SdlScaleMode, Texture, WindowCanvas};
use sdl3::VideoSubsystem;

use crate::PlatformError;

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

/// Scaling modes. `PixelPerfect`, `SharpBilinear` and `Crt` are the plan's three
/// (§4.7); `Bilinear` is the plain GPU bilinear stretch, same as RetroArch's
/// "Bilinear Filtering" toggle. `Crt` has no shader yet (Phase 3) and renders
/// through the `SharpBilinear` path meanwhile.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScaleMode {
    PixelPerfect,
    Bilinear,
    SharpBilinear,
    Crt,
}

impl ScaleMode {
    pub fn next(self) -> Self {
        match self {
            ScaleMode::PixelPerfect => ScaleMode::Bilinear,
            ScaleMode::Bilinear => ScaleMode::SharpBilinear,
            ScaleMode::SharpBilinear => ScaleMode::Crt,
            ScaleMode::Crt => ScaleMode::PixelPerfect,
        }
    }
    pub fn label(self) -> &'static str {
        match self {
            ScaleMode::PixelPerfect => "pixel perfect",
            ScaleMode::Bilinear => "bilinear",
            ScaleMode::SharpBilinear => "sharp bilinear",
            ScaleMode::Crt => "crt (placeholder: sharp bilinear)",
        }
    }
}

pub struct Video {
    canvas: WindowCanvas,
    /// Streaming texture at native core resolution, nearest-sampled.
    src: Option<SrcTexture>,
    /// Integer-prescaled intermediate for the sharp-bilinear path.
    mid: Option<MidTexture>,
    fullscreen: bool,
}

struct SrcTexture {
    tex: Texture,
    w: u32,
    h: u32,
    format: PixelFormat,
}

struct MidTexture {
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
            mid: None,
            fullscreen: false,
        })
    }

    pub fn toggle_fullscreen(&mut self) {
        self.fullscreen = !self.fullscreen;
        let _ = self.canvas.window_mut().set_fullscreen(self.fullscreen);
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
            tex.set_scale_mode(SdlScaleMode::Nearest);
            self.src = Some(SrcTexture { tex, w, h, format });
        }
    }

    fn ensure_mid(&mut self, w: u32, h: u32) {
        let stale = match &self.mid {
            Some(m) => m.w != w || m.h != h,
            None => true,
        };
        if stale {
            let mut tex = self
                .canvas
                .create_texture_target(SdlFormat::XRGB8888, w, h)
                .expect("create target texture");
            tex.set_scale_mode(SdlScaleMode::Linear);
            self.mid = Some(MidTexture { tex, w, h });
        }
    }

    /// Draw one frame. `aspect_ratio <= 0` means "use 4:3".
    pub fn present(&mut self, frame: &FrameRef, aspect_ratio: f32, mode: ScaleMode) {
        self.ensure_src(frame.width, frame.height, frame.format);

        let expected_pitch = frame.width as usize * frame.format.bytes_per_pixel();
        {
            let src = self.src.as_mut().unwrap();
            // `Texture::update` wants tightly-packed rows if pitch matches; it
            // accepts an explicit pitch otherwise.
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

        // The source texture is sampled nearest for every mode except the plain
        // bilinear stretch.
        {
            let want = if mode == ScaleMode::Bilinear {
                SdlScaleMode::Linear
            } else {
                SdlScaleMode::Nearest
            };
            self.src.as_mut().unwrap().tex.set_scale_mode(want);
        }

        self.canvas.set_draw_color(Color::RGB(0, 0, 0));
        self.canvas.clear();

        match mode {
            ScaleMode::PixelPerfect => {
                let scale = ((out_w / frame.width).min(out_h / frame.height)).max(1);
                let dw = frame.width * scale;
                let dh = frame.height * scale;
                let dst = centered(out_w, out_h, dw, dh);
                let src = self.src.as_ref().unwrap();
                let _ = self.canvas.copy(&src.tex, None::<FRect>, dst);
            }
            ScaleMode::Bilinear => {
                // RetroArch-style: one linear stretch of the raw frame to a
                // 4:3 rect that fills the screen height.
                let dst = fit_aspect(out_w, out_h, aspect);
                let src = self.src.as_ref().unwrap();
                let _ = self.canvas.copy(&src.tex, None::<FRect>, dst);
            }
            ScaleMode::SharpBilinear | ScaleMode::Crt => {
                // Stage 1: nearest integer prescale into the intermediate.
                let k = (out_h / frame.height).clamp(1, 8);
                let mw = frame.width * k;
                let mh = frame.height * k;
                self.ensure_mid(mw, mh);

                // Borrow dance: take textures out, operate, put back.
                let src = self.src.take().unwrap();
                let mut mid = self.mid.take().unwrap();
                let _ = self.canvas.with_texture_canvas(&mut mid.tex, |c| {
                    c.set_draw_color(Color::RGB(0, 0, 0));
                    c.clear();
                    let _ = c.copy(&src.tex, None::<FRect>, None::<FRect>);
                });

                // Stage 2: linear blit to a 4:3-corrected rect that fills height.
                let dst = fit_aspect(out_w, out_h, aspect);
                let _ = self.canvas.copy(&mid.tex, None::<FRect>, dst);

                self.src = Some(src);
                self.mid = Some(mid);
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
