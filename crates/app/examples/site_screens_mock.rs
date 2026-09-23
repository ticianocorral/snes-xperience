//! MOCK — screens for the Retro Xperience site, 100% synthetic: a fake
//! homebrew game ("Mundo do Tomate") rendered inside the real cabinet, via
//! the same headless path as `ra_osd_mock`. Every pixel of game art is drawn
//! here from scratch — no ROM imagery anywhere on the site.
//!
//! Run: `cargo run -p xperience-app --example site_screens_mock` — writes
//! BMPs to /tmp/site-mocks/ (hero, inicial, ra, cart-NN frames). Convert to
//! site/assets/ with `sips -s format png` and assemble the GIF with ffmpeg.

use std::path::PathBuf;
use std::time::Duration;

use xperience_app::idle;
use xperience_platform::{FrameRef, PanelButton, PixelFormat, Platform};

const GAME_TITLE: &str = "Mundo do Tomate (homebrew)";

const LOGO_W: u32 = 480;
const LOGO_H: u32 = 120;
const LABEL_W: u32 = 200;
const LABEL_H: u32 = 300;

fn main() -> anyhow::Result<()> {
    let out = PathBuf::from("/tmp/site-mocks");
    std::fs::create_dir_all(&out)?;

    let plat = Platform::new().map_err(|e| anyhow::anyhow!(e.to_string()))?;
    let mut cab = plat
        .create_cabinet("SNES Xperience", 1280, 800, false)
        .map_err(|e| anyhow::anyhow!(e.to_string()))?;
    cab.set_nameplate("SNES Xperience v0.16.0\nsnes9x 1.63");

    let logo = logo_image();
    let label = label_image();

    // --- Tela inicial: slot vazio, tubo em estática (o painel idle real,
    //     com a "Estante de games" e o rodapé "Configurações"). Sem o
    //     prompt de core: nos screens do site o snes9x já está instalado. ---
    cab.clear_panel();
    idle::capture_preview(
        &mut cab,
        idle::RESTING_STATIC,
        true,
        true,
        &out.join("inicial.bmp"),
    )
    .map_err(|e| anyhow::anyhow!(e.to_string()))?;
    println!("wrote inicial.bmp");

    // --- A inserção do cartucho (o GIF do site): mesmo caminho da animação
    //     real — `set_cartridge_motion` sobre o tubo em estática. ---
    cab.set_panel(
        Some((LOGO_W, LOGO_H, &logo)),
        Some((LABEL_W, LABEL_H, &label)),
        GAME_TITLE,
        &commands(),
    );
    let steps = 14;
    for i in 0..steps {
        let t = i as f32 / (steps - 1) as f32;
        cab.set_cartridge_motion(Some((t, false)));
        cab.capture_static_bmp(idle::RESTING_STATIC, &out.join(format!("cart-{i:02}.bmp")))
            .map_err(|e| anyhow::anyhow!(e.to_string()))?;
    }
    cab.set_cartridge_motion(None);
    for i in 0..3 {
        cab.capture_static_bmp(
            idle::RESTING_STATIC,
            &out.join(format!("cart-{:02}.bmp", steps + i)),
        )
        .map_err(|e| anyhow::anyhow!(e.to_string()))?;
    }
    println!("wrote cart frames");

    // --- Hero: o jogo fake rodando no tubo, painel de verdade. ---
    cab.set_powered(true);
    cab.set_session_time(Duration::from_secs(1_493));
    let pixels = scene(512, 448);
    let frame = FrameRef {
        width: 512,
        height: 448,
        pitch: 512 * 4,
        format: PixelFormat::Xrgb8888,
        pixels: &pixels,
    };
    cab.capture_bmp(&frame, 4.0 / 3.0, &out.join("hero.bmp"))
        .map_err(|e| anyhow::anyhow!(e.to_string()))?;
    println!("wrote hero.bmp");

    // --- O badge fixo do RA (fila de OSD vazia): hardcore e softcore. ---
    cab.set_ra_status(Some(xperience_platform::RaStatus { hardcore: true }));
    cab.capture_bmp(&frame, 4.0 / 3.0, &out.join("ra-badge-hardcore.bmp"))
        .map_err(|e| anyhow::anyhow!(e.to_string()))?;
    cab.set_ra_status(Some(xperience_platform::RaStatus { hardcore: false }));
    cab.capture_bmp(&frame, 4.0 / 3.0, &out.join("ra-badge-softcore.bmp"))
        .map_err(|e| anyhow::anyhow!(e.to_string()))?;
    cab.set_ra_status(None);
    println!("wrote ra-badge shots");

    // --- Conquistas: notificação no queixo da TV, badge sintético. ---
    let badge = badge_image();
    cab.set_image(xperience_platform::DEMO_BADGE_IMG, 64, 64, &badge);
    cab.push_osd(
        &[
            "CONQUISTA DESBLOQUEADA",
            "Primeiro Tomate Colhido",
            "+10 pontos",
        ],
        Some(xperience_platform::DEMO_BADGE_IMG),
        Duration::from_secs(6),
    );
    cab.capture_bmp(&frame, 4.0 / 3.0, &out.join("ra.bmp"))
        .map_err(|e| anyhow::anyhow!(e.to_string()))?;
    println!("wrote ra.bmp");

    Ok(())
}

fn commands() -> Vec<(PanelButton, String)> {
    use PanelButton::*;
    vec![
        (Notebook, "Anotações".to_string()),
        (Cheats, "Cheats".to_string()),
        (PrintScreen, "Printscreen".to_string()),
        (SaveState, "Salvar".to_string()),
        (LoadState, "Carregar".to_string()),
    ]
}

// ---------------------------------------------------------------------
// O jogo fake: cenário no tubo (XRGB8888, como um frame do core).
// ---------------------------------------------------------------------

fn scene(w: u32, h: u32) -> Vec<u8> {
    let (w, h) = (w as i32, h as i32);
    let mut px = vec![0u8; (w * h * 4) as usize];

    // Céu.
    for y in 0..h {
        let t = y as f32 / h as f32;
        let c = (110.0 - 20.0 * t) as u8;
        for x in 0..w {
            put_xrgb(
                &mut px,
                w,
                h,
                x,
                y,
                (c, 160.0 as u8 + (40.0 * t) as u8, 250u8),
            );
        }
    }
    // Sol.
    disc(&mut px, w, h, 60, 54, 30, (255, 220, 90));
    disc(&mut px, w, h, 60, 54, 22, (255, 240, 160));
    // Nuvens.
    for (cx, cy, cw) in [(150, 70, 100), (330, 120, 130), (450, 60, 80)] {
        for dy in -12..=12 {
            for dx in -(cw / 2)..(cw / 2) {
                let inside =
                    (dx * dx) as f32 / ((cw * cw) / 4) as f32 + (dy * dy) as f32 / 144.0 < 1.0;
                if inside {
                    put_xrgb(&mut px, w, h, cx + dx, cy + dy, (252, 252, 250));
                }
            }
        }
    }
    // Duas colinas de morros.
    for x in 0..w {
        let y0 = 270 + ((x as f32 * 0.010).sin() * 42.0) as i32;
        for y in y0..h {
            put_xrgb(&mut px, w, h, x, y, (52, 128, 58));
        }
    }
    for x in 0..w {
        let y0 = 330 + ((x as f32 * 0.021 + 1.7).sin() * 30.0) as i32;
        for y in y0..h {
            put_xrgb(&mut px, w, h, x, y, (82, 178, 82));
        }
    }
    // Chão xadrez.
    for y in (h - 64)..h {
        for x in 0..w {
            let c = if ((x / 16) + (y / 16)) % 2 == 0 {
                (196, 110, 60)
            } else {
                (168, 90, 48)
            };
            put_xrgb(&mut px, w, h, x, y, c);
        }
    }
    // Blocos dourados.
    for bx in [60, 108, 156] {
        block(&mut px, w, h, bx, 160, 28, (240, 180, 60), (120, 70, 20));
    }
    // Tomates flutuantes (os "coletáveis").
    for (cx, cy) in [250, 350].into_iter().map(|x| (x, 210)) {
        disc(&mut px, w, h, cx, cy, 9, (215, 45, 45));
        disc(&mut px, w, h, cx, cy - 8, 3, (70, 150, 60));
    }
    // Mastro com bandeira no fim da fase.
    for y in (h - 64 - 110)..(h - 64) {
        put_xrgb(&mut px, w, h, 452, y, (180, 180, 190));
        put_xrgb(&mut px, w, h, 453, y, (140, 140, 150));
    }
    for dy in 0..22 {
        for dx in 0..26 - dy {
            put_xrgb(&mut px, w, h, 454 + dx, h - 64 - 110 + dy, (70, 160, 70));
        }
    }
    // Herói: o tomate.
    draw_sprite(
        &mut px,
        w,
        h,
        120,
        h - 64 - 33,
        3,
        &tomato(),
        &TOMATO_COLORS,
    );
    // HUD.
    hud_text(&mut px, w, 16, 12, "PONTOS 003200");
    hud_text(&mut px, w, w - 118, 12, "TOMATE 1");

    px
}

fn tomato() -> Vec<&'static str> {
    vec![
        "....gggg....",
        "...g....g...",
        ".rrrrrrrrrr.",
        "rrrrrrrrrrrr",
        "rrwwrrrrwwrr",
        "rrwerrrrwerr",
        "rrrrrrrrrrrr",
        "rrrrmmmmrrrr",
        ".rrrrrrrrrr.",
        "..nn....nn..",
    ]
}

const TOMATO_COLORS: &[(&str, (u8, u8, u8))] = &[
    ("r", (215, 45, 45)),
    ("g", (70, 150, 60)),
    ("w", (250, 250, 245)),
    ("e", (30, 30, 30)),
    ("m", (140, 25, 25)),
    ("n", (90, 55, 35)),
];

fn draw_sprite(
    px: &mut [u8],
    w: i32,
    h: i32,
    x0: i32,
    y0: i32,
    s: i32,
    rows: &[&'static str],
    colors: &[(&str, (u8, u8, u8))],
) {
    for (dy, row) in rows.iter().enumerate() {
        for (dx, ch) in row.chars().enumerate() {
            if ch == '.' {
                continue;
            }
            let Some((_, c)) = colors.iter().find(|(k, _)| k.chars().next() == Some(ch)) else {
                continue;
            };
            for yy in 0..s {
                for xx in 0..s {
                    put_xrgb(
                        px,
                        w,
                        h,
                        x0 + dx as i32 * s + xx,
                        y0 + dy as i32 * s + yy,
                        *c,
                    );
                }
            }
        }
    }
}

fn disc(px: &mut [u8], w: i32, h: i32, cx: i32, cy: i32, r: i32, c: (u8, u8, u8)) {
    for dy in -r..=r {
        for dx in -r..=r {
            if dx * dx + dy * dy <= r * r {
                put_xrgb(px, w, h, cx + dx, cy + dy, c);
            }
        }
    }
}

fn block(
    px: &mut [u8],
    w: i32,
    h: i32,
    x0: i32,
    y0: i32,
    s: i32,
    fill: (u8, u8, u8),
    edge: (u8, u8, u8),
) {
    for dy in 0..s {
        for dx in 0..s {
            let c = if dy < 2 || dx < 2 || dy > s - 3 || dx > s - 3 {
                edge
            } else {
                fill
            };
            put_xrgb(px, w, h, x0 + dx, y0 + dy, c);
        }
    }
}

fn hud_text(px: &mut [u8], w: i32, x0: i32, y0: i32, text: &str) {
    draw_text_xrgb(px, w, x0 + 1, y0 + 1, 2, text, (20, 20, 30));
    draw_text_xrgb(px, w, x0, y0, 2, text, (255, 255, 255));
}

// ---------------------------------------------------------------------
// Fonte de pixel 5×5, desenhada à mão — usada no HUD, no rótulo do
// cartucho, no logotipo do painel e no badge. Nada de fonte de sistema.
// ---------------------------------------------------------------------

fn glyph(ch: char) -> Option<[&'static str; 5]> {
    Some(match ch {
        'A' => [".###.", "#...#", "#####", "#...#", "#...#"],
        'B' => ["####.", "#...#", "####.", "#...#", "####."],
        'D' => ["####.", "#...#", "#...#", "#...#", "####."],
        'E' => ["#####", "#....", "###..", "#....", "#####"],
        'H' => ["#...#", "#...#", "#####", "#...#", "#...#"],
        'M' => ["#...#", "##.##", "#.#.#", "#...#", "#...#"],
        'N' => ["#...#", "##..#", "#.#.#", "#..##", "#...#"],
        'O' => [".###.", "#...#", "#...#", "#...#", ".###."],
        'P' => ["####.", "#...#", "####.", "#....", "#...."],
        'R' => ["####.", "#...#", "####.", "#..#.", "#...#"],
        'S' => [".####", "#....", ".###.", "....#", "####."],
        'T' => ["#####", "..#..", "..#..", "..#..", "..#.."],
        'U' => ["#...#", "#...#", "#...#", "#...#", ".###."],
        'W' => ["#...#", "#...#", "#.#.#", "##.##", "#...#"],
        '0' => [".###.", "#..##", "#.#.#", "##..#", ".###."],
        '1' => ["..#..", ".##..", "..#..", "..#..", "#####"],
        '2' => [".###.", "#...#", "..##.", ".#...", "#####"],
        '3' => ["####.", "....#", ".###.", "....#", "####."],
        '8' => [".###.", "#...#", ".###.", "#...#", ".###."],
        ' ' => [".....", ".....", ".....", ".....", "....."],
        _ => return None,
    })
}

fn draw_text_xrgb(px: &mut [u8], w: i32, x0: i32, y0: i32, s: i32, text: &str, c: (u8, u8, u8)) {
    let mut pen = x0;
    for ch in text.chars() {
        if let Some(g) = glyph(ch) {
            for (gy, row) in g.iter().enumerate() {
                for (gx, cell) in row.chars().enumerate() {
                    if cell != '#' {
                        continue;
                    }
                    for yy in 0..s {
                        for xx in 0..s {
                            put_xrgb(
                                px,
                                w,
                                i32::MAX,
                                pen + gx as i32 * s + xx,
                                y0 + gy as i32 * s + yy,
                                c,
                            );
                        }
                    }
                }
            }
        }
        pen += 6 * s;
    }
}

fn put_xrgb(px: &mut [u8], w: i32, _h: i32, x: i32, y: i32, c: (u8, u8, u8)) {
    let i = match (y * w + x).checked_mul(4) {
        Some(i) if ((i + 3) as usize) < px.len() => i as usize,
        _ => return,
    };
    px[i] = c.2;
    px[i + 1] = c.1;
    px[i + 2] = c.0; // XRGB8888, little-endian
}

// ---------------------------------------------------------------------
// Arte do cartucho/logotipo/badge em RGBA — imagens que o painel do
// gabinete recebe como se fossem as PNGs do usuário.
// ---------------------------------------------------------------------

fn put_rgba(px: &mut [u8], w: u32, h: u32, x: i32, y: i32, c: (u8, u8, u8, u8)) {
    if x < 0 || y < 0 || x >= w as i32 || y >= h as i32 {
        return;
    }
    let i = ((y as u32 * w + x as u32) * 4) as usize;
    if i + 3 < px.len() {
        px[i] = c.0;
        px[i + 1] = c.1;
        px[i + 2] = c.2;
        px[i + 3] = c.3;
    }
}

/// Logotipo do jogo no topo do painel: wordmark de pixel sobre fundo
/// transparente.
fn logo_image() -> Vec<u8> {
    let (w, h) = (LOGO_W as usize * 4, LOGO_H as usize);
    let mut px = vec![0u8; w * h as usize];
    let (w32, h32) = (LOGO_W, LOGO_H);
    let red = (200u8, 40u8, 40u8, 255u8);
    let green = (60u8, 140u8, 60u8, 255u8);
    // "MUNDO" grande…
    let s = 14;
    let x = (LOGO_W as i32 - 5 * 6 * s) / 2;
    draw_text_rgba(&mut px, w32, h32, x, 0, s, "MUNDO", red);
    // …e "DO TOMATE" embaixo.
    let s = 8;
    let x = (LOGO_W as i32 - 9 * 6 * s) / 2;
    draw_text_rgba(&mut px, w32, h32, x, 78, s, "DO TOMATE", green);
    // Folhinha sobre o primeiro O.
    for dy in 0..6 {
        for dx in 0..10 {
            put_rgba(&mut px, w32, h32, 30 + 4 * 6 * 14 + dx, dy + 2, green);
        }
    }
    px
}

/// Rótulo do cartucho: papel creme, tomate desenhado e o nome do jogo.
fn label_image() -> Vec<u8> {
    let (w, h) = (LABEL_W, LABEL_H);
    let mut px = vec![0u8; (w * h * 4) as usize];
    let cream = (245u8, 238u8, 220u8, 255u8);
    let red = (200u8, 40u8, 40u8, 255u8);
    let dark_red = (150u8, 25u8, 25u8, 255u8);
    let green = (60u8, 140u8, 60u8, 255u8);
    let ink = (60u8, 50u8, 40u8, 255u8);
    for y in 0..h as i32 {
        for x in 0..w as i32 {
            put_rgba(&mut px, w, h, x, y, cream);
        }
    }
    // Moldura.
    for b in 0..8 {
        for x in 0..w as i32 {
            put_rgba(&mut px, w, h, x, b, red);
            put_rgba(&mut px, w, h, x, h as i32 - 1 - b, red);
        }
        for y in 0..h as i32 {
            put_rgba(&mut px, w, h, b, y, red);
            put_rgba(&mut px, w, h, w as i32 - 1 - b, y, red);
        }
    }
    // O tomate.
    let (cx, cy) = (w as i32 / 2, 108);
    disc_rgba(&mut px, w, h, cx, cy, 52, dark_red);
    disc_rgba(&mut px, w, h, cx, cy, 46, red);
    disc_rgba(
        &mut px,
        w,
        h,
        cx - 16,
        cy - 18,
        10,
        (240u8, 120u8, 120u8, 255u8),
    );
    // Folha e cabinho.
    for dy in 0..10 {
        for dx in -22i32..22 {
            if dx.abs() + dy * 2 < 26 {
                put_rgba(&mut px, w, h, cx + dx, cy - 46 + dy, green);
            }
        }
    }
    for dy in 0..12 {
        for dx in 0..6 {
            put_rgba(
                &mut px,
                w,
                h,
                cx - 3 + dx,
                cy - 58 + dy,
                (90u8, 60u8, 35u8, 255u8),
            );
        }
    }
    // Nome do jogo.
    let s = 5;
    let x = (w as i32 - 5 * 6 * s) / 2;
    draw_text_rgba(&mut px, w, h, x, 190, s, "MUNDO", ink);
    let s = 4;
    let x = (w as i32 - 9 * 6 * s) / 2;
    draw_text_rgba(&mut px, w, h, x, 228, s, "DO TOMATE", ink);
    let s = 2;
    let x = (w as i32 - 14 * 6 * s) / 2;
    draw_text_rgba(&mut px, w, h, x, h as i32 - 34, s, "HOMEBREW 8-BIT", red);
    px
}

/// Badge 64×64 "desbloqueado": tomate em medalha — sintético de ponta a
/// ponta, no lugar do badge real do RetroAchievements.
fn badge_image() -> Vec<u8> {
    const S: u32 = 64;
    let mut px = vec![0u8; (S * S * 4) as usize];
    let gold = (230u8, 180u8, 60u8, 255u8);
    let navy = (35u8, 30u8, 60u8, 255u8);
    let red = (215u8, 45u8, 45u8, 255u8);
    let green = (70u8, 150u8, 60u8, 255u8);
    for y in 0..S as i32 {
        for x in 0..S as i32 {
            let (dx, dy) = (x - S as i32 / 2, y - S as i32 / 2);
            let d = (dx * dx + dy * dy) as f32;
            if d <= 31.0 * 31.0 {
                let c = if d >= 27.0 * 27.0 { gold } else { navy };
                put_rgba(&mut px, S, S, x, y, c);
            }
        }
    }
    disc_rgba(&mut px, S, S, 32, 36, 17, red);
    for dx in -10..10 {
        put_rgba(&mut px, S, S, 32 + dx, 17, green);
        put_rgba(&mut px, S, S, 32 + dx, 18, green);
    }
    disc_rgba(&mut px, S, S, 26, 32, 4, (245u8, 140u8, 140u8, 255u8));
    px
}

fn disc_rgba(px: &mut [u8], w: u32, h: u32, cx: i32, cy: i32, r: i32, c: (u8, u8, u8, u8)) {
    for dy in -r..=r {
        for dx in -r..=r {
            if dx * dx + dy * dy <= r * r {
                put_rgba(px, w, h, cx + dx, cy + dy, c);
            }
        }
    }
}

fn draw_text_rgba(
    px: &mut [u8],
    w: u32,
    h: u32,
    x0: i32,
    y0: i32,
    s: i32,
    text: &str,
    c: (u8, u8, u8, u8),
) {
    let mut pen = x0;
    for ch in text.chars() {
        if let Some(g) = glyph(ch) {
            for (gy, row) in g.iter().enumerate() {
                for (gx, cell) in row.chars().enumerate() {
                    if cell != '#' {
                        continue;
                    }
                    for yy in 0..s {
                        for xx in 0..s {
                            put_rgba(
                                px,
                                w,
                                h,
                                pen + gx as i32 * s + xx,
                                y0 + gy as i32 * s + yy,
                                c,
                            );
                        }
                    }
                }
            }
        }
        pen += 6 * s;
    }
}
