//! Window plus scaling modes.
//!
//! - `PixelPerfect`: integer nearest, letterboxed.
//! - `Bilinear`: linear-sampled and drawn through a barrel-distorted mesh so the
//!   picture bulges like a CRT tube (curved edges, corner cut-off, edge
//!   vignette). No scanlines. Pair it with the core's Blargg NTSC option for the
//!   composite/RF colour bleed.

use sdl3::pixels::{Color, FColor, PixelFormat as SdlFormat};
use sdl3::rect::Rect;
use sdl3::render::{FPoint, ScaleMode as SdlScaleMode, Texture, Vertex, WindowCanvas};
use sdl3::VideoSubsystem;

use crate::PlatformError;

/// CRT tube shape. `WARP` is how hard the edges bow (0 = flat); `VIGNETTE` is
/// how much the corners darken; `GRID` is the mesh resolution.
const CRT_WARP: f32 = 0.06;
const CRT_VIGNETTE: f32 = 0.22;
const CRT_GRID: usize = 32;

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
            ScaleMode::Bilinear => "bilinear + crt tube",
        }
    }
}

pub struct Video {
    canvas: WindowCanvas,
    /// Streaming texture at native core resolution.
    src: Option<SrcTexture>,
    /// Cached CRT mesh; rebuilt only when the game rect resizes.
    mesh: Option<CrtMesh>,
    fullscreen: bool,
}

struct SrcTexture {
    tex: Texture,
    w: u32,
    h: u32,
    format: PixelFormat,
}

struct CrtMesh {
    verts: Vec<Vertex>,
    indices: Vec<i32>,
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
            mesh: None,
            fullscreen: false,
        })
    }

    pub fn toggle_fullscreen(&mut self) {
        self.fullscreen = !self.fullscreen;
        let _ = self.canvas.window_mut().set_fullscreen(self.fullscreen);
    }

    /// Render one frame into an offscreen target and save it as a BMP. Works
    /// headless (a background window never composites on macOS), so the real
    /// output — CRT warp and all — can be eyeballed without watching the window.
    pub fn capture_bmp(
        &mut self,
        frame: &FrameRef,
        aspect_ratio: f32,
        mode: ScaleMode,
        path: &std::path::Path,
    ) -> Result<(), PlatformError> {
        self.ensure_src(frame.width, frame.height, frame.format);
        self.upload(frame);

        let (out_w, out_h) = self
            .canvas
            .output_size()
            .unwrap_or((frame.width, frame.height));
        let aspect = if aspect_ratio > 0.0 {
            aspect_ratio
        } else {
            4.0 / 3.0
        };
        let dst = fit_aspect(out_w, out_h, aspect);
        self.ensure_mesh(dst);
        self.set_src_filter(mode);

        let mut target = self
            .canvas
            .create_texture_target(SdlFormat::RGBA32, out_w, out_h)
            .map_err(|e| PlatformError::Sdl(e.to_string()))?;

        let src = self.src.take().unwrap();
        let mesh = self.mesh.take().unwrap();
        let mut saved: Result<(), PlatformError> = Ok(());
        let outcome = self.canvas.with_texture_canvas(&mut target, |c| {
            c.set_draw_color(Color::RGB(0, 0, 0));
            c.clear();
            draw(c, &src, &mesh, dst, mode);
            saved = c
                .read_pixels(None::<Rect>)
                .and_then(|s| s.save_bmp(path))
                .map_err(|e| PlatformError::Sdl(e.to_string()));
        });
        self.src = Some(src);
        self.mesh = Some(mesh);
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

    fn set_src_filter(&mut self, mode: ScaleMode) {
        let want = match mode {
            ScaleMode::PixelPerfect => SdlScaleMode::Nearest,
            ScaleMode::Bilinear => SdlScaleMode::Linear,
        };
        self.src.as_mut().unwrap().tex.set_scale_mode(want);
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

    fn ensure_mesh(&mut self, dst: Rect) {
        let (w, h) = (dst.width(), dst.height());
        if matches!(&self.mesh, Some(m) if m.w == w && m.h == h) {
            return;
        }
        self.mesh = Some(build_crt_mesh(dst));
    }

    /// Draw one frame. `aspect_ratio <= 0` means "use 4:3".
    pub fn present(&mut self, frame: &FrameRef, aspect_ratio: f32, mode: ScaleMode) {
        self.ensure_src(frame.width, frame.height, frame.format);
        self.upload(frame);

        let (out_w, out_h) = self
            .canvas
            .output_size()
            .unwrap_or((frame.width, frame.height));
        let aspect = if aspect_ratio > 0.0 {
            aspect_ratio
        } else {
            4.0 / 3.0
        };
        let dst = fit_aspect(out_w, out_h, aspect);
        self.ensure_mesh(dst);
        self.set_src_filter(mode);

        self.canvas.set_draw_color(Color::RGB(0, 0, 0));
        self.canvas.clear();

        let src = self.src.take().unwrap();
        let mesh = self.mesh.take().unwrap();
        draw(&mut self.canvas, &src, &mesh, dst, mode);
        self.src = Some(src);
        self.mesh = Some(mesh);

        self.canvas.present();
    }
}

/// Draw the game into the current render target. `dst` is the 4:3 fit rect.
fn draw<T: sdl3::render::RenderTarget>(
    canvas: &mut sdl3::render::Canvas<T>,
    src: &SrcTexture,
    mesh: &CrtMesh,
    dst: Rect,
    mode: ScaleMode,
) {
    match mode {
        ScaleMode::PixelPerfect => {
            let (ow, oh) = canvas.output_size().unwrap_or((dst.width(), dst.height()));
            let scale = ((ow / src.w).min(oh / src.h)).max(1);
            let d = centered(ow, oh, src.w * scale, src.h * scale);
            let _ = canvas.copy(&src.tex, None::<sdl3::render::FRect>, d);
        }
        ScaleMode::Bilinear => {
            let _ = canvas.render_geometry(&mesh.verts, Some(&src.tex), &mesh.indices[..]);
        }
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

/// Build a textured grid over `dst` whose vertex positions are barrel-distorted
/// (edges bow out, corners pull in) with an edge vignette baked into the vertex
/// colours. Texture coordinates stay a plain grid, so the picture — not just the
/// outline — curves.
fn build_crt_mesh(dst: Rect) -> CrtMesh {
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
                position: FPoint::new(px, py),
                color: FColor::RGBA(shade, shade, shade, 1.0),
                tex_coord: FPoint::new(u, v),
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
