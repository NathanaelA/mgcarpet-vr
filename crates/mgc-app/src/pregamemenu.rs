//! The pre-game game-selection main menu.
//!
//! Shown once at launch when the player passed neither `--level` nor
//! `--campaign`, BEFORE any campaign frontend or gameplay: it lets the
//! player pick which of the three games to play (Magic Carpet 1, its
//! Hidden Worlds expansion, or Magic Carpet 2) and whether to run in
//! Enhanced mode, then Start launches the chosen game's campaign
//! exactly as `--campaign <id>` would have.
//!
//! Unlike the retail frontends (which composite baked VGA art), this
//! screen is entirely self-contained: the three option images are
//! compiled into the binary (`menu/MC*.png`, decoded with the `png`
//! crate already in the tree) and the labels are drawn with a small
//! built-in 5x7 bitmap font, so the menu needs no baked assets to
//! appear. The whole 640x480 screen composes on the CPU into one RGBA
//! buffer and resolves to a single full-screen quad — the same shape
//! the MC1 frontend uses (`frontend_mc1::Mc1Menu::frame`).

use mgc_render::UiQuad;

use crate::campaign::CampaignId;
use crate::{get_baked_directory, IS_ANDROID};


/// Authored screen resolution (letterboxed into the real window). 4:3
/// to match the option art and the retail frontends.
pub const W: usize = 640;
pub const H: usize = 480;

/// The three option images, compiled in. Order is the on-screen order
/// and maps 1:1 onto [`GAMES`].
const IMAGE_BYTES: [&[u8]; 3] = [
    include_bytes!("../../../assets/pregame-menu/MC1.png"),
    include_bytes!("../../../assets/pregame-menu/MC1HW.png"),
    include_bytes!("../../../assets/pregame-menu/MC2.png"),
];

/// The campaign each option launches, and its caption.
const GAMES: [(CampaignId, &str); 3] = [
    (CampaignId::Mc1, "MAGIC CARPET"),
    (CampaignId::Mc1Hw, "HIDDEN WORLDS"),
    (CampaignId::Mc2, "MAGIC CARPET 2"),
];

/// One decoded option image (RGBA8, top-left origin).
struct Image {
    w: usize,
    h: usize,
    rgba: Vec<u8>,
}

/// One frontend action, drained by the app.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MenuAction {
    /// Launch the chosen game's campaign; `enhanced` picks the flight
    /// models (see the app's Start handler).
    Start { game: CampaignId, enhanced: bool },
    /// Leave the game without launching anything (Esc).
    Quit,
}

pub struct PreGameMenu {
    images: Vec<Image>,
    /// Which option is highlighted (index into [`GAMES`]). Defaults to
    /// 0 (Magic Carpet 1), per the issue.
    selected: usize,
    /// The Enhanced-mode switch.
    enhanced: bool,
    pending: Option<MenuAction>,
    has_mc1: bool,
    has_mc1hw: bool,
    has_mc2: bool,

}

// ---- layout (authored 640x480 space) ---------------------------------

/// The three option boxes: x, y, w, h.
const BOXES: [(f32, f32, f32, f32); 3] = [
    (32.0, 120.0, 176.0, 210.0),
    (232.0, 120.0, 176.0, 210.0),
    (432.0, 120.0, 176.0, 210.0),
];
/// The Enhanced-mode switch: the tick box, plus a wider hit rect that
/// also covers its label so a click on the text toggles it too.
const CHECK_BOX: (f32, f32, f32, f32) = (196.0, 356.0, 28.0, 28.0);
const CHECK_HIT: (f32, f32, f32, f32) = (196.0, 352.0, 260.0, 36.0);
/// The Start button.
const START_BTN: (f32, f32, f32, f32) = (260.0, 412.0, 120.0, 48.0);

impl PreGameMenu {
    /// Decode the compiled-in option art. `enhanced` seeds the switch
    /// from the resolved config so the menu reflects the current
    /// setting.
    pub fn new(enhanced: bool) -> Result<Self, String> {
        let mut images = Vec::with_capacity(3);
        for (i, bytes) in IMAGE_BYTES.iter().enumerate() {
            images.push(decode_png(bytes).map_err(|e| {
                format!("pregame-menu image {}: {e}", GAMES[i].1)
            })?);
        }
        let mc1_path = get_baked_directory().join("mc1/level-000.mgcl");
        let has_mc1 = std::fs::File::open(mc1_path.clone()).is_ok();
        let mc1hw_path = get_baked_directory().join("mc1hw/level-000.mgcl");
        let has_mc1hw = std::fs::File::open(mc1hw_path.clone()).is_ok();
        let mc2_path = get_baked_directory().join("mc2/level-000.mgcl");
        let has_mc2 = std::fs::File::open(mc2_path.clone()).is_ok();
        let mut selected = 0;
        if !has_mc1 {
            if has_mc1hw {
                selected = 1;
            } else if has_mc2 {
                selected = 2;
            }
        }

        Ok(Self {
            images,
            selected,
            enhanced,
            pending: None,
            has_mc1,
            has_mc1hw,
            has_mc2,
        })
    }

    pub fn tick(&mut self, _dt: f32) {}

    pub fn take_action(&mut self) -> Option<MenuAction> {
        self.pending.take()
    }

    /// Esc leaves the game (nothing has launched yet).
    #[allow(dead_code)]
    pub fn escape(&mut self) {
        self.pending = Some(MenuAction::Quit);
    }

    /// Left-click at a window position.
    pub fn click(&mut self, size: (f32, f32), cursor: (f32, f32)) {
        let (mx, my) = crate::ui::unletterbox(cursor, size, W as f32, H as f32);
        for (i, b) in BOXES.iter().enumerate() {
            if in_rect(mx, my, *b) {
                if (i == 0 && !self.has_mc1) || (i == 1 && !self.has_mc1hw) || (i == 2 && !self.has_mc2) {
                    // Do not allow selection of unavailable games.
                    return;
                }
                self.selected = i;
                return;
            }
        }
        if IS_ANDROID {
            if in_rect(mx, my, CHECK_HIT) {
                self.enhanced = !self.enhanced;
                return;
            }
        }
        if in_rect(mx, my, START_BTN) {
            if (self.selected == 0 && !self.has_mc1) || (self.selected == 1 && !self.has_mc1hw) || (self.selected == 2 && !self.has_mc2) {
                // Do not allow starting unavailable games.  This path can really only occur if NO game data is available -- but we still want to
                // handle it.
                return;
            }
            self.pending = Some(MenuAction::Start {
                game: GAMES[self.selected].0,
                enhanced: self.enhanced,
            });
        }
    }

    /// Compose this frame's screen and return it (an owned RGBA buffer
    /// the app uploads as the UI atlas) plus one full-screen
    /// letterboxed quad.
    pub fn frame(&mut self, size: (f32, f32), cursor: (f32, f32)) -> (Vec<u8>, Vec<UiQuad>) {
        let (scale, ox, oy) = crate::ui::letterbox(size, W as f32, H as f32);
        let (mx, my) = crate::ui::unletterbox(cursor, size, W as f32, H as f32);

        let mut buf = vec![0u8; W * H * 4];
        // Background: a flat dark slate.
        fill(&mut buf, 0.0, 0.0, W as f32, H as f32, [18, 20, 30, 255]);

        // Titles.
        let title = "MAGIC CARPET";
        draw_text_centered(&mut buf, W as f32 / 2.0, 30.0, title, 4, [235, 210, 120, 255]);
        draw_text_centered(&mut buf, W as f32 / 2.0, 82.0, "SELECT A GAME", 2, [180, 190, 210, 255]);

        // Option boxes.
        for (i, b) in BOXES.iter().enumerate() {
            let (bx, by, bw, bh) = *b;
            let hovered = in_rect(mx, my, *b);
            let sel = i == self.selected;
            // Box backing.
            let bg = if sel { [40, 46, 64, 255] } else { [26, 28, 40, 255] };
            fill(&mut buf, bx, by, bw, bh, bg);
            // The option image, fitted into the upper part of the box
            // (leaving a caption strip at the bottom), aspect-preserved.
            let img = &self.images[i];
            let pad = 8.0;
            let area_w = bw - 2.0 * pad;
            let area_h = bh - 2.0 * pad - 26.0;
            let s = (area_w / img.w as f32).min(area_h / img.h as f32);
            let dw = img.w as f32 * s;
            let dh = img.h as f32 * s;
            let dx = bx + (bw - dw) / 2.0;
            let dy = by + pad + (area_h - dh) / 2.0;
            blit_scaled(&mut buf, img, dx, dy, dw, dh);
            // Caption.
            let cap = GAMES[i].1;
            draw_text_centered(&mut buf, bx + bw / 2.0, by + bh - 20.0, cap, 2, [210, 214, 224, 255]);
            // Border: bright gold for the selection, a lighter edge on
            // hover, dim otherwise.
            let (edge, thick) = if sel {
                ([245, 205, 70, 255], 4.0)
            } else if hovered {
                ([150, 156, 174, 255], 2.0)
            } else {
                ([70, 74, 92, 255], 2.0)
            };
            border(&mut buf, bx, by, bw, bh, thick, edge);

            // Red X for games whose data is not present.
            let available = match i {
                0 => self.has_mc1,
                1 => self.has_mc1hw,
                2 => self.has_mc2,
                _ => true,
            };
            if !available {
                cross_out(&mut buf, *b, [220, 60, 60, 255]);
            }
        }

        if IS_ANDROID {
            // Enhanced-mode switch.
            let (cx, cy, cw, ch) = CHECK_BOX;
            let check_hover = in_rect(mx, my, CHECK_HIT);
            fill(&mut buf, cx, cy, cw, ch, [26, 28, 40, 255]);
            border(&mut buf, cx, cy, cw, ch, 2.0, if check_hover { [200, 206, 224, 255] } else { [120, 126, 144, 255] });
            if self.enhanced {
                fill(&mut buf, cx + 6.0, cy + 6.0, cw - 12.0, ch - 12.0, [90, 220, 120, 255]);
            }
            draw_text(&mut buf, cx + cw + 14.0, cy + 6.0, "ENHANCED MODE", 2, [220, 224, 234, 255]);
        }

        // Start button.
        let (sx, sy, sw, sh) = START_BTN;
        let start_hover = in_rect(mx, my, START_BTN);
        let btn = if start_hover { [70, 150, 90, 255] } else { [50, 120, 74, 255] };
        fill(&mut buf, sx, sy, sw, sh, btn);
        border(&mut buf, sx, sy, sw, sh, 2.0, [230, 236, 240, 255]);
        draw_text_centered(&mut buf, sx + sw / 2.0, sy + sh / 2.0 - 8.0, "START", 3, [245, 248, 250, 255]);

        let quads = vec![UiQuad {
            rect: [ox, oy, W as f32 * scale, H as f32 * scale],
            uv: [0.0, 0.0, W as f32, H as f32],
            tint: [1.0, 1.0, 1.0, 1.0],
        }];
        (buf, quads)
    }
}

// ---- geometry / compositing helpers -----------------------------------

fn in_rect(mx: f32, my: f32, r: (f32, f32, f32, f32)) -> bool {
    mx >= r.0 && mx < r.0 + r.2 && my >= r.1 && my < r.1 + r.3
}

/// Alpha-over one pixel.
fn put(buf: &mut [u8], x: usize, y: usize, c: [u8; 4]) {
    if x >= W || y >= H {
        return;
    }
    let o = (y * W + x) * 4;
    let a = c[3] as u32;
    if a == 0 {
        return;
    }
    if a == 255 {
        buf[o..o + 4].copy_from_slice(&c);
        return;
    }
    let ia = 255 - a;
    for k in 0..3 {
        buf[o + k] = ((c[k] as u32 * a + buf[o + k] as u32 * ia) / 255) as u8;
    }
    buf[o + 3] = 255;
}

fn fill(buf: &mut [u8], x: f32, y: f32, w: f32, h: f32, c: [u8; 4]) {
    let x0 = x.max(0.0) as usize;
    let y0 = y.max(0.0) as usize;
    let x1 = ((x + w).min(W as f32)).max(0.0) as usize;
    let y1 = ((y + h).min(H as f32)).max(0.0) as usize;
    for yy in y0..y1 {
        for xx in x0..x1 {
            put(buf, xx, yy, c);
        }
    }
}

/// A rectangle outline `t` pixels thick, inside the given rect.
fn border(buf: &mut [u8], x: f32, y: f32, w: f32, h: f32, t: f32, c: [u8; 4]) {
    fill(buf, x, y, w, t, c);
    fill(buf, x, y + h - t, w, t, c);
    fill(buf, x, y, t, h, c);
    fill(buf, x + w - t, y, t, h, c);
}

/// Draw a thick line from (x0,y0) to (x1,y1).
fn line(buf: &mut [u8], x0: f32, y0: f32, x1: f32, y1: f32, thick: f32, c: [u8; 4]) {
    let dx = x1 - x0;
    let dy = y1 - y0;
    let steps = dx.abs().max(dy.abs()).max(1.0) as usize;
    for i in 0..=steps {
        let t = i as f32 / steps as f32;
        let x = x0 + dx * t;
        let y = y0 + dy * t;
        fill(buf, x - thick / 2.0, y - thick / 2.0, thick, thick, c);
    }
}

/// Draw a red X across a rectangle to mark it unavailable.
fn cross_out(buf: &mut [u8], r: (f32, f32, f32, f32), c: [u8; 4]) {
    let (x, y, w, h) = r;
    let margin = 12.0;
    let x0 = x + margin;
    let y0 = y + margin;
    let x1 = x + w - margin;
    let y1 = y + h - margin;
    line(buf, x0, y0, x1, y1, 6.0, c);
    line(buf, x1, y0, x0, y1, 6.0, c);
}

/// Nearest-neighbour blit of an RGBA image, alpha-blended.
fn blit_scaled(buf: &mut [u8], img: &Image, dx: f32, dy: f32, dw: f32, dh: f32) {
    let dw_i = dw.round() as i32;
    let dh_i = dh.round() as i32;
    if dw_i <= 0 || dh_i <= 0 {
        return;
    }
    for j in 0..dh_i {
        let sy = (j as f32 / dh * img.h as f32) as usize;
        let sy = sy.min(img.h - 1);
        for i in 0..dw_i {
            let sx = (i as f32 / dw * img.w as f32) as usize;
            let sx = sx.min(img.w - 1);
            let so = (sy * img.w + sx) * 4;
            let c = [
                img.rgba[so],
                img.rgba[so + 1],
                img.rgba[so + 2],
                img.rgba[so + 3],
            ];
            put(buf, (dx as i32 + i) as usize, (dy as i32 + j) as usize, c);
        }
    }
}

// ---- text (built-in 5x7 uppercase font) --------------------------------

/// Glyph width/height in the base (unscaled) grid, plus inter-glyph gap.
const GW: usize = 5;
const GH: usize = 7;

fn draw_text(buf: &mut [u8], x: f32, y: f32, s: &str, scale: usize, c: [u8; 4]) {
    let mut cx = x as i32;
    let step = ((GW + 1) * scale) as i32;
    for ch in s.chars() {
        let g = glyph(ch);
        for (row, bits) in g.iter().enumerate() {
            for col in 0..GW {
                if bits & (1 << (GW - 1 - col)) != 0 {
                    let px = cx + (col * scale) as i32;
                    let py = y as i32 + (row * scale) as i32;
                    fill(buf, px as f32, py as f32, scale as f32, scale as f32, c);
                }
            }
        }
        cx += step;
    }
}

fn text_width(s: &str, scale: usize) -> f32 {
    if s.is_empty() {
        return 0.0;
    }
    ((s.chars().count() * (GW + 1) * scale) - scale) as f32
}

fn draw_text_centered(buf: &mut [u8], cx: f32, y: f32, s: &str, scale: usize, c: [u8; 4]) {
    draw_text(buf, cx - text_width(s, scale) / 2.0, y, s, scale, c);
}

/// 5x7 bitmap for an uppercase character (each row's low 5 bits, MSB =
/// leftmost pixel). Unknown characters render blank.
fn glyph(ch: char) -> [u8; GH] {
    match ch.to_ascii_uppercase() {
        'A' => [0x0E, 0x11, 0x11, 0x1F, 0x11, 0x11, 0x11],
        'B' => [0x1E, 0x11, 0x11, 0x1E, 0x11, 0x11, 0x1E],
        'C' => [0x0E, 0x11, 0x10, 0x10, 0x10, 0x11, 0x0E],
        'D' => [0x1E, 0x11, 0x11, 0x11, 0x11, 0x11, 0x1E],
        'E' => [0x1F, 0x10, 0x10, 0x1E, 0x10, 0x10, 0x1F],
        'F' => [0x1F, 0x10, 0x10, 0x1E, 0x10, 0x10, 0x10],
        'G' => [0x0E, 0x11, 0x10, 0x17, 0x11, 0x11, 0x0E],
        'H' => [0x11, 0x11, 0x11, 0x1F, 0x11, 0x11, 0x11],
        'I' => [0x0E, 0x04, 0x04, 0x04, 0x04, 0x04, 0x0E],
        'J' => [0x07, 0x02, 0x02, 0x02, 0x12, 0x12, 0x0C],
        'K' => [0x11, 0x12, 0x14, 0x18, 0x14, 0x12, 0x11],
        'L' => [0x10, 0x10, 0x10, 0x10, 0x10, 0x10, 0x1F],
        'M' => [0x11, 0x1B, 0x15, 0x15, 0x11, 0x11, 0x11],
        'N' => [0x11, 0x19, 0x15, 0x13, 0x11, 0x11, 0x11],
        'O' => [0x0E, 0x11, 0x11, 0x11, 0x11, 0x11, 0x0E],
        'P' => [0x1E, 0x11, 0x11, 0x1E, 0x10, 0x10, 0x10],
        'Q' => [0x0E, 0x11, 0x11, 0x11, 0x15, 0x12, 0x0D],
        'R' => [0x1E, 0x11, 0x11, 0x1E, 0x14, 0x12, 0x11],
        'S' => [0x0F, 0x10, 0x10, 0x0E, 0x01, 0x01, 0x1E],
        'T' => [0x1F, 0x04, 0x04, 0x04, 0x04, 0x04, 0x04],
        'U' => [0x11, 0x11, 0x11, 0x11, 0x11, 0x11, 0x0E],
        'V' => [0x11, 0x11, 0x11, 0x11, 0x11, 0x0A, 0x04],
        'W' => [0x11, 0x11, 0x11, 0x15, 0x15, 0x15, 0x0A],
        'X' => [0x11, 0x11, 0x0A, 0x04, 0x0A, 0x11, 0x11],
        'Y' => [0x11, 0x11, 0x0A, 0x04, 0x04, 0x04, 0x04],
        'Z' => [0x1F, 0x01, 0x02, 0x04, 0x08, 0x10, 0x1F],
        '0' => [0x0E, 0x11, 0x13, 0x15, 0x19, 0x11, 0x0E],
        '1' => [0x04, 0x0C, 0x04, 0x04, 0x04, 0x04, 0x0E],
        '2' => [0x0E, 0x11, 0x01, 0x06, 0x08, 0x10, 0x1F],
        '3' => [0x1F, 0x02, 0x04, 0x02, 0x01, 0x11, 0x0E],
        '4' => [0x02, 0x06, 0x0A, 0x12, 0x1F, 0x02, 0x02],
        '5' => [0x1F, 0x10, 0x1E, 0x01, 0x01, 0x11, 0x0E],
        '6' => [0x0E, 0x10, 0x10, 0x1E, 0x11, 0x11, 0x0E],
        '7' => [0x1F, 0x01, 0x02, 0x04, 0x08, 0x08, 0x08],
        '8' => [0x0E, 0x11, 0x11, 0x0E, 0x11, 0x11, 0x0E],
        '9' => [0x0E, 0x11, 0x11, 0x0F, 0x01, 0x01, 0x0E],
        _ => [0, 0, 0, 0, 0, 0, 0],
    }
}

// ---- PNG decode --------------------------------------------------------

/// Decode PNG bytes to an 8-bit RGBA image. Palette/grayscale/tRNS are
/// expanded by the decoder; 16-bit is reduced to 8.
fn decode_png(bytes: &[u8]) -> Result<Image, String> {
    let mut decoder = png::Decoder::new(bytes);
    decoder.set_transformations(png::Transformations::EXPAND | png::Transformations::STRIP_16);
    let mut reader = decoder.read_info().map_err(|e| e.to_string())?;
    let mut raw = vec![0u8; reader.output_buffer_size()];
    let info = reader.next_frame(&mut raw).map_err(|e| e.to_string())?;
    raw.truncate(info.buffer_size());
    let (w, h) = (info.width as usize, info.height as usize);
    let px = w * h;
    let mut rgba = vec![255u8; px * 4];
    match info.color_type {
        png::ColorType::Rgba => rgba.copy_from_slice(&raw),
        png::ColorType::Rgb => {
            for i in 0..px {
                rgba[i * 4..i * 4 + 3].copy_from_slice(&raw[i * 3..i * 3 + 3]);
                rgba[i * 4 + 3] = 255;
            }
        }
        png::ColorType::Grayscale => {
            for i in 0..px {
                let v = raw[i];
                rgba[i * 4] = v;
                rgba[i * 4 + 1] = v;
                rgba[i * 4 + 2] = v;
                rgba[i * 4 + 3] = 255;
            }
        }
        png::ColorType::GrayscaleAlpha => {
            for i in 0..px {
                let v = raw[i * 2];
                rgba[i * 4] = v;
                rgba[i * 4 + 1] = v;
                rgba[i * 4 + 2] = v;
                rgba[i * 4 + 3] = raw[i * 2 + 1];
            }
        }
        png::ColorType::Indexed => {
            return Err("indexed image not expanded (unexpected)".into());
        }
    }
    Ok(Image { w, h, rgba })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn images_decode() {
        let m = PreGameMenu::new(false).expect("compiled-in art decodes");
        assert_eq!(m.images.len(), 3);
        for img in &m.images {
            assert!(img.w > 0 && img.h > 0);
            assert_eq!(img.rgba.len(), img.w * img.h * 4);
        }
    }

    /// The default highlight is Magic Carpet 1, per the issue.
    #[test]
    fn default_selection_is_mc1() {
        let m = PreGameMenu::new(false).unwrap();
        assert_eq!(m.selected, 0);
        assert_eq!(GAMES[m.selected].0, CampaignId::Mc1);
    }

    /// Clicking an option highlights it; Start then reports that game.
    #[test]
    fn click_selects_and_start_launches() {
        let mut m = PreGameMenu::new(false).unwrap();
        m.has_mc2 = true; // pretend MC2 is available
        let size = (W as f32, H as f32);
        // The window maps 1:1 onto authored space here (no letterbox).
        let center = |r: (f32, f32, f32, f32)| (r.0 + r.2 / 2.0, r.1 + r.3 / 2.0);
        m.click(size, center(BOXES[2]));
        assert_eq!(m.selected, 2);
        m.click(size, center(START_BTN));
        assert_eq!(
            m.take_action(),
            Some(MenuAction::Start {
                game: CampaignId::Mc2,
                enhanced: false
            })
        );
    }

    /// The switch toggles and rides into the Start action.
    #[test]
    fn enhanced_switch_toggles() {
        let mut m = PreGameMenu::new(false).unwrap();
        m.has_mc1 = true; // pretend MC1 is available
        let size = (W as f32, H as f32);
        let center = |r: (f32, f32, f32, f32)| (r.0 + r.2 / 2.0, r.1 + r.3 / 2.0);
        if IS_ANDROID {
            m.click(size, center(CHECK_BOX));
            assert!(m.enhanced);
        }
        m.click(size, center(START_BTN));
        assert_eq!(
            m.take_action(),
            Some(MenuAction::Start {
                game: CampaignId::Mc1,
                enhanced: IS_ANDROID
            })
        );
    }
}
