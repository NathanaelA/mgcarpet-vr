//! The MC1/HW main menu — the retail 320×200 frontend screen
//! (`sub_4BD60_4C0A0`, remc1 :58484; docs/traces/
//! mc1-campaign-save-menu.md "Main menu" recon). Shared by both
//! campaigns (one DATA/SCREENS set on the CD).
//!
//! Retail law reproduced:
//! - MMMASK.DAT is the hotspot map: the pixel VALUE under the cursor
//!   is the button id (mouse probed in 320×200 space).
//! - Highlight = palette-brighten of the hovered mask region: every
//!   pixel whose mask matches the hovered id is remapped through a
//!   ×1.30 brightness LUT (retail sub_51E84/sub_504A0) — no overlay
//!   sprites.
//! - GLOBE.DAT (30 frames) / TIMER.DAT (3 frames) delta-animate over
//!   the screen on alternate menu frames; the timer only runs while
//!   a game is underway.
//! - Save/load submode: the six mask regions 5-10 become slots 1-6,
//!   labels drawn into the parchment rects (`dword_4A7EC`); "--" =
//!   empty slot. Slot labels max 20 chars.
//!
//! The whole screen composes on the CPU exactly like retail's VGA
//! buffer (bg indices + movie deltas + sprite blits + font masks +
//! the brighten LUT), then resolves through the menu palette into
//! one RGBA quad.
//!
//! Button wiring (retail's Play button doubles as
//! new-game-when-idle/resume-when-active — here the two roles are
//! explicit): 1 = START NEW GAME (confirm), 11 = CONTINUE
//! (the next campaign level — the retail between-level beat),
//! 5/6 = LOAD/SAVE submodes, 2 = change the save name, 4 = quit
//! (confirm), 3 = multiplayer (not in this remake).

use std::path::Path;

use mgc_render::UiQuad;

/// One frontend action, drained by the app.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Mc1Action {
    /// Start over at campaign level 0 (confirmed).
    NewGame,
    /// Resume/launch the current campaign level.
    Continue,
    /// Write the campaign to a slot. Carries NO label: the slot row is
    /// DERIVED (player name + level + progress), so there is nothing
    /// for the player to author here — see [`Mc1Action::SetName`].
    SaveTo {
        slot: usize,
    },
    LoadFrom(usize),
    /// The edited save name (retail slot label, max 20).
    SetName(String),
    Quit,
}

const W: usize = 320;
const H: usize = 200;

/// The six slot-label rects (`dword_4A7EC_4AB2C`, :2079), 320-space.
const SLOT_RECTS: [(f32, f32, f32, f32); 6] = [
    (170.0, 0.0, 150.0, 36.0),
    (161.0, 37.0, 159.0, 35.0),
    (191.0, 73.0, 129.0, 32.0),
    (172.0, 104.0, 148.0, 27.0),
    (186.0, 133.0, 134.0, 31.0),
    (173.0, 165.0, 147.0, 28.0),
];

/// A baked menu movie: cropped frames at a fixed position, plus the
/// touched-pixel mask — retail steps only the FLIC deltas over the
/// live screen, so exactly the delta-touched pixels may be painted
/// (the crop's other pixels belong to the menu art beneath).
struct Movie {
    x: usize,
    y: usize,
    w: usize,
    h: usize,
    frames: Vec<Vec<u8>>,
    mask: Vec<u8>,
}

impl Movie {
    fn load(dir: &Path, bin: &str, mask_bin: &str, json: &str) -> Result<Self, String> {
        let meta: serde_json::Value = serde_json::from_slice(
            &std::fs::read(dir.join(json)).map_err(|e| format!("{json}: {e}"))?,
        )
        .map_err(|e| format!("{json}: {e}"))?;
        let get = |k: &str| meta[k].as_u64().ok_or(format!("{json}: missing {k}"));
        let (x, y, w, h, n) = (
            get("x")? as usize,
            get("y")? as usize,
            get("w")? as usize,
            get("h")? as usize,
            get("frames")? as usize,
        );
        let raw = std::fs::read(dir.join(bin)).map_err(|e| format!("{bin}: {e}"))?;
        if raw.len() != n * w * h {
            return Err(format!("{bin}: {} bytes for {n} {w}x{h} frames", raw.len()));
        }
        let mask = std::fs::read(dir.join(mask_bin))
            .ok()
            .filter(|m| m.len() == w * h)
            .unwrap_or_else(|| vec![1; w * h]);
        let frames = raw.chunks_exact(w * h).map(|c| c.to_vec()).collect();
        Ok(Self {
            x,
            y,
            w,
            h,
            frames,
            mask,
        })
    }

    fn blit(&self, frame: usize, buf: &mut [u8]) {
        let f = &self.frames[frame % self.frames.len()];
        for row in 0..self.h {
            for col in 0..self.w {
                if self.mask[row * self.w + col] != 0 {
                    buf[(self.y + row) * W + self.x + col] = f[row * self.w + col];
                }
            }
        }
    }
}

/// An indexed sprite bank (MMSPR / FONT1) kept CPU-side for blits.
struct Bank {
    atlas: Vec<u8>,
    atlas_w: usize,
    rects: Vec<Option<(usize, usize, usize, usize)>>,
}

impl Bank {
    fn load(dir: &Path, bin: &str, json: &str) -> Result<Self, String> {
        let atlas = std::fs::read(dir.join(bin)).map_err(|e| format!("{bin}: {e}"))?;
        let index: mgc_formats::bundle::SpriteIndex = serde_json::from_slice(
            &std::fs::read(dir.join(json)).map_err(|e| format!("{json}: {e}"))?,
        )
        .map_err(|e| format!("{json}: {e}"))?;
        let rects = index
            .sprites
            .iter()
            .map(|s| {
                let f = s.frames.first()?;
                (s.width > 0 && s.height > 0).then_some((
                    f.x as usize,
                    f.y as usize,
                    s.width as usize,
                    s.height as usize,
                ))
            })
            .collect();
        Ok(Self {
            atlas,
            atlas_w: index.atlas_width as usize,
            rects,
        })
    }

    /// Blit sprite pixels (index 0 transparent) into the screen.
    fn blit(&self, id: usize, x: i32, y: i32, buf: &mut [u8]) {
        let Some((sx, sy, w, h)) = self.rects.get(id).copied().flatten() else {
            return;
        };
        for row in 0..h {
            let py = y + row as i32;
            if py < 0 || py >= H as i32 {
                continue;
            }
            for col in 0..w {
                let px = x + col as i32;
                if px < 0 || px >= W as i32 {
                    continue;
                }
                let p = self.atlas[(sy + row) * self.atlas_w + sx + col];
                if p != 0 {
                    buf[py as usize * W + px as usize] = p;
                }
            }
        }
    }

    /// Blit a glyph as a solid color mask (fonts are 1-bit masks).
    fn blit_mask(&self, id: usize, x: i32, y: i32, color: u8, buf: &mut [u8]) {
        let Some((sx, sy, w, h)) = self.rects.get(id).copied().flatten() else {
            return;
        };
        for row in 0..h {
            let py = y + row as i32;
            if py < 0 || py >= H as i32 {
                continue;
            }
            for col in 0..w {
                let px = x + col as i32;
                if px < 0 || px >= W as i32 {
                    continue;
                }
                if self.atlas[(sy + row) * self.atlas_w + sx + col] != 0 {
                    buf[py as usize * W + px as usize] = color;
                }
            }
        }
    }

    fn width_of(&self, id: usize) -> usize {
        self.rects
            .get(id)
            .copied()
            .flatten()
            .map_or(4, |(_, _, w, _)| w)
    }
}

/// The modal state over the base menu.
enum Modal {
    /// Load confirm for a slot.
    ConfirmLoad {
        slot: usize,
    },
    ConfirmNew,
    ConfirmQuit,
    /// The rename dialog (button 2): edits the CURRENT save name.
    EditName {
        buf: String,
    },
}

/// Which submode the six slot regions are in (retail `byte_12CBCD`).
#[derive(Clone, Copy, PartialEq, Eq)]
enum Sub {
    None,
    Load,
    Save,
}

pub struct Mc1Menu {
    bg: Vec<u8>,
    /// The fully-unrolled SCROLL.DAT screen — the retail save/load
    /// dialog backdrop (None on an older bake: dark panel).
    scroll_bg: Option<Vec<u8>>,
    pal: [u8; 768],
    mask: Vec<u8>,
    globe: Movie,
    timer: Movie,
    sprites: Bank,
    font: Bank,
    /// ×1.30 brightness LUT per palette index (retail sub_504A0).
    bright: Vec<u8>,
    /// Palette index used for text ink (nearest-to-white).
    white: u8,
    /// Palette index for dim text ("--" rows).
    dim: u8,
    anim: f32,
    sub: Sub,
    modal: Option<Modal>,
    /// Slot labels + occupied, refreshed by the app on entry.
    slots: Vec<(String, bool)>,
    /// Timer runs only while a game is underway (retail
    /// `!byte_9687C`).
    pub game_active: bool,
    /// The campaign's current save name — pre-fills the rename
    /// dialog (deliberate: editing beats retyping, matching MC2's
    /// name dialog). Refreshed by the app on entry alongside the
    /// slots.
    pub player_name: String,
    pending: Option<Mc1Action>,
    /// Composed screen is dirty (anything changed since last
    /// resolve).
    screen: Vec<u8>,
}

impl Mc1Menu {
    pub fn load(dir: &Path) -> Result<Self, String> {
        let read = |name: &str| -> Result<Vec<u8>, String> {
            std::fs::read(dir.join(name)).map_err(|e| {
                format!(
                    "{}: {e} (rebake — epoch 18 adds it)",
                    dir.join(name).display()
                )
            })
        };
        let bg = read("menu-bg.bin")?;
        let mask = read("menu-mask.bin")?;
        let pal_v = read("menu-pal.bin")?;
        if bg.len() != W * H || mask.len() != W * H || pal_v.len() != 768 {
            return Err("mc1-ui menu members have unexpected sizes".into());
        }
        let scroll_bg = std::fs::read(dir.join("scroll-bg.bin"))
            .ok()
            .filter(|b| b.len() == W * H);
        let pal: [u8; 768] = pal_v.try_into().unwrap();
        let globe = Movie::load(dir, "globe.bin", "globe-mask.bin", "globe.json")?;
        let timer = Movie::load(dir, "timer.bin", "timer-mask.bin", "timer.json")?;
        let sprites = Bank::load(dir, "menu-sprites.bin", "menu-sprites.json")?;
        let font = Bank::load(dir, "font.bin", "font.json")?;
        // The ×1.30 brighten LUT: for each palette index find the
        // index whose color best matches the brightened color
        // (retail brightens the 6-bit palette entries directly and
        // installs them — an index remap reproduces it on a fixed
        // palette).
        let rgb = |i: usize| {
            [
                pal[i * 3] as i32,
                pal[i * 3 + 1] as i32,
                pal[i * 3 + 2] as i32,
            ]
        };
        let bright: Vec<u8> = (0..256)
            .map(|i| {
                let t = rgb(i).map(|c| ((c * 13) / 10).min(63));
                let mut best = (i, i32::MAX);
                for j in 0..256 {
                    let c = rgb(j);
                    let d = (0..3).map(|k| (c[k] - t[k]).pow(2)).sum();
                    if d < best.1 {
                        best = (j, d);
                    }
                }
                best.0 as u8
            })
            .collect();
        let brightness = |i: usize| rgb(i).iter().sum::<i32>();
        let white = (0..256).max_by_key(|&i| brightness(i)).unwrap_or(255) as u8;
        // Dim ink: mid-brightness pick (a parchment brown exists in
        // every retail menu palette; mid-range lands on it).
        let mut order: Vec<usize> = (0..256).collect();
        order.sort_by_key(|&i| brightness(i));
        let dim = order[170] as u8;
        Ok(Self {
            bg,
            scroll_bg,
            pal,
            mask,
            globe,
            timer,
            sprites,
            font,
            bright,
            white,
            dim,
            anim: 0.0,
            sub: Sub::None,
            modal: None,
            slots: Vec::new(),
            game_active: false,
            player_name: String::new(),
            pending: None,
            screen: vec![0; W * H],
        })
    }

    /// Refresh the slot list (labels; "--" = empty like retail).
    pub fn set_slots(&mut self, slots: Vec<(String, bool)>) {
        self.slots = slots;
    }

    pub fn tick(&mut self, dt: f32) {
        self.anim += dt;
    }

    pub fn take_action(&mut self) -> Option<Mc1Action> {
        self.pending.take()
    }

    pub fn editing(&self) -> bool {
        matches!(self.modal, Some(Modal::EditName { .. }))
    }

    /// Esc: close the modal / leave the submode. Never arms the quit
    /// confirm (Esc doubles as the in-game release/abandon key;
    /// quitting is the Quit hotspot's job).
    pub fn escape(&mut self) {
        if self.modal.is_some() {
            self.modal = None;
        } else if self.sub != Sub::None {
            self.sub = Sub::None;
        }
    }

    pub fn key_char(&mut self, c: char) {
        if !(c == ' ' || c.is_ascii_alphanumeric()) {
            return;
        }
        match &mut self.modal {
            Some(Modal::EditName { buf }) if buf.len() < 20 => {
                buf.push(c.to_ascii_uppercase());
            }
            _ => {}
        }
    }

    pub fn key_backspace(&mut self) {
        if let Some(Modal::EditName { buf }) = &mut self.modal {
            buf.pop();
        }
    }

    pub fn key_enter(&mut self) {
        match self.modal.take() {
            Some(Modal::EditName { buf }) => {
                self.pending = Some(Mc1Action::SetName(buf));
            }
            other => self.modal = other,
        }
    }

    /// The hotspot id under a 320-space point, gated by retail
    /// validity (:58861): multiplayer needs a network, slots need a
    /// submode, Continue needs an active game.
    fn hot_id(&self, mx: f32, my: f32) -> Option<u8> {
        if !(0.0..W as f32).contains(&mx) || !(0.0..H as f32).contains(&my) {
            return None;
        }
        let id = self.mask[my as usize * W + mx as usize];
        let valid = match id {
            0 => false,
            3 => false, // multiplayer: no network in this remake
            7..=10 => self.sub != Sub::None,
            11 => self.game_active,
            _ => true,
        };
        valid.then_some(id)
    }

    /// The retail OK / Cancel / name-field hit rects (320-space,
    /// :59141-66).
    fn ok_hit(mx: f32, my: f32) -> bool {
        (68.0..=81.0).contains(&mx) && (106.0..=116.0).contains(&my)
    }
    fn cancel_hit(mx: f32, my: f32) -> bool {
        (240.0..=250.0).contains(&mx) && (105.0..=115.0).contains(&my)
    }

    pub fn click(&mut self, size: (f32, f32), cursor: (f32, f32)) {
        let scale = (size.0 / W as f32).min(size.1 / H as f32);
        let (mx, my) = (cursor.0 / scale, cursor.1 / scale);
        if self.modal.is_some() {
            let ok = Self::ok_hit(mx, my);
            let cancel = Self::cancel_hit(mx, my);
            match self.modal.take() {
                Some(Modal::EditName { buf }) => {
                    if ok {
                        self.pending = Some(Mc1Action::SetName(buf));
                    } else if !cancel {
                        self.modal = Some(Modal::EditName { buf });
                    }
                }
                Some(Modal::ConfirmLoad { slot }) => {
                    if ok {
                        self.pending = Some(Mc1Action::LoadFrom(slot));
                        self.sub = Sub::None;
                    } else if !cancel {
                        self.modal = Some(Modal::ConfirmLoad { slot });
                    }
                }
                Some(Modal::ConfirmNew) => {
                    if ok {
                        self.pending = Some(Mc1Action::NewGame);
                    } else if !cancel {
                        self.modal = Some(Modal::ConfirmNew);
                    }
                }
                Some(Modal::ConfirmQuit) => {
                    if ok {
                        self.pending = Some(Mc1Action::Quit);
                    } else if !cancel {
                        self.modal = Some(Modal::ConfirmQuit);
                    }
                }
                None => {}
            }
            return;
        }
        let Some(id) = self.hot_id(mx, my) else {
            return;
        };
        match (id, self.sub) {
            // Submode: the six regions are slots 1-6.
            (5..=10, Sub::Load) => {
                let slot = id as usize - 5;
                if self.slots.get(slot).is_some_and(|s| s.1) {
                    self.modal = Some(Modal::ConfirmLoad { slot });
                }
            }
            // Picking a slot SAVES. No label editor in between: the
            // row is composed for display (player name + level +
            // progress), so seeding an editor from it and writing the
            // result back accumulated the suffix on every save.
            (5..=10, Sub::Save) => {
                self.pending = Some(Mc1Action::SaveTo {
                    slot: id as usize - 5,
                });
                self.sub = Sub::None;
            }
            // New Game asks only when there IS progress to lose
            // (retail confirms via the byte_9687C gate; a fresh
            // profile starts straight away).
            (1, _) => {
                if self.game_active {
                    self.modal = Some(Modal::ConfirmNew);
                } else {
                    self.pending = Some(Mc1Action::NewGame);
                }
            }
            (2, _) => {
                // Pre-filled with the current name — the dialog
                // EDITS it (backspace to clear) instead of asking
                // from scratch every time.
                self.modal = Some(Modal::EditName {
                    buf: self.player_name.clone(),
                });
            }
            (4, _) => self.modal = Some(Modal::ConfirmQuit),
            (5, Sub::None) => self.sub = Sub::Load,
            (6, Sub::None) => self.sub = Sub::Save,
            (11, _) => self.pending = Some(Mc1Action::Continue),
            _ => {}
        }
    }

    fn text(&self, s: &str, x: i32, y: i32, color: u8, buf: &mut [u8]) {
        let mut cx = x;
        for c in s.chars() {
            let id = (c as usize).wrapping_add(1);
            if c != ' ' {
                self.font.blit_mask(id, cx, y, color, buf);
            }
            cx += self.font.width_of(id) as i32;
        }
    }

    fn text_width(&self, s: &str) -> usize {
        s.chars()
            .map(|c| self.font.width_of((c as usize).wrapping_add(1)))
            .sum()
    }

    /// Compose + resolve this frame's screen; returns the RGBA quad
    /// set (one screen quad; the atlas is re-uploaded by the app).
    pub fn frame(&mut self, size: (f32, f32), cursor: (f32, f32)) -> (Vec<u8>, Vec<UiQuad>) {
        let scale = (size.0 / W as f32).min(size.1 / H as f32);
        let (mx, my) = (cursor.0 / scale, cursor.1 / scale);
        let mut buf = std::mem::take(&mut self.screen);
        buf.copy_from_slice(&self.bg);
        // The menu movies: retail steps them on alternate menu
        // frames (~15 fps effective); the globe loops always, the
        // timer only mid-campaign.
        let step = (self.anim * 15.0) as usize;
        self.globe.blit(step, &mut buf);
        // The hourglass runs only mid-campaign (retail !byte_9687C);
        // at rest the base art's own hourglass shows untouched.
        if self.game_active {
            self.timer.blit(step / 5, &mut buf);
        }
        // Toplevel menu: the disk icons on the two top books (MMSPR
        // 1 = Load on the top book, 2 = Save on the second — retail
        // :59043-47, 640-buffer (358,10)/(336,86) halved). They
        // vanish while a submode lists the slots and return with
        // the base menu.
        if self.sub == Sub::None {
            self.sprites.blit(1, 179, 5, &mut buf);
            self.sprites.blit(2, 168, 43, &mut buf);
        }
        // Submode: the six slot labels centered in the book rects
        // (the right-side book stack IS the slot list).
        if self.sub != Sub::None {
            for (k, r) in SLOT_RECTS.iter().enumerate() {
                let (label, occupied) = self
                    .slots
                    .get(k)
                    .cloned()
                    .unwrap_or_else(|| ("--".into(), false));
                let w = self.text_width(&label) as f32;
                let x = (r.0 + (r.2 - w) / 2.0) as i32;
                let y = (r.1 + r.3 / 2.0 - 3.0) as i32;
                let color = if occupied { self.white } else { self.dim };
                self.text(&label, x, y, color, &mut buf);
            }
        }
        // Modal: the retail scroll.dat parchment screen as the
        // backdrop (the fully-unrolled last frame; retail plays the
        // unroll movie — the end state is the dialog), MMSPR art,
        // OK/Cancel at their retail hit spots, texts.
        let caret_on = (self.anim * 4.0) as u32 % 2 == 0;
        if let Some(m) = &self.modal {
            if let Some(scroll) = &self.scroll_bg {
                buf.copy_from_slice(scroll);
            } else {
                // Older bake (no scroll art): brightened panel stand-in.
                for y in 70..125 {
                    for x in 60..260 {
                        let i = y * W + x;
                        buf[i] = self.bright[buf[i] as usize];
                    }
                }
            }
            let mut texts: Vec<(String, i32, i32)> = Vec::new();
            let centered = |menu: &Self, s: &str| 65 + (189 - menu.text_width(s) as i32) / 2;
            match m {
                Modal::EditName { buf: edit } => {
                    // Retail asks "Enter your name:" AND "Enter your
                    // call-name:" (etext 34/35) with only the
                    // call-name ever used — collapsed to ONE name
                    // (deliberate, MC2-style).
                    let prompt = "What is your name";
                    texts.push((prompt.into(), centered(self, prompt), 78));
                    let mut shown = edit.clone();
                    if caret_on {
                        shown.push('_');
                    }
                    texts.push((shown, 112, 90));
                }
                Modal::ConfirmLoad { slot } => {
                    let label = self
                        .slots
                        .get(*slot)
                        .map(|s| s.0.clone())
                        .unwrap_or_default();
                    texts.push((label.clone(), centered(self, &label), 92));
                }
                Modal::ConfirmNew => {
                    // Retail sentence (etext 36).
                    let prompt = "New Game? Yes/No";
                    texts.push((prompt.into(), centered(self, prompt), 92));
                }
                Modal::ConfirmQuit => {
                    // The exit-to-DOS icon (MMSPR 7, the C:\ scroll —
                    // the same art as the base screen's bottom-center
                    // Quit hotspot), centered in the confirm
                    // viewport (65,75,189,44), captioned with the
                    // retail sentence (etext 33).
                    let prompt = "Quit to DOS";
                    texts.push((prompt.into(), centered(self, prompt), 78));
                    self.sprites.blit(7, 143, 90, &mut buf);
                }
            }
            self.sprites.blit(5, 66, 104, &mut buf);
            self.sprites.blit(6, 238, 104, &mut buf);
            let white = self.white;
            for (s, x, y) in texts {
                self.text(&s, x, y, white, &mut buf);
            }
        }
        // Resolve indices → RGBA with the hover brighten (only in
        // the base menu — retail highlights only live hotspots).
        let hover = if self.modal.is_none() {
            self.hot_id(mx, my)
        } else {
            None
        };
        let mut rgba = vec![0u8; W * H * 4];
        for i in 0..W * H {
            let mut idx = buf[i] as usize;
            if let Some(h) = hover
                && self.mask[i] == h
            {
                idx = self.bright[idx] as usize;
            }
            let o = i * 4;
            rgba[o] = self.pal[idx * 3] << 2;
            rgba[o + 1] = self.pal[idx * 3 + 1] << 2;
            rgba[o + 2] = self.pal[idx * 3 + 2] << 2;
            rgba[o + 3] = 255;
        }
        self.screen = buf;
        let quads = vec![UiQuad {
            rect: [0.0, 0.0, W as f32 * scale, H as f32 * scale],
            uv: [0.0, 0.0, W as f32, H as f32],
            tint: [1.0, 1.0, 1.0, 1.0],
        }];
        (rgba, quads)
    }
}
