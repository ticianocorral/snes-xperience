//! Window and the single presentation path: the frame is linear-sampled and
//! drawn through a barrel-distorted mesh so it bulges like a CRT tube (curved
//! edges, corners cut off, edge vignette). No scanlines. The NTSC colour bleed
//! is applied upstream (see `xperience-ntsc`). Around the tube sits a static
//! dark cabinet (Fase 3): the screen is recessed into it so the picture reads
//! as *inside* a TV, always the brightest thing in the window.

use sdl3::pixels::{Color, FColor, PixelFormat as SdlFormat};
use sdl3::rect::Rect;
use sdl3::render::{ScaleMode as SdlScaleMode, Texture, Vertex, WindowCanvas};
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

pub struct Video {
    canvas: WindowCanvas,
    /// Streaming texture at the incoming frame's resolution.
    src: Option<SrcTexture>,
    /// Cached CRT mesh; rebuilt only when the game rect resizes.
    mesh: Option<CrtMesh>,
    /// Cached cabinet mesh; rebuilt only when the window or screen rect changes.
    bezel: Option<BezelMesh>,
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

/// The chamfered ring from the window edge to the recessed screen.
struct BezelMesh {
    verts: Vec<Vertex>,
    indices: Vec<i32>,
    /// Cache key: (window w, window h, screen w, screen h).
    key: (u32, u32, u32, u32),
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
        canvas.set_draw_color(Color::RGB(RECESS.0, RECESS.1, RECESS.2));
        canvas.clear();
        canvas.present();
        Ok(Self {
            canvas,
            src: None,
            mesh: None,
            bezel: None,
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
        path: &std::path::Path,
    ) -> Result<(), PlatformError> {
        self.ensure_src(frame.width, frame.height, frame.format);
        self.upload(frame);

        let (out_w, out_h) = self
            .canvas
            .output_size()
            .unwrap_or((frame.width, frame.height));
        let screen = screen_area(out_w, out_h);
        let dst = fit_aspect_in(screen, resolve_aspect(aspect_ratio));
        self.ensure_mesh(dst);
        self.ensure_bezel(out_w, out_h, dst);

        let mut target = self
            .canvas
            .create_texture_target(SdlFormat::RGBA32, out_w, out_h)
            .map_err(|e| PlatformError::Sdl(e.to_string()))?;

        let src = self.src.take().unwrap();
        let mesh = self.mesh.take().unwrap();
        let bezel = self.bezel.take().unwrap();
        let mut saved: Result<(), PlatformError> = Ok(());
        let outcome = self.canvas.with_texture_canvas(&mut target, |c| {
            c.set_draw_color(Color::RGB(RECESS.0, RECESS.1, RECESS.2));
            c.clear();
            let _ = c.render_geometry(&bezel.verts, None, &bezel.indices[..]);
            let _ = c.render_geometry(&mesh.verts, Some(&src.tex), &mesh.indices[..]);
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
        self.mesh = Some(build_crt_mesh(dst));
    }

    fn ensure_bezel(&mut self, out_w: u32, out_h: u32, screen: Rect) {
        let key = (out_w, out_h, screen.width(), screen.height());
        if matches!(&self.bezel, Some(b) if b.key == key) {
            return;
        }
        self.bezel = Some(build_bezel_mesh(out_w, out_h, screen, key));
    }

    /// Draw one frame to the window. `aspect_ratio <= 0` means "use 4:3".
    pub fn present(&mut self, frame: &FrameRef, aspect_ratio: f32) {
        self.ensure_src(frame.width, frame.height, frame.format);
        self.upload(frame);

        let (out_w, out_h) = self
            .canvas
            .output_size()
            .unwrap_or((frame.width, frame.height));
        let screen = screen_area(out_w, out_h);
        let dst = fit_aspect_in(screen, resolve_aspect(aspect_ratio));
        self.ensure_mesh(dst);
        self.ensure_bezel(out_w, out_h, dst);

        self.canvas
            .set_draw_color(Color::RGB(RECESS.0, RECESS.1, RECESS.2));
        self.canvas.clear();

        let src = self.src.take().unwrap();
        let mesh = self.mesh.take().unwrap();
        let bezel = self.bezel.take().unwrap();
        let _ = self
            .canvas
            .render_geometry(&bezel.verts, None, &bezel.indices[..]);
        let _ = self
            .canvas
            .render_geometry(&mesh.verts, Some(&src.tex), &mesh.indices[..]);
        self.src = Some(src);
        self.mesh = Some(mesh);
        self.bezel = Some(bezel);

        self.canvas.present();
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
                position: sdl3::render::FPoint::new(px, py),
                color: FColor::RGBA(shade, shade, shade, 1.0),
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
