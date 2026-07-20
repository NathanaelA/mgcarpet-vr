//! The MC2 main menu — the retail "temple" frontend screen
//! (`MainMenu_76FA0`, MenusAndIntros.cpp:816; HSCREEN0 case-4 art;
//! docs/traces/mc2-campaign-save-menu.md "Main menu" recon).
//!
//! Retail law reproduced:
//! - 640×480 temple background with its own palette; button art is
//!   part of the background — hovering draws the lit sprite over it
//!   (the `byte_21` index, MI:2035-38), clicking plays sample 14.
//! - Idle dressing: two fires (sprites 1-8 / 9-16) and two incense
//!   burners (17-25 / 26-34) at ~25 fps (step every 4 ticks of the
//!   100 Hz clock); sprite 66 blitted permanently at (185,232)
//!   (MI:897).
//! - Cursor = case-4 sprite 39.
//! - Dialogs = the parchment scroll (strip sprite 72, OK 70/71,
//!   Cancel 68/69 — the case-4 `x_WORD_17DF06..0E` registry), same
//!   16-px-step opening as the map screen's.
//! - Name entry (`SetPlayerNameDialog_78E00` MI:4799): chars
//!   space/0-9/letters, uppercased, max 12, blinking `_` caret.
//! - Save/Load slot dialogs: 8 rows at 16-px pitch, "Empty" for
//!   vacant slots, Save rows editable (max 15).
//!
//! Buttons wired: New Game (enter the map — the campaign RESET lives
//! on the map's corner button, retail law), Set Name, Save, Load,
//! Exit. Multiplayer / Set Keys / Language / Joystick draw their
//! hover art but report as not-in-this-remake when clicked.

use std::path::Path;

use mgc_render::UiQuad;

/// One frontend action, drained by the app.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MenuAction {
    /// New Game / the door — enter the world-map screen.
    EnterMap,
    SaveTo {
        slot: usize,
        label: String,
    },
    LoadFrom(usize),
    SetName(String),
    Quit,
}

/// The frontend click sample (MI:5858).
const SND_CLICK: u8 = 14;

/// OK / Cancel positions+hit rects (retail DrawScrollDialog2 mode 3:
/// OK at x1+15, Cancel right-aligned to x1+barW-12, bottoms on
/// y1+height).
fn ok_rect(x1: f32, y1: f32, h: f32) -> (f32, f32, f32, f32) {
    (x1 + 15.0, y1 + h - 28.0, 42.0, 28.0)
}
fn cancel_rect(x1: f32, y1: f32, h: f32) -> (f32, f32, f32, f32) {
    (x1 + DIALOG_W - 12.0 - 39.0, y1 + h - 30.0, 39.0, 30.0)
}
fn in_rect(mx: f32, my: f32, r: (f32, f32, f32, f32)) -> bool {
    mx >= r.0 && mx < r.0 + r.2 && my >= r.1 && my < r.1 + r.3
}

/// Screen dimensions (retail 640×480).
const W: f32 = 640.0;
const H: f32 = 480.0;
/// The parchment strip width (sprite 72).
const DIALOG_W: f32 = 114.0;

/// The nine temple buttons (`str_E1BAC`, MI:334-44): position,
/// hit-box, the hover sprite (`byte_21` — retail DrawMenuAnimations
/// draws this index for the hovered button), and what a click does.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Button {
    NewGame,
    SetName,
    Multiplayer,
    Save,
    SetKeys,
    Load,
    Exit,
    Language,
    Joystick,
}

const BUTTONS: [(Button, (f32, f32), (f32, f32), usize); 9] = [
    (Button::NewGame, (206.0, 67.0), (80.0, 80.0), 51),
    (Button::SetName, (281.0, 65.0), (80.0, 80.0), 52),
    (Button::Multiplayer, (362.0, 72.0), (80.0, 80.0), 53),
    (Button::Save, (200.0, 157.0), (80.0, 80.0), 54),
    (Button::SetKeys, (405.0, 231.0), (60.0, 44.0), 106),
    (Button::Load, (391.0, 158.0), (80.0, 80.0), 55),
    (Button::Exit, (294.0, 25.0), (52.0, 44.0), 56),
    (Button::Language, (289.0, 155.0), (60.0, 44.0), 57),
    (Button::Joystick, (185.0, 232.0), (60.0, 44.0), 58),
];

/// The idle set dressing (MI:443-48): (x, y, first, last).
const ANIMS: [(f32, f32, usize, usize); 4] = [
    (17.0, 159.0, 1, 8),    // left fire
    (531.0, 156.0, 9, 16),  // right fire
    (154.0, 308.0, 17, 25), // left incense
    (482.0, 308.0, 26, 34), // right incense
];

enum Modal {
    /// Name entry ((356,112) h 80, title 420).
    Name { buf: String },
    /// Slot picker; `save` = editable rows (else Load).
    Slots {
        save: bool,
        open: f32,
        slots: Vec<(String, bool)>,
        selected: Option<usize>,
        edit: Option<String>,
    },
    /// Exit confirm ((352,26) h 80, title 407 "Exit").
    ExitConfirm,
}

pub struct MainMenu {
    atlas: Vec<u8>,
    atlas_w: u32,
    atlas_h: u32,
    rects: Vec<Option<(f32, f32, f32, f32)>>,
    font: Vec<Option<(f32, f32, f32, f32)>>,
    strings: Vec<String>,
    anim: f32,
    modal: Option<Modal>,
    pending: Option<MenuAction>,
    sounds: Vec<u8>,
}

impl MainMenu {
    /// Load the case-4 members out of the baked `assets/mc2-ui`
    /// bundle.
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
        if bg.len() != (W as usize) * (H as usize) {
            return Err(format!("menu-bg.bin: {} bytes", bg.len()));
        }
        let pal = read("menu-pal.bin")?;
        if pal.len() != 768 {
            return Err("menu-pal.bin: not 768 bytes".into());
        }
        let sprites_px = read("menu-sprites.bin")?;
        let index: mgc_formats::bundle::SpriteIndex =
            serde_json::from_slice(&read("menu-sprites.json")?)
                .map_err(|e| format!("menu-sprites.json: {e}"))?;
        let font_px = read("font.bin")?;
        let font_index: mgc_formats::bundle::SpriteIndex =
            serde_json::from_slice(&read("font.json")?).map_err(|e| format!("font.json: {e}"))?;
        let strings: Vec<String> = serde_json::from_slice(&read("strings.json")?)
            .map_err(|e| format!("strings.json: {e}"))?;

        let rgb =
            |i: usize| -> [u8; 3] { [pal[i * 3] << 2, pal[i * 3 + 1] << 2, pal[i * 3 + 2] << 2] };
        // Atlas: bg (640×480), sprite bank below, font masks below
        // that. Width = 640 (the packed banks are 512 wide).
        let atlas_w = W as u32;
        let sprites_y = H as usize;
        let font_y = sprites_y + index.atlas_height as usize;
        let atlas_h = font_y as u32 + font_index.atlas_height;
        let mut atlas = vec![0u8; (atlas_w * atlas_h * 4) as usize];
        for (i, &p) in bg.iter().enumerate() {
            let c = rgb(p as usize);
            let o = ((i / W as usize) * atlas_w as usize + i % W as usize) * 4;
            atlas[o..o + 3].copy_from_slice(&c);
            atlas[o + 3] = 255;
        }
        for y in 0..index.atlas_height as usize {
            for x in 0..index.atlas_width as usize {
                let p = sprites_px[y * index.atlas_width as usize + x];
                if p == 0 {
                    continue;
                }
                let c = rgb(p as usize);
                let o = ((sprites_y + y) * atlas_w as usize + x) * 4;
                atlas[o..o + 3].copy_from_slice(&c);
                atlas[o + 3] = 255;
            }
        }
        for y in 0..font_index.atlas_height as usize {
            for x in 0..font_index.atlas_width as usize {
                if font_px[y * font_index.atlas_width as usize + x] == 0 {
                    continue;
                }
                let o = ((font_y + y) * atlas_w as usize + x) * 4;
                atlas[o..o + 4].copy_from_slice(&[255, 255, 255, 255]);
            }
        }
        let rect_at = |s: &mgc_formats::bundle::SpriteEntry, base: usize| {
            let f = s.frames.first()?;
            (s.width > 0 && s.height > 0).then_some((
                f.x as f32,
                (f.y as usize + base) as f32,
                s.width as f32,
                s.height as f32,
            ))
        };
        let rects = index
            .sprites
            .iter()
            .map(|s| rect_at(s, sprites_y))
            .collect();
        let font = font_index
            .sprites
            .iter()
            .map(|s| rect_at(s, font_y))
            .collect();
        Ok(Self {
            atlas,
            atlas_w,
            atlas_h,
            rects,
            font,
            strings,
            anim: 0.0,
            modal: None,
            pending: None,
            sounds: Vec::new(),
        })
    }

    pub fn atlas(&self) -> (u32, u32, &[u8]) {
        (self.atlas_w, self.atlas_h, &self.atlas)
    }

    pub fn tick(&mut self, dt: f32) {
        self.anim += dt;
        if let Some(Modal::Slots { open, .. }) = &mut self.modal {
            *open = (*open + 16.0 * 70.0 * dt).min(200.0);
        }
    }

    pub fn take_action(&mut self) -> Option<MenuAction> {
        self.pending.take()
    }

    pub fn take_sounds(&mut self) -> Vec<u8> {
        std::mem::take(&mut self.sounds)
    }

    /// A modal edit field is accepting keystrokes.
    pub fn editing(&self) -> bool {
        matches!(
            &self.modal,
            Some(Modal::Name { .. }) | Some(Modal::Slots { edit: Some(_), .. })
        )
    }

    /// Esc: close the modal only. Retail auto-selects the Exit
    /// button (MI:5842-43), but Esc also serves as the
    /// pointer-release/abandon key in play, so here it never quits
    /// (deliberate) — quitting is the Exit button's job.
    pub fn escape(&mut self) {
        self.modal = None;
    }

    /// Keystrokes: the name filter is retail `sub_7C200` (space,
    /// 0-9, letters), uppercased, max 12 (MI:4848-73); slot labels
    /// max 15.
    pub fn key_char(&mut self, c: char) {
        if !(c == ' ' || c.is_ascii_alphanumeric()) {
            return;
        }
        match &mut self.modal {
            Some(Modal::Name { buf }) => {
                if buf.len() < 12 {
                    buf.push(c.to_ascii_uppercase());
                }
            }
            Some(Modal::Slots { edit: Some(e), .. }) if e.len() < 15 => {
                e.push(c);
            }
            _ => {}
        }
    }

    pub fn key_backspace(&mut self) {
        match &mut self.modal {
            Some(Modal::Name { buf }) => {
                buf.pop();
            }
            Some(Modal::Slots { edit: Some(e), .. }) => {
                e.pop();
            }
            _ => {}
        }
    }

    /// Enter: commit the name / close a slot edit field.
    pub fn key_enter(&mut self) {
        match &mut self.modal {
            Some(Modal::Name { buf }) => {
                self.pending = Some(MenuAction::SetName(std::mem::take(buf)));
                self.modal = None;
            }
            Some(Modal::Slots {
                edit,
                selected,
                slots,
                ..
            }) => {
                if let (Some(e), Some(k)) = (edit.take(), *selected)
                    && let Some(s) = slots.get_mut(k)
                {
                    s.0 = e;
                }
            }
            _ => {}
        }
    }

    /// Open the slot picker (the app scans SAVE%d.GAM labels).
    pub fn open_slots(&mut self, save: bool, slots: Vec<(String, bool)>) {
        self.modal = Some(Modal::Slots {
            save,
            open: 0.0,
            slots,
            selected: None,
            edit: None,
        });
    }

    /// Open the name-entry dialog seeded with the current name.
    pub fn open_name(&mut self, current: &str) {
        self.modal = Some(Modal::Name {
            buf: current.to_string(),
        });
    }

    fn button_hit(mx: f32, my: f32) -> Option<Button> {
        BUTTONS
            .iter()
            .find(|(_, pos, size, _)| {
                mx >= pos.0 && mx < pos.0 + size.0 && my >= pos.1 && my < pos.1 + size.1
            })
            .map(|&(b, _, _, _)| b)
    }

    /// Left-click at a window position. Returns the button the app
    /// must service with a slot scan (Save/Load), if any.
    pub fn click(&mut self, size: (f32, f32), cursor: (f32, f32)) -> Option<&'static str> {
        let scale = (size.0 / W).min(size.1 / H);
        let (mx, my) = (cursor.0 / scale, cursor.1 / scale);
        if self.modal.is_some() {
            self.modal_click(mx, my);
            return None;
        }
        let btn = Self::button_hit(mx, my)?;
        self.sounds.push(SND_CLICK);
        match btn {
            Button::NewGame => self.pending = Some(MenuAction::EnterMap),
            Button::SetName => return Some("name"),
            Button::Save => return Some("save"),
            Button::Load => return Some("load"),
            Button::Exit => self.modal = Some(Modal::ExitConfirm),
            Button::Multiplayer | Button::SetKeys | Button::Language | Button::Joystick => {
                println!("main menu: that button is not part of this remake (yet)");
            }
        }
        None
    }

    fn modal_click(&mut self, mx: f32, my: f32) {
        let Some(modal) = &mut self.modal else { return };
        match modal {
            Modal::Name { buf } => {
                let (x1, y1, h) = (356.0 - DIALOG_W / 2.0, 112.0, 80.0);
                if in_rect(mx, my, ok_rect(x1, y1, h)) {
                    self.sounds.push(SND_CLICK);
                    self.pending = Some(MenuAction::SetName(std::mem::take(buf)));
                    self.modal = None;
                } else if in_rect(mx, my, cancel_rect(x1, y1, h)) {
                    self.sounds.push(SND_CLICK);
                    self.modal = None;
                }
            }
            Modal::ExitConfirm => {
                let (x1, y1, h) = (352.0 - DIALOG_W / 2.0, 26.0, 80.0);
                if in_rect(mx, my, ok_rect(x1, y1, h)) {
                    self.sounds.push(SND_CLICK);
                    self.pending = Some(MenuAction::Quit);
                    self.modal = None;
                } else if in_rect(mx, my, cancel_rect(x1, y1, h)) {
                    self.sounds.push(SND_CLICK);
                    self.modal = None;
                }
            }
            Modal::Slots {
                save,
                open,
                slots,
                selected,
                edit,
            } => {
                if *open < 200.0 {
                    return;
                }
                // Retail anchors: Save (78,160), Load (448,160).
                // Rows at y1+32+16k (retail y1+16*(k+1), 1-based).
                let (x1, y1, h) = (if *save { 78.0 } else { 448.0 }, 160.0, 200.0);
                for k in 0..slots.len() {
                    let ry = y1 + 32.0 + 16.0 * k as f32;
                    if mx >= x1 + 10.0 && mx < x1 + 10.0 + 92.0 && my >= ry && my < ry + 16.0 {
                        if !*save && !slots[k].1 {
                            return;
                        }
                        *selected = Some(k);
                        if *save {
                            *edit = Some(if slots[k].1 {
                                slots[k].0.clone()
                            } else {
                                String::new()
                            });
                        }
                        self.sounds.push(SND_CLICK);
                        return;
                    }
                }
                if in_rect(mx, my, ok_rect(x1, y1, h)) {
                    self.sounds.push(SND_CLICK);
                    let action = if *save {
                        let label = edit.take().or_else(|| selected.map(|k| slots[k].0.clone()));
                        selected.map(|k| MenuAction::SaveTo {
                            slot: k,
                            label: label.unwrap_or_default(),
                        })
                    } else {
                        selected.filter(|&k| slots[k].1).map(MenuAction::LoadFrom)
                    };
                    if let Some(a) = action {
                        self.pending = Some(a);
                        self.modal = None;
                    }
                } else if in_rect(mx, my, cancel_rect(x1, y1, h)) {
                    self.sounds.push(SND_CLICK);
                    self.modal = None;
                }
            }
        }
    }

    fn sprite(&self, id: usize, pos: (f32, f32), scale: f32) -> Option<UiQuad> {
        let (sx, sy, w, h) = self.rects.get(id).copied().flatten()?;
        Some(UiQuad {
            rect: [pos.0 * scale, pos.1 * scale, w * scale, h * scale],
            uv: [sx, sy, w, h],
            tint: [1.0, 1.0, 1.0, 1.0],
        })
    }

    fn text(&self, s: &str, x: f32, y: f32, color: [f32; 4], scale: f32) -> Vec<UiQuad> {
        let mut out = Vec::new();
        let mut cx = x;
        for c in s.chars() {
            let id = (c as usize).wrapping_add(1);
            let Some((sx, sy, w, h)) = self.font.get(id).copied().flatten() else {
                cx += 4.0;
                continue;
            };
            if c != ' ' {
                out.push(UiQuad {
                    rect: [cx * scale, y * scale, w * scale, h * scale],
                    uv: [sx, sy, w, h],
                    tint: color,
                });
            }
            cx += w;
        }
        out
    }

    fn text_width(&self, s: &str) -> f32 {
        s.chars()
            .map(|c| {
                self.font
                    .get((c as usize).wrapping_add(1))
                    .copied()
                    .flatten()
                    .map_or(4.0, |(_, _, w, _)| w)
            })
            .sum()
    }

    /// The retail unrolled-scroll dialog panel at (x1,y1), open to
    /// `openh` (DrawScrollDialog2 law — the roller bar at the top
    /// AND the animated bottom edge, a solid parchment fill with
    /// vertical edge lines between them; ONE scroll, not a stack).
    fn parchment(&self, x1: f32, y1: f32, openh: f32, scale: f32, out: &mut Vec<UiQuad>) {
        let (bar_w, bar_h) = self
            .rects
            .get(72)
            .copied()
            .flatten()
            .map_or((DIALOG_W, 12.0), |(_, _, w, h)| (w, h));
        let solid = |r: [f32; 4], tint: [f32; 4]| UiQuad {
            rect: [r[0] * scale, r[1] * scale, r[2] * scale, r[3] * scale],
            uv: [0.0; 4],
            tint,
        };
        if openh > 0.0 {
            let parchment = [168.0 / 255.0, 144.0 / 255.0, 116.0 / 255.0, 1.0];
            let edge = [148.0 / 255.0, 124.0 / 255.0, 100.0 / 255.0, 1.0];
            let top = y1 + bar_h - 2.0;
            out.push(solid([x1 + 10.0, top, bar_w - 22.0, openh], parchment));
            out.push(solid([x1 + 10.0, top, 1.0, openh], edge));
            out.push(solid([x1 + bar_w - 12.0, top, 1.0, openh], edge));
        }
        if let Some(q) = self.sprite(72, (x1, y1), scale) {
            out.push(q);
        }
        if let Some(q) = self.sprite(72, (x1, y1 + openh), scale) {
            out.push(q);
        }
    }

    /// The title in dim ink under the top roller (retail 7FCB0 draw
    /// between x1+10 and x1+10+barW-22 at y1+barH+2).
    fn dialog_title(&self, idx: usize, x1: f32, y1: f32, scale: f32, out: &mut Vec<UiQuad>) {
        let ink = [88.0 / 255.0, 64.0 / 255.0, 36.0 / 255.0, 1.0];
        if let Some(t) = self.strings.get(idx) {
            let tx = x1 + 10.0 + (DIALOG_W - 22.0 - self.text_width(t)) / 2.0;
            out.extend(self.text(t, tx, y1 + 14.0, ink, scale));
        }
    }

    fn ok_cancel(
        &self,
        x1: f32,
        y1: f32,
        h: f32,
        cursor: (f32, f32),
        scale: f32,
        out: &mut Vec<UiQuad>,
    ) {
        let (mx, my) = cursor;
        let ok = ok_rect(x1, y1, h);
        let ca = cancel_rect(x1, y1, h);
        if let Some(q) = self.sprite(
            if in_rect(mx, my, ok) { 71 } else { 70 },
            (ok.0, ok.1),
            scale,
        ) {
            out.push(q);
        }
        if let Some(q) = self.sprite(
            if in_rect(mx, my, ca) { 69 } else { 68 },
            (ca.0, ca.1),
            scale,
        ) {
            out.push(q);
        }
    }

    /// This frame's quads (over black letterbox, like the map).
    pub fn quads(&self, size: (f32, f32), cursor: (f32, f32)) -> Vec<UiQuad> {
        let scale = (size.0 / W).min(size.1 / H);
        let mut quads = Vec::new();
        // Background.
        quads.push(UiQuad {
            rect: [0.0, 0.0, W * scale, H * scale],
            uv: [0.0, 0.0, W, H],
            tint: [1.0, 1.0, 1.0, 1.0],
        });
        // Idle dressing at ~25 fps.
        let frame = (self.anim * 25.0) as usize;
        for (x, y, first, last) in ANIMS {
            let id = first + frame % (last - first + 1);
            if let Some(q) = self.sprite(id, (x, y), scale) {
                quads.push(q);
            }
        }
        // The permanent (185,232) blit (MI:897).
        if let Some(q) = self.sprite(66, (185.0, 232.0), scale) {
            quads.push(q);
        }
        // Hover art (retail lights the hovered button only).
        let (mx, my) = (cursor.0 / scale, cursor.1 / scale);
        if self.modal.is_none()
            && let Some(btn) = Self::button_hit(mx, my)
            && let Some(&(_, pos, _, spr)) = BUTTONS.iter().find(|(b, ..)| *b == btn)
        {
            if let Some(q) = self.sprite(spr, pos, scale) {
                quads.push(q);
            }
        }
        // Modal on top.
        let caret_on = (self.anim * 4.0) as u32 % 2 == 0;
        match &self.modal {
            None => {}
            Some(Modal::Name { buf }) => {
                let (x1, y1, h) = (356.0 - DIALOG_W / 2.0, 112.0, 80.0);
                self.parchment(x1, y1, h, scale, &mut quads);
                self.dialog_title(420, x1, y1, scale, &mut quads);
                let ink = [88.0 / 255.0, 64.0 / 255.0, 36.0 / 255.0, 1.0];
                let mut shown = buf.clone();
                if caret_on {
                    shown.push('_');
                }
                quads.extend(self.text(&shown, x1 + 15.0, y1 + 30.0, ink, scale));
                self.ok_cancel(x1, y1, h, (mx, my), scale, &mut quads);
            }
            Some(Modal::ExitConfirm) => {
                let (x1, y1, h) = (352.0 - DIALOG_W / 2.0, 26.0, 80.0);
                self.parchment(x1, y1, h, scale, &mut quads);
                self.dialog_title(407, x1, y1, scale, &mut quads);
                self.ok_cancel(x1, y1, h, (mx, my), scale, &mut quads);
            }
            Some(Modal::Slots {
                save,
                open,
                slots,
                selected,
                edit,
            }) => {
                let (x1, y1, h) = (if *save { 78.0 } else { 448.0 }, 160.0, 200.0);
                self.parchment(x1, y1, *open, scale, &mut quads);
                if *open > 17.0 {
                    self.dialog_title(if *save { 422 } else { 421 }, x1, y1, scale, &mut quads);
                }
                if *open >= h {
                    let white = [1.0, 1.0, 1.0, 1.0];
                    let ink = [88.0 / 255.0, 64.0 / 255.0, 36.0 / 255.0, 1.0];
                    for (k, (label, _)) in slots.iter().enumerate() {
                        let ry = y1 + 32.0 + 16.0 * k as f32;
                        let sel = *selected == Some(k);
                        let shown = if sel && let Some(e) = edit {
                            let mut s = format!("{}. {e}", k + 1);
                            if caret_on {
                                s.push('_');
                            }
                            s
                        } else {
                            format!("{}. {label}", k + 1)
                        };
                        let color = if sel { white } else { ink };
                        quads.extend(self.text(&shown, x1 + 20.0, ry, color, scale));
                    }
                    self.ok_cancel(x1, y1, h, (mx, my), scale, &mut quads);
                }
            }
        }
        // Cursor (case-4 sprite 39).
        if let Some((sx, sy, w, h)) = self.rects.get(39).copied().flatten() {
            quads.push(UiQuad {
                rect: [cursor.0, cursor.1, w * scale, h * scale],
                uv: [sx, sy, w, h],
                tint: [1.0, 1.0, 1.0, 1.0],
            });
        }
        quads
    }
}
