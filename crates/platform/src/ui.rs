//! A tiny immediate-mode 2D layer for the selector screen: filled/outlined
//! rects, 8x8 bitmap text (font8x8, no system font), and letterboxed images
//! uploaded from decoded RGBA. Nothing here knows about the emulator.

use std::collections::HashMap;

use sdl3::pixels::{Color, PixelFormat as SdlFormat};
use sdl3::rect::Rect;
use sdl3::render::{BlendMode, FRect, ScaleMode, Texture, WindowCanvas};
use sdl3::VideoSubsystem;

use crate::PlatformError;

/// 8x8 glyph cell, before scaling.
const GLYPH: u32 = 8;

pub struct Ui {
    canvas: WindowCanvas,
    /// 128 glyphs laid out horizontally, white on transparent.
    font: Texture,
    images: HashMap<u64, ImgTex>,
}

struct ImgTex {
    tex: Texture,
    w: u32,
    h: u32,
}

impl Ui {
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

        let font = build_font_atlas(&mut canvas)?;
        Ok(Self {
            canvas,
            font,
            images: HashMap::new(),
        })
    }

    pub fn size(&self) -> (u32, u32) {
        self.canvas.output_size().unwrap_or((1280, 720))
    }

    pub fn toggle_fullscreen(&mut self) {
        let on = !self.canvas.window().fullscreen_state().is_true();
        let _ = self.canvas.window_mut().set_fullscreen(on);
    }

    pub fn begin(&mut self, bg: (u8, u8, u8)) {
        self.canvas.set_draw_color(Color::RGB(bg.0, bg.1, bg.2));
        self.canvas.clear();
    }

    pub fn present(&mut self) {
        self.canvas.present();
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

    /// Draw `s` at `(x, y)`, `scale`× the 8px cell. Returns the advance width.
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
                let _ = self.canvas.copy(&self.font, src, dst);
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

    /// Height [`Ui::text_wrapped`] would take for `s` at `max_w` / `scale`,
    /// without drawing — for sizing a scroll region.
    pub fn wrapped_height(&self, max_w: u32, scale: u32, s: &str) -> i32 {
        wrapped_height(max_w, scale, s)
    }

    /// Clip subsequent drawing to `rect` (window pixels); `None` clears it.
    pub fn clip(&mut self, rect: Option<(i32, i32, u32, u32)>) {
        self.canvas
            .set_clip_rect(rect.map(|(x, y, w, h)| Rect::new(x, y, w, h)));
    }

    pub fn has_image(&self, id: u64) -> bool {
        self.images.contains_key(&id)
    }

    /// Register/replace an image from tightly-packed RGBA8.
    pub fn set_image(&mut self, id: u64, w: u32, h: u32, rgba: &[u8]) {
        if w == 0 || h == 0 || rgba.len() < (w * h * 4) as usize {
            return;
        }
        let mut tex = match self.canvas.create_texture_static(SdlFormat::RGBA32, w, h) {
            Ok(t) => t,
            Err(e) => {
                log::warn!("ui: texture {w}x{h}: {e}");
                return;
            }
        };
        if tex.update(None, rgba, (w * 4) as usize).is_err() {
            return;
        }
        tex.set_blend_mode(BlendMode::Blend);
        tex.set_scale_mode(ScaleMode::Linear);
        self.images.insert(id, ImgTex { tex, w, h });
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
            None::<FRect>,
            Rect::new(dx, dy, dw.max(1) as u32, dh.max(1) as u32),
        );
    }
}

/// Line-count math shared by [`Ui::text_wrapped`]'s layout and
/// [`Ui::wrapped_height`]. Mirrors the wrap loop: greedy word packing into
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
    tex.set_scale_mode(ScaleMode::Nearest);
    Ok(tex)
}

trait FullscreenBool {
    fn is_true(&self) -> bool;
}
impl FullscreenBool for sdl3::video::FullscreenType {
    fn is_true(&self) -> bool {
        !matches!(self, sdl3::video::FullscreenType::Off)
    }
}

#[cfg(test)]
mod tests {
    use super::{wrapped_height, GLYPH};

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
