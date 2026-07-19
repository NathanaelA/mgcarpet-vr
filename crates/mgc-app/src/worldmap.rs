//! The MC2 campaign world-map screen — the between-levels hub
//! (retail `NewGameDialog`/`DrawAnimTextsAndPlaySounds_7D400`,
//! MenusAndIntros.cpp; docs/traces/mc2-campaign-save-menu.md): the
//! 1280×960 scrolling map with animated portals, ambient set
//! dressing, and the travelling carpet.
//!
//! Retail law, scaled 2×:
//! - completed main = planted flag, frames 37-43; the NEXT portal
//!   pops into existence once per session (sound 41, frames 70-83,
//!   MI:2797-2823) then idles as the open portal 33-35; later
//!   portals are not drawn at all (the draw loop breaks on the
//!   first hidden one, MI:2825).
//! - secret revealed = the same pop-in then 270-272; completed
//!   secret = 305-311 (MI:2828-2879).
//! - the carpet (8 heading families of 4 frames, sprites 1-32)
//!   flies portal-to-portal at ~6 px/frame (3× Bresenham steps of
//!   2, `sub_80D40`/`MoveAnimObject_7E9D0`), stamping trail dot
//!   sprite 139 into the map every >8 px (`DrawMapObject_812D0`) —
//!   the dotted route line. Travel sound 19.
//! - ambient decorations: the `x_BYTE_E26C8_str[16]` table
//!   (MI:199-216) via `DrawAnimSprite_81CA0` (EF:46934): loop rows
//!   draw always; burst rows are INVISIBLE while waiting, then play
//!   frames first..last-1 once. The frame-85/86 rows vanish once the
//!   finale portal opens (MI:2786).
//! - cursor = bank sprite 239 (MI:986 — the map screen's own; 39 is
//!   the MAIN MENU chunk's cursor).
//! - anim cadence: 100 Hz clock, portal/ambient step every 8 ticks
//!   (12.5 fps), carpet frames every 16 (6.25 fps).
//!
//! The screen renders as UI quads over one atlas (background +
//! sprite bank), swapped in for the level's UI atlas while up.

use std::collections::HashMap;
use std::path::Path;

use mgc_render::UiQuad;

use crate::campaign::{MC2_MAIN_PORTALS, MC2_PORTAL_HIT, MC2_SECRETS};
use crate::saves::Mc2Save;

/// Retail viewport size the map scrolls within.
const VIEW_W: f32 = 640.0;
const VIEW_H: f32 = 480.0;
/// The same in pixels (border frame rows).
const SCREEN_W: usize = 640;
/// Map dimensions (worldmap-bg.bin).
const MAP_W: usize = 1280;
const MAP_H: usize = 960;
/// Portal/ambient frame rate (clock step ≥8 of a 100 Hz clock).
const ANIM_FPS: f32 = 12.5;
/// Carpet frame rate (clock step ≥16).
const CARPET_FPS: f32 = 6.25;
/// Carpet travel speed: retail moves 3×2 px per ~60 Hz frame.
const TRAVEL_SPEED: f32 = 360.0;
/// Retail sample ids (MC2 sound bank).
const SND_PORTAL_OPEN: u8 = 41;
const SND_TRAVEL: u8 = 19;
/// The frontend click (every menu/map button, MI:2414).
const SND_CLICK: u8 = 14;

/// One drawable/clickable portal, resolved from the campaign record.
struct Portal {
    level: u32,
    pos: (f32, f32),
    state: PortalState,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum PortalState {
    /// Completed main level — the planted flag (replayable).
    Flag,
    /// The next uncompleted main level (pop-in, then open portal).
    Next,
    /// Revealed-uncompleted secret portal (pop-in, then 270-272).
    SecretRevealed,
    /// Completed secret portal (replayable).
    SecretDone,
}

/// The once-per-session portal materialization (retail `byte_19` /
/// `byte_16` — reset every launch, so reopening the map replays
/// nothing but a fresh reveal always pops).
enum Pop {
    Popping { started: f32 },
    Done,
}

/// The travelling carpet: parked position to portal-center.
struct Travel {
    pos: (f32, f32),
    target: (f32, f32),
    /// Level to launch on arrival (the clicked portal).
    launch: Option<u32>,
    /// This leg flies the canonical frontier segment (departing the
    /// last completed portal for the pending one) — the only flight
    /// that draws route dots (off-route trips never generate the
    /// line).
    on_route: bool,
}

/// Ambient set-dressing row (`x_BYTE_E26C8_str`, semantic fields).
/// `burst` rows wait invisible for `delay` seconds, play one cycle
/// (firing `sound` as it starts — the map's creature screams,
/// EF:46999-47009), repeat; loop rows draw forever. Frames span
/// `first..last` — the retail wrap at `last-2` makes `last` itself
/// unused (EF:46945-49). `start` = the authored initial frame
/// (phase-offsets the loop rows so the meteors don't fall in sync).
struct Ambient {
    pos: (f32, f32),
    first: u16,
    last: u16,
    start: u16,
    delay: f32,
    burst: bool,
    sound: Option<u8>,
}

/// The table verbatim (MI:200-215, terminator dropped). Sounds fire
/// only on rows whose `time4_22 != -1`: the two head-poke screams
/// (sample 38), the frame-86 burst at (545,54) (sample 23), and the
/// per-cycle meteor whoosh row (sample 5 — the burst twin of the
/// (831,245) loop meteor, delay 0 so it fires every cycle). The
/// frames-46-58 loop rows ARE the falling-star streaks — the one at
/// (630,607) sits by portal 3's region (the "star that fell" level).
const AMBIENTS: [Ambient; 15] = [
    Ambient {
        pos: (447.0, 628.0),
        first: 115,
        last: 138,
        start: 115,
        delay: 4.0,
        burst: true,
        sound: Some(38),
    },
    Ambient {
        pos: (876.0, 534.0),
        first: 115,
        last: 138,
        start: 117,
        delay: 8.0,
        burst: true,
        sound: Some(38),
    },
    Ambient {
        pos: (545.0, 54.0),
        first: 85,
        last: 86,
        start: 85,
        delay: 0.0,
        burst: false,
        sound: None,
    },
    Ambient {
        pos: (655.0, 58.0),
        first: 85,
        last: 86,
        start: 85,
        delay: 0.0,
        burst: false,
        sound: None,
    },
    Ambient {
        pos: (564.0, 88.0),
        first: 85,
        last: 86,
        start: 85,
        delay: 0.0,
        burst: false,
        sound: None,
    },
    Ambient {
        pos: (614.0, 123.0),
        first: 85,
        last: 86,
        start: 85,
        delay: 0.0,
        burst: false,
        sound: None,
    },
    Ambient {
        pos: (545.0, 54.0),
        first: 86,
        last: 92,
        start: 86,
        delay: 8.0,
        burst: true,
        sound: Some(23),
    },
    Ambient {
        pos: (655.0, 58.0),
        first: 86,
        last: 92,
        start: 88,
        delay: 4.0,
        burst: true,
        sound: None,
    },
    Ambient {
        pos: (564.0, 88.0),
        first: 86,
        last: 92,
        start: 89,
        delay: 22.0,
        burst: true,
        sound: None,
    },
    Ambient {
        pos: (614.0, 123.0),
        first: 86,
        last: 92,
        start: 90,
        delay: 21.0,
        burst: true,
        sound: None,
    },
    Ambient {
        pos: (831.0, 245.0),
        first: 46,
        last: 58,
        start: 49,
        delay: 0.0,
        burst: false,
        sound: None,
    },
    Ambient {
        pos: (831.0, 245.0),
        first: 46,
        last: 58,
        start: 49,
        delay: 0.0,
        burst: true,
        sound: Some(5),
    },
    Ambient {
        pos: (863.0, 329.0),
        first: 46,
        last: 58,
        start: 46,
        delay: 0.0,
        burst: false,
        sound: None,
    },
    Ambient {
        pos: (630.0, 607.0),
        first: 46,
        last: 58,
        start: 52,
        delay: 0.0,
        burst: false,
        sound: None,
    },
    Ambient {
        pos: (244.0, 632.0),
        first: 46,
        last: 58,
        start: 56,
        delay: 0.0,
        burst: false,
        sound: None,
    },
];

/// The four always-on map-screen corner buttons
/// (`mapMenuButtons_E23E0`, MI:321-26): grey idle / gold hover sprite
/// pairs, hit-box = the sprite's own dims (MI:2408-9).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MapButton {
    /// Bottom-right (581,427), sprites 246/247 — back to the menu.
    Exit,
    /// Bottom-left (0,427), 248/249 — confirm + campaign reset.
    NewGame,
    /// Top-left (0,0), 250/251.
    Save,
    /// Top-right (581,0), 252/253.
    Load,
}

const MAP_BUTTONS: [(MapButton, (f32, f32), usize, usize); 4] = [
    (MapButton::Exit, (581.0, 427.0), 246, 247),
    (MapButton::NewGame, (0.0, 427.0), 248, 249),
    (MapButton::Save, (0.0, 0.0), 250, 251),
    (MapButton::Load, (581.0, 0.0), 252, 253),
];

/// A committed frontend action, drained by the app.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MapAction {
    /// Write the campaign record to this slot under this label.
    SaveTo { slot: usize, label: String },
    /// Load this slot's record.
    LoadFrom(usize),
    /// Confirmed campaign reset (retail sub_7E640 — stays on the map).
    NewGame,
    /// Leave the map for the main menu (Exit button / Esc).
    ExitToMenu,
}

/// Which parchment dialog is up (retail scroll dialogs; anchors +
/// heights from the `str_26` descriptors, MI:321-26).
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum DialogKind {
    Save,
    Load,
    NewGame,
}

/// The pop-open parchment scroll dialog (`DrawScrollDialog_7BF20`
/// MI:5402: opens toward its full height in 16-px steps, OK/Cancel
/// once fully open; slot rows at 16-px pitch).
struct Dialog {
    kind: DialogKind,
    anchor: (f32, f32),
    height: f32,
    /// Animated opening height (16 px per retail frame).
    open: f32,
    /// Slot labels + occupied flags (scanned by the app on open).
    slots: Vec<(String, bool)>,
    selected: Option<usize>,
    /// Save-label edit buffer (Some while typing; retail sub_7F6A0:
    /// filtered chars, max 15, "_" caret).
    edit: Option<String>,
}

/// The retail scroll-dialog width — the roller-bar art (sprite 254,
/// 114 px); anchors 29/510 line the Save/Load dialogs up under
/// their corner buttons.
const DIALOG_W: f32 = 114.0;

/// OK / Cancel positions+hit rects (retail DrawScrollDialog2 mode 3:
/// OK at x1+15, Cancel right-aligned to x1+barW-12, bottoms on
/// y1+height — resting on the bottom roller).
fn dialog_button_rects(
    anchor: (f32, f32),
    height: f32,
    bar_w: f32,
) -> ((f32, f32, f32, f32), (f32, f32, f32, f32)) {
    let (x1, y1) = anchor;
    let ok = (x1 + 15.0, y1 + height - 28.0, 42.0, 28.0);
    let cancel = (x1 + bar_w - 12.0 - 39.0, y1 + height - 30.0, 39.0, 30.0);
    (ok, cancel)
}

fn in_rect(mx: f32, my: f32, r: (f32, f32, f32, f32)) -> bool {
    mx >= r.0 && mx < r.0 + r.2 && my >= r.1 && my < r.1 + r.3
}

pub struct WorldMap {
    /// RGBA atlas: the map background at (0,0)..(1280,960), the
    /// sprite bank's packed atlas blitted below it at y = 960, the
    /// 640×480 border frame below that, the FONT1 glyph masks
    /// (white, tinted per draw) at the bottom.
    atlas: Vec<u8>,
    atlas_w: u32,
    atlas_h: u32,
    /// Sprite id → (atlas x, atlas y, w, h), frame 0.
    rects: Vec<Option<(f32, f32, f32, f32)>>,
    /// The border frame's atlas rect (640×480, transparent center).
    border_rect: Option<(f32, f32, f32, f32)>,
    /// FONT1 glyph rects, id = char + 1 (glyphs are white masks).
    font: Vec<Option<(f32, f32, f32, f32)>>,
    /// The English frontend strings (LANGUAGE/L2.TXT): level
    /// descriptions at 23+level, dialog titles 421/422/467.
    strings: Vec<String>,
    dialog: Option<Dialog>,
    pending_action: Option<MapAction>,
    pending_button: Option<MapButton>,
    /// The pending level's description text stays up until a portal
    /// click starts a leg (re-arms per visit with the narrative).
    desc_dismissed: bool,
    /// Map scroll in retail 640×480 viewport units.
    pub scroll: (f32, f32),
    /// Animation clock (seconds).
    anim: f32,
    /// Per-portal-level materialization state (session-local).
    pop: HashMap<u32, Pop>,
    travel: Option<Travel>,
    /// Level launch armed by an arrived click-travel.
    pending_launch: Option<u32>,
    /// Sample ids fired this frame, drained by the app's mixer.
    sounds: Vec<u8>,
    /// The next-level narrative latch: retail plays the pending
    /// level's briefing (speech row = level, segment 0) once per
    /// map visit (`IsPlayingCDTrack_17E09D`,
    /// `PresentLevelDescription_80C30` MI:3596-3601).
    narrated: bool,
    /// The pending narrative, drained by the app (level number).
    pending_narrative: Option<u32>,
    /// Where the carpet rests between legs — the portal of the
    /// level just played (`set_parked`).
    parked: (f32, f32),
    /// The dotted route: the trail is the
    /// FIXED main-line path, identical on every load — the only
    /// question per segment is drawn-or-not. Segments up to the
    /// frontier stamp on map entry; the frontier segment (into a
    /// portal revealed THIS session's last completion) stays blank
    /// until the carpet actually flies it (or the next entry).
    /// `levels_completed` as of the previous entry — None = first
    /// entry this session (a load: full trail).
    last_seen_completed: Option<u32>,
    /// The frontier segment (last completed → pending portal) is
    /// stamped.
    frontier_drawn: bool,
    /// The edge-scroll ramp (retail `shift_step`, px/frame at the
    /// 70 Hz retail clock; 0 while no edge is touched).
    edge_step: f32,
}

impl WorldMap {
    /// Load the baked `assets/mc2-ui` world-map bundle.
    pub fn load(dir: &Path) -> Result<Self, String> {
        let read = |name: &str| -> Result<Vec<u8>, String> {
            std::fs::read(dir.join(name)).map_err(|e| {
                format!(
                    "{}: {e} (rebake — epoch 16 adds it)",
                    dir.join(name).display()
                )
            })
        };
        let bg = read("worldmap-bg.bin")?;
        if bg.len() != MAP_W * MAP_H {
            return Err(format!(
                "worldmap-bg.bin: {} bytes (want {})",
                bg.len(),
                MAP_W * MAP_H
            ));
        }
        let pal = read("worldmap-pal.bin")?;
        if pal.len() != 768 {
            return Err(format!("worldmap-pal.bin: {} bytes (want 768)", pal.len()));
        }
        let sprites_px = read("worldmap-sprites.bin")?;
        let index: mgc_formats::bundle::SpriteIndex =
            serde_json::from_slice(&read("worldmap-sprites.json")?)
                .map_err(|e| format!("worldmap-sprites.json: {e}"))?;
        if sprites_px.len() != (index.atlas_width * index.atlas_height) as usize {
            return Err("worldmap-sprites.bin does not match its index".into());
        }

        // Resolve the 6-bit VGA palette once (<<2 to 8-bit).
        let rgb =
            |i: usize| -> [u8; 3] { [pal[i * 3] << 2, pal[i * 3 + 1] << 2, pal[i * 3 + 2] << 2] };

        // The frontend overlay members. Optional: an older bake still
        // gets the map, just without the border overlay / dialogs /
        // description text.
        let border = std::fs::read(dir.join("worldmap-border.bin")).ok();
        let font_px = std::fs::read(dir.join("font.bin")).ok();
        let font_index: Option<mgc_formats::bundle::SpriteIndex> = font_px
            .as_ref()
            .and_then(|_| std::fs::read(dir.join("font.json")).ok())
            .and_then(|b| serde_json::from_slice(&b).ok());
        let strings: Vec<String> = std::fs::read(dir.join("strings.json"))
            .ok()
            .and_then(|b| serde_json::from_slice(&b).ok())
            .unwrap_or_default();
        if border.is_none() || font_index.is_none() || strings.is_empty() {
            eprintln!(
                "note: mc2-ui bundle predates epoch 18 — map overlay/menus degraded (rebake)"
            );
        }

        // One RGBA atlas: bg on top, sprite atlas below, then the
        // 640×480 border frame, then the FONT1 glyph masks (white —
        // tinted per draw). The bg is opaque; everything else
        // resolves index 0 to alpha 0 (the engine-wide transparent
        // index).
        let border_h = border.as_ref().map_or(0, |b| b.len() / SCREEN_W) as u32;
        let font_h = font_index.as_ref().map_or(0, |f| f.atlas_height);
        let atlas_w = MAP_W as u32;
        let atlas_h = MAP_H as u32 + index.atlas_height + border_h + font_h;
        let mut atlas = vec![0u8; (atlas_w * atlas_h * 4) as usize];
        for (i, &p) in bg.iter().enumerate() {
            let c = rgb(p as usize);
            let o = i * 4;
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
                let o = ((MAP_H + y) * MAP_W + x) * 4;
                atlas[o..o + 3].copy_from_slice(&c);
                atlas[o + 3] = 255;
            }
        }
        let border_y = MAP_H + index.atlas_height as usize;
        if let Some(b) = &border {
            for (i, &p) in b.iter().enumerate() {
                if p == 0 {
                    continue;
                }
                let c = rgb(p as usize);
                let o = ((border_y + i / SCREEN_W) * MAP_W + i % SCREEN_W) * 4;
                atlas[o..o + 3].copy_from_slice(&c);
                atlas[o + 3] = 255;
            }
        }
        let font_y = border_y + border_h as usize;
        let mut font_rects: Vec<Option<(f32, f32, f32, f32)>> = Vec::new();
        if let (Some(px), Some(fi)) = (&font_px, &font_index) {
            for y in 0..fi.atlas_height as usize {
                for x in 0..fi.atlas_width as usize {
                    if px[y * fi.atlas_width as usize + x] == 0 {
                        continue;
                    }
                    let o = ((font_y + y) * MAP_W + x) * 4;
                    atlas[o..o + 4].copy_from_slice(&[255, 255, 255, 255]);
                }
            }
            font_rects = fi
                .sprites
                .iter()
                .map(|s| {
                    let f = s.frames.first()?;
                    (s.width > 0 && s.height > 0).then_some((
                        f.x as f32,
                        (f.y as usize + font_y) as f32,
                        s.width as f32,
                        s.height as f32,
                    ))
                })
                .collect();
        }

        let rects = index
            .sprites
            .iter()
            .map(|s| {
                let f = s.frames.first()?;
                (s.width > 0 && s.height > 0).then_some((
                    f.x as f32,
                    (f.y + MAP_H as u32) as f32,
                    s.width as f32,
                    s.height as f32,
                ))
            })
            .collect();

        Ok(Self {
            atlas,
            atlas_w,
            atlas_h,
            rects,
            border_rect: border.map(|_| (0.0, border_y as f32, SCREEN_W as f32, border_h as f32)),
            font: font_rects,
            strings,
            dialog: None,
            pending_action: None,
            pending_button: None,
            desc_dismissed: false,
            scroll: (0.0, 0.0),
            anim: 0.0,
            pop: HashMap::new(),
            travel: None,
            pending_launch: None,
            sounds: Vec::new(),
            narrated: false,
            pending_narrative: None,
            parked: (0.0, 0.0),
            last_seen_completed: None,
            frontier_drawn: false,
            edge_step: 0.0,
        })
    }

    /// Forget the session-local presentation state — a LOADED or
    /// RESET campaign record starts a fresh session: pop-in latches
    /// replay, the route stamps in full on the next entry, the
    /// carpet is unparked (hidden until the first launch when
    /// nothing is completed).
    pub fn session_reset(&mut self) {
        self.pop.clear();
        self.travel = None;
        self.last_seen_completed = None;
        self.parked = (0.0, 0.0);
        self.dialog = None;
    }

    /// A fresh map visit: the narrative latch re-arms (retail
    /// resets its once-per-visit CD-track flag on entry), and the
    /// dotted route re-stamps — the frontier segment joins it
    /// UNLESS its portal was revealed by the completion that led
    /// here (then the carpet flying there draws it, or the next
    /// entry does).
    pub fn enter_visit(&mut self, save: &Mc2Save) {
        self.narrated = false;
        self.desc_dismissed = false;
        let completed = save.levels_completed;
        self.frontier_drawn = match self.last_seen_completed {
            // First sight this session (boot/load): the full trail.
            None => true,
            // Re-entry at the same frontier: stamped.
            Some(c) if c == completed => true,
            // The frontier just advanced: its segment waits for the
            // carpet (or the next visit).
            _ => false,
        };
        self.last_seen_completed = Some(completed);
    }

    /// The composed RGBA atlas for `Renderer::load_ui_atlas`.
    pub fn atlas(&self) -> (u32, u32, &[u8]) {
        (self.atlas_w, self.atlas_h, &self.atlas)
    }

    /// Park the scroll on a portal's authored viewport anchor
    /// (`viewPortPos`, the retail map-travel camera).
    pub fn anchor_to(&mut self, save: &Mc2Save) {
        let next = (save.levels_completed as usize).min(MC2_MAIN_PORTALS.len() - 1);
        let (vx, vy) = MC2_MAIN_PORTALS[next].viewport;
        self.scroll = (vx as f32, vy as f32);
        self.clamp();
    }

    /// The center of a main portal (retail anchors travel to the
    /// portal position plus half the flag sprite, MI:2283-90).
    fn portal_center(&self, i: usize) -> (f32, f32) {
        let (w, h) = self
            .rects
            .get(37)
            .copied()
            .flatten()
            .map_or((20.0, 20.0), |(_, _, w, h)| (w, h));
        let p = MC2_MAIN_PORTALS[i].pos;
        (p.0 as f32 + w / 2.0, p.1 as f32 + h / 2.0)
    }

    /// The map position of a level's portal center — mains anchor
    /// to the flag sprite footprint, secrets to their own spot.
    fn level_pos(&self, level: u32) -> (f32, f32) {
        if let Some(&(_, _, pos)) = MC2_SECRETS.iter().find(|&&(_, l, _)| l as u32 == level) {
            let half = MC2_PORTAL_HIT as f32 / 2.0;
            return (pos.0 as f32 + half, pos.1 as f32 + half);
        }
        self.portal_center((level as usize).min(MC2_MAIN_PORTALS.len() - 1))
    }

    /// Park the carpet on the level just played (completed, failed
    /// or replayed — the player's map position). Across save/load
    /// retail itself resets to the
    /// last activated portal (the `.GAM` stores no position), which
    /// is what `run.current` resolves to on resume.
    pub fn set_parked(&mut self, level: u32) {
        self.parked = self.level_pos(level);
    }

    /// Advance the animations, the ambient sounds, the narrative
    /// latch and the carpet.
    pub fn tick(&mut self, dt: f32, save: &Mc2Save) {
        let prev = self.anim;
        self.anim += dt;
        // The parchment scroll opens 16 px per retail frame
        // (DrawScrollDialog_7BF20 MI:5419-51, ~70 Hz).
        if let Some(d) = &mut self.dialog {
            d.open = (d.open + 16.0 * 70.0 * dt).min(d.height);
        }
        // Ambient burst sounds — fire as a burst's visible phase
        // begins (retail plays the sample on the wait→anim edge,
        // EF:46999-47009): the creature screams and the meteor
        // whoosh.
        for a in &AMBIENTS {
            let (Some(id), true) = (a.sound, a.burst) else {
                continue;
            };
            let count = (a.last - a.first).max(1) as f32;
            let period = a.delay + count / ANIM_FPS;
            let starts = |t: f32| ((t - a.delay) / period).floor();
            if self.anim > a.delay && starts(self.anim) > starts(prev) {
                self.sounds.push(id);
            }
        }
        // The pending-level narrative: once per map visit, after the
        // next portal has materialized — suppressed while that
        // level's secret portal is non-hidden (retail
        // `PresentLevelDescription_80C30` MI:3583-3601: text 23+lvl,
        // speech row = lvl segment 0, `IsPlayingCDTrack` latch).
        if !self.narrated && self.travel.is_none() {
            let next = save.levels_completed;
            if next < 25 {
                let suppressed = MC2_SECRETS.iter().enumerate().any(|(i, &(parent, _, _))| {
                    parent as u32 == next && matches!(save.secrets[i].activated, 1 | 2)
                });
                if !suppressed && matches!(self.pop.get(&next), Some(Pop::Done)) {
                    self.narrated = true;
                    self.pending_narrative = Some(next);
                }
            }
        }
        let Some(t) = &mut self.travel else { return };
        let (dx, dy) = (t.target.0 - t.pos.0, t.target.1 - t.pos.1);
        let dist = (dx * dx + dy * dy).sqrt();
        let step = TRAVEL_SPEED * dt;
        if dist <= step.max(3.0) {
            // Arrived: park there; a click leg launches its level;
            // a flown frontier leg completes the dotted route.
            let t = self.travel.take().unwrap();
            self.parked = t.target;
            self.pending_launch = t.launch;
            if t.on_route {
                self.frontier_drawn = true;
            }
            return;
        }
        t.pos.0 += dx / dist * step;
        t.pos.1 += dy / dist * step;
        // Follow the carpet with the viewport when it drifts out.
        let _ = save;
        let margin = 60.0;
        let vx = (t.pos.0 - self.scroll.0).clamp(margin, VIEW_W - margin);
        let vy = (t.pos.1 - self.scroll.1).clamp(margin, VIEW_H - margin);
        self.scroll.0 = t.pos.0 - vx;
        self.scroll.1 = t.pos.1 - vy;
        self.clamp();
    }

    /// A click-travel has landed: the level to launch, once.
    pub fn take_launch(&mut self) -> Option<u32> {
        self.pending_launch.take()
    }

    /// Sample ids fired since the last drain.
    pub fn take_sounds(&mut self) -> Vec<u8> {
        std::mem::take(&mut self.sounds)
    }

    /// The pending-level briefing to play (speech row, segment 0),
    /// once per visit.
    pub fn take_narrative(&mut self) -> Option<u32> {
        self.pending_narrative.take()
    }

    /// Pan the viewport (move keys), retail-viewport units.
    pub fn pan(&mut self, dx: f32, dy: f32) {
        if self.travel.is_some() {
            return; // the travel leg owns the camera
        }
        self.scroll.0 += dx;
        self.scroll.1 += dy;
        self.clamp();
    }

    /// Retail pointer edge-scroll (MI:3132-75): the confined cursor
    /// sitting on the exact screen-edge pixel scrolls the map that
    /// way, X and Y independent (corner = diagonal), with the retail
    /// ramp — `shift_step += 4` per moving frame, capped 24 px/frame,
    /// reset the moment no edge is touched. Modeled at the ~70 Hz
    /// retail frame clock; suspended while the carpet flies (the leg
    /// owns the camera, MI:3133).
    pub fn edge_scroll(&mut self, dir: (f32, f32), dt: f32) {
        const RETAIL_FPS: f32 = 70.0;
        if dir == (0.0, 0.0) || self.travel.is_some() || self.dialog.is_some() {
            self.edge_step = 0.0;
            return;
        }
        self.edge_step = (self.edge_step + 4.0 * RETAIL_FPS * dt).min(24.0);
        self.scroll.0 += dir.0 * self.edge_step * RETAIL_FPS * dt;
        self.scroll.1 += dir.1 * self.edge_step * RETAIL_FPS * dt;
        self.clamp();
    }

    fn clamp(&mut self) {
        self.scroll.0 = self.scroll.0.clamp(0.0, (MAP_W as f32) - VIEW_W);
        self.scroll.1 = self.scroll.1.clamp(0.0, (MAP_H as f32) - VIEW_H);
    }

    /// The clickable portals for the current campaign record.
    fn portals(save: &Mc2Save) -> Vec<Portal> {
        let mut out = Vec::new();
        let completed = save.levels_completed as usize;
        for (i, p) in MC2_MAIN_PORTALS.iter().enumerate() {
            let state = match i.cmp(&completed) {
                std::cmp::Ordering::Less => PortalState::Flag,
                std::cmp::Ordering::Equal => PortalState::Next,
                std::cmp::Ordering::Greater => continue, // still hidden
            };
            out.push(Portal {
                level: i as u32,
                pos: (p.pos.0 as f32, p.pos.1 as f32),
                state,
            });
        }
        for (i, &(_, level, pos)) in MC2_SECRETS.iter().enumerate() {
            let state = match save.secrets[i].activated {
                1 => PortalState::SecretDone,
                2 => PortalState::SecretRevealed,
                _ => continue, // hidden
            };
            out.push(Portal {
                level: level as u32,
                pos: (pos.0 as f32, pos.1 as f32),
                state,
            });
        }
        out
    }

    /// Resolve one portal's current sprite through the pop-in
    /// machine (starts it on first sight where due). None = not
    /// drawn yet (materialization pending behind a travel leg).
    fn portal_sprite(&mut self, p: &Portal) -> Option<usize> {
        let frame = (self.anim * ANIM_FPS) as usize;
        let (pops, idle): (bool, usize) = match p.state {
            PortalState::Flag => return Some(37 + frame % 7),
            PortalState::SecretDone => return Some(305 + frame % 7),
            PortalState::Next => (true, 33),
            PortalState::SecretRevealed => (true, 270),
        };
        debug_assert!(pops);
        match self.pop.get(&p.level) {
            None => {
                self.pop
                    .insert(p.level, Pop::Popping { started: self.anim });
                self.sounds.push(SND_PORTAL_OPEN);
                Some(70)
            }
            Some(Pop::Popping { started }) => {
                let f = 70 + ((self.anim - started) * ANIM_FPS) as usize;
                if f > 83 {
                    self.pop.insert(p.level, Pop::Done);
                    Some(idle)
                } else {
                    Some(f)
                }
            }
            Some(Pop::Done) => Some(idle + frame % 3),
        }
    }

    // ----------------------------------------------------------------
    // Frontend overlay: border frame, corner buttons, parchment
    // dialogs, FONT1 text (docs/traces/mc2-campaign-save-menu.md,
    // "Map border overlay" recon).

    /// A screen-space sprite (640×480 coordinates, ignores the map
    /// scroll — retail draws the overlay after the map blit).
    fn screen_sprite(&self, id: usize, pos: (f32, f32), scale: f32) -> Option<UiQuad> {
        let (sx, sy, w, h) = self.rects.get(id).copied().flatten()?;
        Some(UiQuad {
            rect: [pos.0 * scale, pos.1 * scale, w * scale, h * scale],
            uv: [sx, sy, w, h],
            tint: [1.0, 1.0, 1.0, 1.0],
        })
    }

    /// FONT1 text at a 640-space position (glyph id = char + 1,
    /// advance = glyph width — retail sub_6F940; glyphs are white
    /// masks, tinted here).
    fn text_quads(&self, s: &str, x: f32, y: f32, color: [f32; 4], scale: f32) -> Vec<UiQuad> {
        let mut out = Vec::new();
        let mut cx = x;
        for c in s.chars() {
            let id = (c as usize).wrapping_add(1);
            let Some((sx, sy, w, h)) = self.font.get(id).copied().flatten() else {
                cx += 4.0; // unknown glyph advances a space
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

    /// Word-wrap into a pixel width (retail sub_7FCB0 wraps the
    /// description between its x bounds).
    fn wrap_text(&self, s: &str, width: f32) -> Vec<String> {
        let mut lines = Vec::new();
        let mut line = String::new();
        for word in s.split_whitespace() {
            let cand = if line.is_empty() {
                word.to_string()
            } else {
                format!("{line} {word}")
            };
            if self.text_width(&cand) > width && !line.is_empty() {
                lines.push(std::mem::take(&mut line));
                line = word.to_string();
            } else {
                line = cand;
            }
        }
        if !line.is_empty() {
            lines.push(line);
        }
        lines
    }

    /// Shadowed text (readability over the map art — the retail
    /// bordered draw's role).
    fn shadowed_text(
        &self,
        s: &str,
        x: f32,
        y: f32,
        color: [f32; 4],
        scale: f32,
        quads: &mut Vec<UiQuad>,
    ) {
        quads.extend(self.text_quads(s, x + 1.0, y + 1.0, [0.0, 0.0, 0.0, 0.9], scale));
        quads.extend(self.text_quads(s, x, y, color, scale));
    }

    /// The corner button under a 640-space point (hit-box = the grey
    /// sprite's own dims, MI:2408-9).
    fn button_hit(&self, mx: f32, my: f32) -> Option<MapButton> {
        for (btn, pos, grey, _) in MAP_BUTTONS {
            let Some((_, _, w, h)) = self.rects.get(grey).copied().flatten() else {
                continue;
            };
            if mx >= pos.0 && mx < pos.0 + w && my >= pos.1 && my < pos.1 + h {
                return Some(btn);
            }
        }
        None
    }

    /// A save-label edit field is accepting keystrokes.
    pub fn dialog_editing(&self) -> bool {
        self.dialog.as_ref().is_some_and(|d| d.edit.is_some())
    }

    /// Open a parchment dialog. `slots` = (label, occupied) per save
    /// slot, scanned by the app (retail probes SAVE%d.GAM on entry,
    /// "Empty" for the rest).
    pub fn open_dialog(&mut self, kind: DialogKind, slots: Vec<(String, bool)>) {
        // Retail str_26 anchors: Save (29,60) h 200, Load (510,60)
        // h 200, New Game confirm (37,348) h 60.
        let (anchor, height) = match kind {
            DialogKind::Save => ((29.0, 60.0), 200.0),
            DialogKind::Load => ((510.0, 60.0), 200.0),
            DialogKind::NewGame => ((37.0, 348.0), 60.0),
        };
        self.dialog = Some(Dialog {
            kind,
            anchor,
            height,
            open: 0.0,
            slots,
            selected: None,
            edit: None,
        });
    }

    /// The committed frontend action, once.
    pub fn take_action(&mut self) -> Option<MapAction> {
        self.pending_action.take()
    }

    /// Esc on the map: an open dialog closes; otherwise back to the
    /// menu (retail NewGameDraw returns 2, MI:3430-31).
    pub fn escape(&mut self) {
        if self.dialog.is_some() {
            self.dialog = None;
        } else {
            self.pending_action = Some(MapAction::ExitToMenu);
        }
    }

    /// A keystroke for the save-label editor (retail sub_7F6A0:
    /// space/0-9/letters, max 15).
    pub fn dialog_char(&mut self, c: char) {
        if let Some(d) = &mut self.dialog
            && let Some(edit) = &mut d.edit
            && (c == ' ' || c.is_ascii_alphanumeric())
            && edit.len() < 15
        {
            edit.push(c);
        }
    }

    pub fn dialog_backspace(&mut self) {
        if let Some(d) = &mut self.dialog
            && let Some(edit) = &mut d.edit
        {
            edit.pop();
        }
    }

    /// Enter closes the edit field, committing the label into the
    /// slot row (the actual save happens on OK — retail law).
    pub fn dialog_enter(&mut self) {
        if let Some(d) = &mut self.dialog
            && let Some(edit) = d.edit.take()
            && let Some(k) = d.selected
            && let Some(slot) = d.slots.get_mut(k)
        {
            slot.0 = edit;
        }
    }

    /// Route a 640-space click through the open dialog. Returns the
    /// quads-space handled flag.
    fn dialog_click(&mut self, mx: f32, my: f32) -> bool {
        let Some(d) = &mut self.dialog else {
            return false;
        };
        if d.open < d.height {
            return true; // still opening — swallow
        }
        let (x1, y1) = d.anchor;
        // Slot rows (Save/Load): (x1+20, y1+32+16k), hit 90×16
        // (retail :2661-66 — rows are 1-based off the title line).
        if !matches!(d.kind, DialogKind::NewGame) {
            for k in 0..d.slots.len() {
                let ry = y1 + 32.0 + 16.0 * k as f32;
                if mx >= x1 + 10.0 && mx < x1 + 10.0 + 92.0 && my >= ry && my < ry + 16.0 {
                    let occupied = d.slots[k].1;
                    if matches!(d.kind, DialogKind::Load) && !occupied {
                        return true; // only occupied slots load
                    }
                    d.selected = Some(k);
                    if matches!(d.kind, DialogKind::Save) {
                        // Select-to-edit (retail copies the label
                        // into the edit buffer; a fresh slot starts
                        // empty).
                        d.edit = Some(if occupied {
                            d.slots[k].0.clone()
                        } else {
                            String::new()
                        });
                    }
                    self.sounds.push(SND_CLICK);
                    return true;
                }
            }
        }
        let (ok_r, ca_r) = dialog_button_rects(d.anchor, d.height, DIALOG_W);
        let ok = in_rect(mx, my, ok_r);
        let cancel = in_rect(mx, my, ca_r);
        if ok {
            self.sounds.push(SND_CLICK);
            let action = match d.kind {
                DialogKind::NewGame => Some(MapAction::NewGame),
                DialogKind::Save => {
                    let label = d
                        .edit
                        .take()
                        .or_else(|| d.selected.map(|k| d.slots[k].0.clone()));
                    d.selected.map(|k| MapAction::SaveTo {
                        slot: k,
                        label: label.unwrap_or_default(),
                    })
                }
                DialogKind::Load => d
                    .selected
                    .filter(|&k| d.slots[k].1)
                    .map(MapAction::LoadFrom),
            };
            if let Some(a) = action {
                self.pending_action = Some(a);
                self.dialog = None;
            }
            return true;
        }
        if cancel {
            self.sounds.push(SND_CLICK);
            self.dialog = None;
            return true;
        }
        true // the open dialog swallows map clicks (retail law)
    }

    /// The overlay's quads: border frame, corner buttons (gold on
    /// hover), the pending level's description, the dialog.
    fn overlay_quads(&mut self, save: &Mc2Save, scale: f32, cursor_640: (f32, f32)) -> Vec<UiQuad> {
        let mut quads = Vec::new();
        // The ornate frame (art only, retail sub_85CC3 every frame).
        if let Some((sx, sy, w, h)) = self.border_rect {
            quads.push(UiQuad {
                rect: [0.0, 0.0, w * scale, h * scale],
                uv: [sx, sy, w, h],
                tint: [1.0, 1.0, 1.0, 1.0],
            });
        }
        // Corner buttons: grey idle, gold under the cursor.
        let hover = self.button_hit(cursor_640.0, cursor_640.1);
        for (btn, pos, grey, gold) in MAP_BUTTONS {
            let id = if hover == Some(btn) { gold } else { grey };
            if let Some(q) = self.screen_sprite(id, pos, scale) {
                quads.push(q);
            }
        }
        // The pending level's description text (retail
        // PresentLevelDescription: strings[23+level] at x 130 w 380,
        // y 280 when the portal sits in the top half of the map,
        // else 60; suppressed with the narrative while the level's
        // secret is revealed — the `narrated` latch already encodes
        // that law).
        if self.narrated && !self.desc_dismissed && self.dialog.is_none() && self.travel.is_none() {
            let level = save.levels_completed as usize;
            if let Some(text) = self.strings.get(23 + level) {
                let portal_y = MC2_MAIN_PORTALS.get(level).map_or(0, |p| p.pos.1);
                let y0 = if portal_y < 478 { 280.0 } else { 60.0 };
                let lh = 9.0;
                for (i, line) in self.wrap_text(text, 372.0).iter().enumerate() {
                    self.shadowed_text(
                        line,
                        134.0,
                        y0 + i as f32 * lh,
                        [1.0, 1.0, 1.0, 1.0],
                        scale,
                        &mut quads,
                    );
                }
            }
        }
        // The parchment scroll dialog — the retail composition
        // (DrawScrollDialog2_7B660 MI:5488-5688): ONE unrolled
        // scroll. The roller-bar sprite (254) draws at the TOP and
        // again at the animated BOTTOM edge; between them a solid
        // parchment fill (palette (0x2A,0x24,0x1D)) with a vertical
        // edge line each side ((0x25,0x1F,0x19)); the title sits in
        // dim ink UNDER the top roller; OK at x1+15 and Cancel
        // right-aligned to x1+barW-12, both resting on the bottom
        // roller. Slot rows start at y1+32 (retail y1+16*(k+1),
        // k = 1-based).
        if let Some(d) = &self.dialog {
            let (x1, y1) = d.anchor;
            let title_id = match d.kind {
                DialogKind::Save => 422,
                DialogKind::Load => 421,
                DialogKind::NewGame => 467,
            };
            let (bar_w, bar_h) = self
                .rects
                .get(254)
                .copied()
                .flatten()
                .map_or((DIALOG_W, 12.0), |(_, _, w, h)| (w, h));
            let parchment = [168.0 / 255.0, 144.0 / 255.0, 116.0 / 255.0, 1.0];
            let edge = [148.0 / 255.0, 124.0 / 255.0, 100.0 / 255.0, 1.0];
            let solid = |r: [f32; 4], tint: [f32; 4]| UiQuad {
                rect: [r[0] * scale, r[1] * scale, r[2] * scale, r[3] * scale],
                uv: [0.0; 4],
                tint,
            };
            if d.open > 0.0 {
                let top = y1 + bar_h - 2.0;
                quads.push(solid([x1 + 10.0, top, bar_w - 22.0, d.open], parchment));
                quads.push(solid([x1 + 10.0, top, 1.0, d.open], edge));
                quads.push(solid([x1 + bar_w - 12.0, top, 1.0, d.open], edge));
            }
            if let Some(q) = self.screen_sprite(254, (x1, y1), scale) {
                quads.push(q);
            }
            if let Some(q) = self.screen_sprite(254, (x1, y1 + d.open), scale) {
                quads.push(q);
            }
            let ink = [88.0 / 255.0, 64.0 / 255.0, 36.0 / 255.0, 1.0];
            let white = [1.0, 1.0, 1.0, 1.0];
            // Title once the scroll has opened past a line height
            // (retail letterHeight+10 gate).
            if d.open > 17.0
                && let Some(title) = self.strings.get(title_id)
            {
                let tx = x1 + 10.0 + (bar_w - 22.0 - self.text_width(title)) / 2.0;
                quads.extend(self.text_quads(title, tx, y1 + bar_h + 2.0, ink, scale));
            }
            if d.open >= d.height {
                if !matches!(d.kind, DialogKind::NewGame) {
                    let caret_on = (self.anim * 4.0) as u32 % 2 == 0;
                    for (k, (label, _occupied)) in d.slots.iter().enumerate() {
                        let ry = y1 + 32.0 + 16.0 * k as f32;
                        let selected = d.selected == Some(k);
                        let shown = if selected && let Some(e) = &d.edit {
                            let mut s = format!("{}. {e}", k + 1);
                            if caret_on {
                                s.push('_');
                            }
                            s
                        } else {
                            format!("{}. {label}", k + 1)
                        };
                        let color = if selected { white } else { ink };
                        quads.extend(self.text_quads(&shown, x1 + 20.0, ry, color, scale));
                    }
                }
                let (mx, my) = cursor_640;
                let (ok_r, ca_r) = dialog_button_rects(d.anchor, d.height, bar_w);
                let ok_h = in_rect(mx, my, ok_r);
                let ca_h = in_rect(mx, my, ca_r);
                if let Some(q) =
                    self.screen_sprite(if ok_h { 258 } else { 257 }, (ok_r.0, ok_r.1), scale)
                {
                    quads.push(q);
                }
                if let Some(q) =
                    self.screen_sprite(if ca_h { 256 } else { 255 }, (ca_r.0, ca_r.1), scale)
                {
                    quads.push(q);
                }
            }
        }
        quads
    }

    /// This frame's quads: background crop, ambient dressing, trail
    /// dots, portals, the carpet, the cursor.
    pub fn quads(&mut self, save: &Mc2Save, size: (f32, f32), cursor: (f32, f32)) -> Vec<UiQuad> {
        let scale = (size.0 / VIEW_W).min(size.1 / VIEW_H);
        let mut quads = Vec::new();
        quads.push(UiQuad {
            rect: [0.0, 0.0, VIEW_W * scale, VIEW_H * scale],
            uv: [self.scroll.0, self.scroll.1, VIEW_W, VIEW_H],
            tint: [1.0, 1.0, 1.0, 1.0],
        });
        // Ambient set dressing (the 85/86 rows vanish at the finale,
        // MI:2786).
        let finale = save.levels_completed >= 25;
        for a in &AMBIENTS {
            if finale && (a.first == 85 || a.first == 86) {
                continue;
            }
            let count = (a.last - a.first).max(1) as f32;
            let id = if a.burst {
                let period = a.delay + count / ANIM_FPS;
                let phase = self.anim % period;
                if phase < a.delay {
                    continue; // waiting bursts are invisible
                }
                a.first as usize + (((phase - a.delay) * ANIM_FPS) as usize).min(count as usize - 1)
            } else {
                // The authored start frame phase-offsets the loops
                // (the meteors fall out of sync).
                let offset = (a.start - a.first) as usize;
                a.first as usize + ((self.anim * ANIM_FPS) as usize + offset) % count as usize
            };
            if let Some(q) = self.sprite(id, a.pos, scale) {
                quads.push(q);
            }
        }
        // The dotted route (sprite 139): the FIXED main-line path.
        // Segments between completed portals always draw; the
        // frontier segment draws per `frontier_drawn`; a frontier
        // leg in flight draws its dots up to the carpet.
        if let Some((sx, sy, w, h)) = self.rects.get(139).copied().flatten() {
            let mut dot = |x: f32, y: f32, quads: &mut Vec<UiQuad>| {
                let vx = x - w / 2.0 - self.scroll.0;
                let vy = y - h / 2.0 - self.scroll.1;
                if vx + w < 0.0 || vy + h < 0.0 || vx > VIEW_W || vy > VIEW_H {
                    return;
                }
                quads.push(UiQuad {
                    rect: [vx * scale, vy * scale, w * scale, h * scale],
                    uv: [sx, sy, w, h],
                    tint: [1.0, 1.0, 1.0, 1.0],
                });
            };
            let dots_along =
                |a: (f32, f32),
                 b: (f32, f32),
                 quads: &mut Vec<UiQuad>,
                 dot: &mut dyn FnMut(f32, f32, &mut Vec<UiQuad>)| {
                    let (dx, dy) = (b.0 - a.0, b.1 - a.1);
                    let len = (dx * dx + dy * dy).sqrt();
                    let n = (len / 12.0) as usize;
                    for k in 1..n {
                        let t = k as f32 / n as f32;
                        dot(a.0 + dx * t, a.1 + dy * t, quads);
                    }
                };
            // Segment j connects portal j → j+1. Connections between
            // completed portals always draw; the frontier connection
            // (into the pending portal) per the flag; finale done =
            // the whole route (retail MapMenuPortalsDraw_81760).
            let c = save.levels_completed as usize;
            let end = if c >= 25 {
                24
            } else if self.frontier_drawn {
                c.min(24)
            } else {
                c.saturating_sub(1)
            };
            for j in 0..end {
                dots_along(
                    self.portal_center(j),
                    self.portal_center(j + 1),
                    &mut quads,
                    &mut dot,
                );
            }
            // The in-flight frontier leg draws itself.
            if let Some(t) = &self.travel
                && t.on_route
            {
                dots_along(self.parked, t.pos, &mut quads, &mut dot);
            }
        }
        for p in Self::portals(save) {
            if let Some(id) = self.portal_sprite(&p)
                && let Some(q) = self.sprite(id, p.pos, scale)
            {
                quads.push(q);
            }
        }
        // The carpet: travelling along a leg (8 heading families of
        // 4 frames, sprites 1-32 — heading mapping assumes the
        // engine's 0-north clockwise convention, VERIFY in playtest),
        // or PARKED on the just-played level's portal (retail rests
        // it between legs; default family 13, MI:956-958). Hidden
        // only before the campaign's first launch.
        let carpet = if let Some(t) = &self.travel {
            let (dx, dy) = (t.target.0 - t.pos.0, t.target.1 - t.pos.1);
            let units =
                (dx.atan2(-dy).rem_euclid(std::f32::consts::TAU)) / std::f32::consts::TAU * 2048.0;
            const FAMILY: [usize; 8] = [17, 5, 9, 13, 1, 21, 25, 29];
            Some((FAMILY[(((units + 128.0) / 256.0) as usize) % 8], t.pos))
        } else if save.levels_completed > 0 || self.parked != (0.0, 0.0) {
            Some((13, self.parked))
        } else {
            None
        };
        if let Some((family, pos)) = carpet {
            let id = family + ((self.anim * CARPET_FPS) as usize) % 4;
            if let Some((sx, sy, w, h)) = self.rects.get(id).copied().flatten() {
                let vx = pos.0 - w / 2.0 - self.scroll.0;
                let vy = pos.1 - h / 2.0 - self.scroll.1;
                quads.push(UiQuad {
                    rect: [vx * scale, vy * scale, w * scale, h * scale],
                    uv: [sx, sy, w, h],
                    tint: [1.0, 1.0, 1.0, 1.0],
                });
            }
        }
        // The frontend overlay: border frame, corner buttons,
        // description text, parchment dialog — over the map, under
        // the cursor (retail draw order).
        let cursor_640 = (cursor.0 / scale, cursor.1 / scale);
        quads.extend(self.overlay_quads(save, scale, cursor_640));
        // Cursor: the map screen's own bank sprite 239 (MI:986).
        if let Some((sx, sy, w, h)) = self.rects.get(239).copied().flatten() {
            quads.push(UiQuad {
                rect: [cursor.0, cursor.1, w * scale, h * scale],
                uv: [sx, sy, w, h],
                tint: [1.0, 1.0, 1.0, 1.0],
            });
        }
        quads
    }

    /// A map-bank sprite at a map position (top-left anchored, like
    /// retail's blits), or None while scrolled off.
    fn sprite(&self, id: usize, pos: (f32, f32), scale: f32) -> Option<UiQuad> {
        let (sx, sy, w, h) = self.rects.get(id).copied().flatten()?;
        let x = pos.0 - self.scroll.0;
        let y = pos.1 - self.scroll.1;
        if x + w < 0.0 || y + h < 0.0 || x > VIEW_W || y > VIEW_H {
            return None;
        }
        Some(UiQuad {
            rect: [x * scale, y * scale, w * scale, h * scale],
            uv: [sx, sy, w, h],
            tint: [1.0, 1.0, 1.0, 1.0],
        })
    }

    /// A corner-button click, drained by the app (Save/Load need the
    /// slot scan before the dialog opens; Exit/NewGame route
    /// directly).
    pub fn take_button(&mut self) -> Option<MapButton> {
        self.pending_button.take()
    }

    /// Handle a click: the frontend overlay first (dialog, corner
    /// buttons — retail suppresses map clicks while a dialog pumps,
    /// MI:2387-88), then a hit portal starts a travel leg that
    /// launches on arrival (MI:3330-3405). Returns true when the
    /// click hit anything.
    pub fn click(&mut self, save: &Mc2Save, size: (f32, f32), cursor: (f32, f32)) -> bool {
        let scale = (size.0 / VIEW_W).min(size.1 / VIEW_H);
        let (sx, sy) = (cursor.0 / scale, cursor.1 / scale);
        if self.dialog.is_some() {
            return self.dialog_click(sx, sy);
        }
        if let Some(btn) = self.button_hit(sx, sy) {
            self.sounds.push(SND_CLICK);
            self.pending_button = Some(btn);
            return true;
        }
        if self.travel.is_some() {
            return false; // one leg at a time
        }
        let mx = sx + self.scroll.0;
        let my = sy + self.scroll.1;
        let hit = MC2_PORTAL_HIT as f32;
        for p in Self::portals(save) {
            if mx >= p.pos.0 && mx < p.pos.0 + hit && my >= p.pos.1 && my < p.pos.1 + hit {
                // The leg departs from wherever the carpet rests.
                let from = self.parked;
                let target = (p.pos.0 + hit / 2.0, p.pos.1 + hit / 2.0);
                println!(
                    "world map: {} level {}",
                    if matches!(p.state, PortalState::Flag | PortalState::SecretDone) {
                        "replaying"
                    } else {
                        "flying to"
                    },
                    p.level
                );
                // The travel sample plays with the click, but only
                // when the flyer actually goes somewhere (retail
                // gates it on leg length, MI:3786; deliberate:
                // immediate, not retail's late start).
                if (target.0 - from.0).abs() > 8.0 || (target.1 - from.1).abs() > 8.0 {
                    self.sounds.push(SND_TRAVEL);
                }
                // The canonical frontier leg — the only flight that
                // draws route dots: departing the LAST COMPLETED
                // portal for the PENDING one. Off-route trips (any
                // other origin or destination) draw nothing; the
                // segment then appears on the next map entry.
                let completed = save.levels_completed;
                let on_route = completed > 0 && p.level == completed && {
                    let canon = self.portal_center(completed as usize - 1);
                    (from.0 - canon.0).abs() < 1.0 && (from.1 - canon.1).abs() < 1.0
                };
                self.travel = Some(Travel {
                    pos: from,
                    target,
                    launch: Some(p.level),
                    on_route,
                });
                // A committed portal trip retires the description
                // text for this visit.
                self.desc_dismissed = true;
                return true;
            }
        }
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn save_with(completed: u32) -> Mc2Save {
        Mc2Save {
            levels_completed: completed,
            ..Default::default()
        }
    }

    fn bare() -> WorldMap {
        WorldMap {
            atlas: Vec::new(),
            atlas_w: 1,
            atlas_h: 1,
            rects: Vec::new(),
            border_rect: None,
            font: Vec::new(),
            strings: Vec::new(),
            dialog: None,
            pending_action: None,
            pending_button: None,
            desc_dismissed: false,
            scroll: (0.0, 0.0),
            anim: 0.0,
            pop: HashMap::new(),
            travel: None,
            pending_launch: None,
            sounds: Vec::new(),
            narrated: false,
            pending_narrative: None,
            parked: (0.0, 0.0),
            last_seen_completed: None,
            frontier_drawn: false,
            edge_step: 0.0,
        }
    }

    #[test]
    fn portal_states_follow_the_record() {
        let mut save = save_with(3);
        save.secrets[0].activated = 2; // secret 30 revealed
        let portals = WorldMap::portals(&save);
        // Mains 0-2 completed + next (3) + the revealed secret.
        assert_eq!(portals.len(), 5);
        assert!(matches!(portals[0].state, PortalState::Flag));
        assert!(matches!(portals[3].state, PortalState::Next));
        assert_eq!(portals[3].level, 3);
        assert_eq!(portals[4].level, 30);
        assert!(matches!(portals[4].state, PortalState::SecretRevealed));
    }

    #[test]
    fn click_starts_travel_then_launches_on_arrival() {
        let mut wm = bare();
        let save = save_with(0);
        wm.scroll = (400.0, 800.0);
        let size = (1280.0, 960.0); // 2× scale
        // Portal 0 sits at (420, 820): cursor over it in window px.
        wm.set_parked(3); // resting on portal 3's flag
        assert!(wm.click(&save, size, (50.0, 50.0)), "portal 0 hit");
        assert!(wm.travel.is_some(), "click starts the carpet leg");
        assert_eq!(
            wm.take_sounds(),
            vec![SND_TRAVEL],
            "the travel sample plays with the click on a real leg"
        );
        // Run the leg to arrival — the launch arms and the carpet
        // parks on the clicked portal.
        for _ in 0..600 {
            wm.tick(1.0 / 60.0, &save);
        }
        assert_eq!(wm.take_launch(), Some(0));
        assert!(wm.travel.is_none());
        // A click out in the sea hits nothing.
        assert!(!wm.click(&save, size, (600.0, 100.0)));
    }

    #[test]
    fn new_portal_pops_on_first_sight_with_sound() {
        let mut wm = bare();
        let p = Portal {
            level: 4,
            pos: (450.0, 652.0),
            state: PortalState::Next,
        };
        // First sight starts the pop-in (sound 41, frame 70);
        // subsequent frames advance it; past 83 it idles open.
        assert_eq!(wm.portal_sprite(&p), Some(70));
        assert!(wm.take_sounds().contains(&SND_PORTAL_OPEN));
        wm.anim = 2.0; // > 14 frames / 12.5 fps
        assert_eq!(wm.portal_sprite(&p), Some(33), "settles into the open idle");
        assert!(matches!(wm.pop.get(&4), Some(Pop::Done)));
        // A revisit never replays the pop (session latch).
        wm.anim = 2.1;
        let idle = wm.portal_sprite(&p).unwrap();
        assert!((33..=35).contains(&idle));
        assert!(wm.take_sounds().is_empty());
    }

    #[test]
    fn route_law_frontier_segment_waits_for_the_carpet() {
        let mut wm = bare();
        // A load (first sight this session): the full trail.
        wm.enter_visit(&save_with(5));
        assert!(wm.frontier_drawn, "load shows the whole route");
        // The frontier advances (a completion led here): the new
        // segment waits.
        wm.enter_visit(&save_with(6));
        assert!(!wm.frontier_drawn, "fresh segment blank until flown");
        // Flying the canonical leg (parked on the last flag, click
        // the pending portal) draws it on arrival.
        wm.set_parked(5);
        let save = save_with(6);
        let size = (1280.0, 960.0);
        // Portal 6 at (763,652): scroll so it's under the cursor.
        wm.scroll = (600.0, 500.0);
        let cur = ((763.0 + 10.0 - 600.0) * 2.0, (652.0 + 10.0 - 500.0) * 2.0);
        assert!(wm.click(&save, size, cur), "pending portal hit");
        assert!(wm.travel.as_ref().unwrap().on_route);
        for _ in 0..900 {
            wm.tick(1.0 / 60.0, &save);
        }
        assert!(wm.frontier_drawn, "flown frontier leg stamps its segment");
        // Re-entry at the same frontier keeps it stamped.
        wm.enter_visit(&save);
        assert!(wm.frontier_drawn);
        // An off-route trip never arms route drawing: park elsewhere,
        // click the pending portal — leg flies, no route dots.
        let mut wm2 = bare();
        wm2.enter_visit(&save_with(5));
        wm2.enter_visit(&save_with(6));
        wm2.set_parked(2); // replayed an old level — off the canon origin
        wm2.scroll = (600.0, 500.0);
        assert!(wm2.click(&save, size, cur));
        assert!(!wm2.travel.as_ref().unwrap().on_route);
        for _ in 0..900 {
            wm2.tick(1.0 / 60.0, &save);
        }
        assert!(!wm2.frontier_drawn, "off-route flight draws nothing");
    }

    #[test]
    fn parked_carpet_follows_the_played_level() {
        let mut wm = bare();
        wm.set_parked(30); // a failed secret parks on ITS portal
        assert_eq!(wm.parked, (287.0 + 20.0, 656.0 + 20.0));
        wm.set_parked(0);
        assert_eq!(wm.parked, (430.0, 830.0)); // fallback flag half = 10
    }

    #[test]
    fn narrative_fires_after_pop_unless_secret_pending() {
        let mut wm = bare();
        let save = save_with(4);
        // Materialize portal 4 (no travel in a bare map), run the
        // pop-in through, then the briefing fires once.
        wm.pop.insert(4, Pop::Popping { started: 0.0 });
        for _ in 0..300 {
            wm.tick(1.0 / 60.0, &save);
            // portal_sprite would advance the pop; emulate its
            // completion the way the draw path does.
            if wm.anim > 1.5 {
                wm.pop.insert(4, Pop::Done);
            }
        }
        assert_eq!(wm.take_narrative(), Some(4));
        assert_eq!(wm.take_narrative(), None, "once per visit");
        // A revealed secret attached to the pending level suppresses
        // it (retail MI:3583-89) — until the next visit after it
        // resolves.
        let mut wm = bare();
        let mut save = save_with(4);
        save.secrets[0].activated = 2; // secret 30, parent 4
        wm.pop.insert(4, Pop::Done);
        for _ in 0..60 {
            wm.tick(1.0 / 60.0, &save);
        }
        assert_eq!(wm.take_narrative(), None, "pending secret suppresses");
    }

    /// Give a bare map the corner-button + dialog sprites so the
    /// overlay hit paths run (dims match the real bank).
    fn with_overlay_rects(mut wm: WorldMap) -> WorldMap {
        wm.rects = vec![None; 260];
        for id in 246..=253 {
            wm.rects[id] = Some((0.0, 0.0, 60.0, 54.0));
        }
        wm.rects[254] = Some((0.0, 0.0, 114.0, 12.0));
        for id in 255..=258 {
            wm.rects[id] = Some((0.0, 0.0, 40.0, 29.0));
        }
        wm
    }

    #[test]
    fn corner_buttons_swallow_map_clicks_and_report() {
        let mut wm = with_overlay_rects(bare());
        let save = save_with(3);
        let size = (1280.0, 960.0); // 2× scale
        // Top-left = the Save button (0,0)+60×54 in 640-space.
        assert!(wm.click(&save, size, (10.0, 10.0)));
        assert_eq!(wm.take_button(), Some(MapButton::Save));
        assert!(wm.take_sounds().contains(&SND_CLICK));
        // Top-right = Load (581,0).
        assert!(wm.click(&save, size, (600.0 * 2.0, 10.0)));
        assert_eq!(wm.take_button(), Some(MapButton::Load));
        // Bottom corners: New Game left, Exit right.
        assert!(wm.click(&save, size, (10.0, 440.0 * 2.0)));
        assert_eq!(wm.take_button(), Some(MapButton::NewGame));
        assert!(wm.click(&save, size, (600.0 * 2.0, 440.0 * 2.0)));
        assert_eq!(wm.take_button(), Some(MapButton::Exit));
    }

    #[test]
    fn save_dialog_select_edit_commit() {
        let mut wm = with_overlay_rects(bare());
        let save = save_with(3);
        let size = (1280.0, 960.0);
        wm.open_dialog(
            DialogKind::Save,
            vec![("OLD".into(), true), ("Empty".into(), false)],
        );
        // Still opening: clicks swallowed, nothing selected.
        assert!(wm.click(&save, size, (100.0, 200.0)));
        assert!(wm.dialog.as_ref().unwrap().selected.is_none());
        // Open it fully, then click slot row 2 (fresh slot): edit
        // field starts EMPTY.
        for _ in 0..40 {
            wm.tick(1.0 / 60.0, &save);
        }
        // Row k=1 at (29+20, 60+32+16) → 640-space (55, 110) → ×2.
        assert!(wm.click(&save, size, (110.0, 224.0)));
        assert_eq!(wm.dialog.as_ref().unwrap().selected, Some(1));
        assert!(wm.dialog_editing());
        // Type a label: filter passes alphanumerics/space, max 15.
        for c in "MY GAME!#".chars() {
            wm.dialog_char(c);
        }
        assert_eq!(wm.dialog.as_ref().unwrap().edit.as_deref(), Some("MY GAME"));
        wm.dialog_backspace();
        wm.dialog_enter(); // commit into the row
        assert_eq!(wm.dialog.as_ref().unwrap().slots[1].0, "MY GAM");
        // OK ((29+15, 60+200-28) → (44,232) 640-space): the action.
        assert!(wm.click(&save, size, (46.0 * 2.0, 240.0 * 2.0)));
        assert_eq!(
            wm.take_action(),
            Some(MapAction::SaveTo {
                slot: 1,
                label: "MY GAM".into()
            })
        );
        assert!(wm.dialog.is_none());
    }

    #[test]
    fn load_dialog_only_occupied_slots() {
        let mut wm = with_overlay_rects(bare());
        let save = save_with(3);
        let size = (1280.0, 960.0);
        wm.open_dialog(
            DialogKind::Load,
            vec![("GAME".into(), true), ("Empty".into(), false)],
        );
        for _ in 0..40 {
            wm.tick(1.0 / 60.0, &save);
        }
        // Row 2 (empty): not selectable. Load anchor x=510: row 1 at
        // (530, 108) 640-space.
        assert!(wm.click(&save, size, (1070.0, 224.0)));
        assert!(wm.dialog.as_ref().unwrap().selected.is_none());
        // Row 1 (occupied, y 92..108) selects; OK commits.
        assert!(wm.click(&save, size, (1070.0, 192.0)));
        assert_eq!(wm.dialog.as_ref().unwrap().selected, Some(0));
        assert!(wm.click(&save, size, ((510.0 + 16.0) * 2.0, 240.0 * 2.0)));
        assert_eq!(wm.take_action(), Some(MapAction::LoadFrom(0)));
    }

    #[test]
    fn escape_closes_dialog_then_exits_to_menu() {
        let mut wm = with_overlay_rects(bare());
        wm.open_dialog(DialogKind::NewGame, Vec::new());
        wm.escape();
        assert!(wm.dialog.is_none());
        assert_eq!(wm.take_action(), None);
        wm.escape();
        assert_eq!(wm.take_action(), Some(MapAction::ExitToMenu));
    }

    #[test]
    fn session_reset_forgets_presentation_state() {
        let mut wm = bare();
        wm.pop.insert(4, Pop::Done);
        wm.set_parked(3);
        wm.enter_visit(&save_with(5));
        wm.session_reset();
        assert!(wm.pop.is_empty());
        assert_eq!(wm.parked, (0.0, 0.0));
        // Next entry reads as a fresh session: full trail.
        wm.enter_visit(&save_with(5));
        assert!(wm.frontier_drawn);
    }

    #[test]
    fn burst_ambients_hide_while_waiting() {
        // Row 8 (655,58): 4 s delay, 6 frames at 12.5 fps.
        let a = &AMBIENTS[7];
        assert!(a.burst);
        let period = a.delay + (a.last - a.first).max(1) as f32 / ANIM_FPS;
        assert!(period > a.delay);
        // Phase inside the delay window → invisible (the quads loop
        // `continue`s); phase after it indexes frames 86..91.
        let phase = a.delay + 0.2;
        let id = a.first as usize
            + (((phase - a.delay) * ANIM_FPS) as usize).min((a.last - a.first) as usize - 1);
        assert!((86..=91).contains(&id));
    }
}
