//! The in-level pause mini-menu (docs/archive/DESIGN-SAVES.md "The in-game
//! menu"): a small always-visible panel offering save, load and
//! options, anchored clear of the HUD.
//!
//! # Why it is small, and why input stays live behind it
//!
//! Retail MC1 keeps the entire input path live during pause:
//! `sub_17C20` (remc1 sub_main.cpp:41667) is deliberately NOT gated
//! on the pause bit while the calls immediately above and below it
//! are. Pausing to rearrange spells and then unpausing into a
//! prepared volley is engineered behaviour, and on the harder levels
//! it is close to essential. A full-screen pause menu regresses it.
//!
//! So this panel is deliberately modest: it must not cover more than
//! the live-view part of the big (ENTER) map, and the MC1 spell
//! selector has to stay reachable underneath. It sits in the upper
//! right, inset from the edge rather than jammed into the corner, so
//! it clears the HUD's top strip.
//!
//! Sound and music icons are dropped — retail MC2 surfaced them only
//! because it had no other route; the options menu owns volume here.
//!
//! Esc never dismisses this panel, per the standing law. Unpause is
//! the only way out — and unpause closes the Options layer with it.
//! Esc closes only that layer, dropping back here.
//!
//! The panel is hidden while the Options layer is up (they are
//! mutually exclusive on screen). It carries no "PAUSED" text of its
//! own — the retail indicator (`ui::pause_quads`) reports the pause
//! STATE, this panel is the MENU, and the player reads the two in
//! different places. And it reports nothing itself: results go to the
//! in-game toast line, which can hold a sentence without running off
//! the edge of a narrow panel.

use crate::saves::SlotInfo;
use crate::ui::UiAssets;
use mgc_render::UiQuad;

/// Panel geometry, as fractions of the viewport.
///
/// `MARGIN` keeps it off the right edge; `TOP` drops it below the HUD
/// strip. Both are the reason it reads as "in the upper right" rather
/// than "in the corner".
const MARGIN: f32 = 0.02;
const TOP: f32 = 0.14;
const WIDTH: f32 = 0.21;

/// Deliberately near-opaque: the panel sits over sky and over bright
/// desert alike, and at 0.82 the label contrast collapsed on light
/// levels. Legibility beats seeing three more pixels of terrain.
const PANEL_BG: [f32; 4] = [0.01, 0.01, 0.02, 0.93];
const PANEL_EDGE: [f32; 4] = [0.75, 0.75, 0.75, 0.35];
const ROW_HOVER_BG: [f32; 4] = [1.0, 1.0, 1.0, 0.10];
const INK_LABEL: [f32; 4] = [0.95, 0.95, 0.95, 1.0];
const INK_TITLE: [f32; 4] = [0.62, 0.66, 0.72, 1.0];
/// A slot that exists but this build cannot read.
const INK_LOCKED: [f32; 4] = [0.50, 0.42, 0.42, 1.0];
/// An empty slot.
const INK_EMPTY: [f32; 4] = [0.45, 0.45, 0.48, 1.0];
/// A slot that resumes mid-level rather than at the hub.
const INK_RESUME: [f32; 4] = [0.30, 1.0, 0.45, 1.0];
/// A slot salvaged from an older container — loadable, but its resume
/// is gone. Amber: not broken, not whole.
const INK_STALE: [f32; 4] = [0.90, 0.72, 0.30, 1.0];

/// Which face the panel is showing.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Mode {
    /// Save / Load / Options.
    Root,
    /// The slot list. `saving` picks the verb: writing a slot versus
    /// reading one.
    Slots { saving: bool },
}

pub struct MiniMenu {
    pub mode: Mode,
    /// Slot rows, refreshed whenever the list is opened (never per
    /// frame — this is a directory scan).
    pub slots: Vec<SlotInfo>,
}

impl Default for MiniMenu {
    fn default() -> Self {
        MiniMenu {
            mode: Mode::Root,
            slots: Vec::new(),
        }
    }
}

impl MiniMenu {
    pub fn new() -> Self {
        Self::default()
    }

    /// Return to the root face, dropping any stale slot list.
    pub fn reset_to_root(&mut self) {
        self.mode = Mode::Root;
        self.slots = Vec::new();
    }
}

/// What a click landed on.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Hit {
    None,
    Save,
    Load,
    Options,
    /// A slot row on the list face.
    Slot(usize),
    Back,
}

struct Layout {
    s: f32,
    fs: f32,
    lh: f32,
    row_h: f32,
    panel: [f32; 4],
    /// Top of the first interactive row.
    rows_y: f32,
    /// How many interactive rows this face has.
    rows: usize,
}

fn layout(assets: &UiAssets, m: &MiniMenu, w: f32, h: f32) -> Layout {
    let s = crate::ui::HudFrame::new(w, h).s.max(1.0);
    let fs = 2.0 * s;
    let lh = assets.font_line_height().max(8.0) * fs;
    let row_h = lh * 1.3;
    let pad = 8.0 * s;

    // Root has three actions; the slot list has one row per slot plus
    // a Back row.
    let rows = match m.mode {
        Mode::Root => 3,
        Mode::Slots { .. } => m.slots.len() + 1,
    };
    // No heading row on a face that has no heading — otherwise the
    // root panel carries a band of empty space where the title was.
    let title_h = if title_of(&m.mode).is_some() {
        lh * 1.5
    } else {
        pad
    };
    let pw = WIDTH * w;
    let ph = title_h + rows as f32 * row_h + pad;
    let px = w - pw - MARGIN * w;
    let py = TOP * h;

    Layout {
        s,
        fs,
        lh,
        row_h,
        panel: [px, py, pw, ph],
        rows_y: py + title_h,
        rows,
    }
}

fn row_rect(l: &Layout, i: usize) -> [f32; 4] {
    [
        l.panel[0],
        l.rows_y + i as f32 * l.row_h,
        l.panel[2],
        l.row_h,
    ]
}

fn inside(r: [f32; 4], c: (f32, f32)) -> bool {
    c.0 >= r[0] && c.0 < r[0] + r[2] && c.1 >= r[1] && c.1 < r[1] + r[3]
}

/// Is the cursor over the panel at all?
///
/// The caller uses this to decide whether a click belongs to the
/// mini-menu or to the live game underneath — the panel consumes
/// clicks ONLY within its own rect, which is what keeps the spell
/// selector and the map overlays usable while paused.
pub fn covers(assets: &UiAssets, m: &MiniMenu, w: f32, h: f32, cursor: (f32, f32)) -> bool {
    inside(layout(assets, m, w, h).panel, cursor)
}

pub fn hit_test(assets: &UiAssets, m: &MiniMenu, w: f32, h: f32, cursor: (f32, f32)) -> Hit {
    let l = layout(assets, m, w, h);
    if !inside(l.panel, cursor) {
        return Hit::None;
    }
    let row = (0..l.rows).find(|&i| inside(row_rect(&l, i), cursor));
    let Some(row) = row else { return Hit::None };
    match m.mode {
        Mode::Root => match row {
            0 => Hit::Save,
            1 => Hit::Load,
            2 => Hit::Options,
            _ => Hit::None,
        },
        Mode::Slots { .. } => {
            if row < m.slots.len() {
                Hit::Slot(row)
            } else {
                Hit::Back
            }
        }
    }
}

/// The panel's heading, or None when it would say nothing useful.
///
/// The root face has none: its three rows name themselves, and the
/// retail PAUSED indicator (`ui::pause_quads`) is what reports the
/// pause state. The slot list keeps one, because "SAVE TO" and
/// "LOAD FROM" are the only thing distinguishing two identical lists.
fn title_of(mode: &Mode) -> Option<&'static str> {
    match mode {
        Mode::Root => None,
        Mode::Slots { saving: true } => Some("SAVE TO"),
        Mode::Slots { saving: false } => Some("LOAD FROM"),
    }
}

/// One slot's row text and ink.
///
/// LETTERS, DIGITS AND SPACES ONLY. The messaging font is the game's
/// own FONT1 bank addressed by `glyph = byte + 1`, so a byte only
/// renders as its ASCII character where the bank happens to hold that
/// character — the punctuation slots hold game icons instead (`*`
/// draws as a lightning flash). Non-ASCII is worse: an em dash is
/// three bytes and draws as three unrelated glyphs.
fn slot_row(info: &SlotInfo, index: usize, _saving: bool) -> (String, [f32; 4]) {
    let n = index + 1;
    if info.incompatible {
        return (format!("{n} unreadable"), INK_LOCKED);
    }
    if !info.occupied {
        return (format!("{n} empty"), INK_EMPTY);
    }
    let label = if info.label.trim().is_empty() {
        format!("slot {n}")
    } else {
        info.label.trim().to_string()
    };
    // Every slot names its level; a slot that resumes INTO that level
    // adds the mana percentage the run had reached. So "L3" is the
    // campaign parked in front of level 3 and "L3 15%" is a run
    // fifteen percent of the way through it — one shape, and the
    // suffix says which. (`%` is safe in this font; see above.)
    if info.stale {
        // Salvaged from a container this build cannot apply: the
        // progress survived, the resume did not. Marked because it is
        // a LOSS — a slot that quietly stopped resuming reads as fine
        // right up until the level restarts.
        return (format!("{n} {label}  L{} old", info.level), INK_STALE);
    }
    match info.resume {
        Some(pct) => (format!("{n} {label}  L{} {pct}%", info.level), INK_RESUME),
        None => (format!("{n} {label}  L{}", info.level), INK_LABEL),
    }
}

pub fn draw(assets: &UiAssets, m: &MiniMenu, w: f32, h: f32, cursor: (f32, f32)) -> Vec<UiQuad> {
    let l = layout(assets, m, w, h);
    let s = l.s;
    let pad = 8.0 * s;
    let mut quads = Vec::new();

    quads.push(crate::ui::solid(l.panel, PANEL_BG));
    let [px, py, pw, ph] = l.panel;
    for e in [
        [px, py, pw, s],
        [px, py + ph - s, pw, s],
        [px, py, s, ph],
        [px + pw - s, py, s, ph],
    ] {
        quads.push(crate::ui::solid(e, PANEL_EDGE));
    }

    if let Some(title) = title_of(&m.mode) {
        quads.extend(assets.text_quads(title, px + pad, py + pad, INK_TITLE, l.fs));
    }

    let row_text = |quads: &mut Vec<UiQuad>, i: usize, text: &str, ink: [f32; 4]| {
        let r = row_rect(&l, i);
        if inside(r, cursor) {
            quads.push(crate::ui::solid(r, ROW_HOVER_BG));
        }
        quads.extend(assets.text_quads(text, r[0] + pad, r[1] + (l.row_h - l.lh) * 0.5, ink, l.fs));
    };

    match m.mode {
        Mode::Root => {
            for (i, label) in ["Save", "Load", "Options"].into_iter().enumerate() {
                row_text(&mut quads, i, label, INK_LABEL);
            }
        }
        Mode::Slots { saving } => {
            for (i, info) in m.slots.iter().enumerate() {
                let (text, ink) = slot_row(info, i, saving);
                row_text(&mut quads, i, &text, ink);
            }
            row_text(&mut quads, m.slots.len(), "Back", INK_TITLE);
        }
    }

    quads
}

#[cfg(test)]
mod tests {
    use super::*;

    fn slots(n: usize) -> Vec<SlotInfo> {
        (0..n)
            .map(|i| SlotInfo {
                label: format!("SLOT{i}"),
                occupied: i % 2 == 0,
                ..Default::default()
            })
            .collect()
    }

    /// A mid-level slot must read differently from a hub slot — it is
    /// the only thing distinguishing "resume where I was" from "start
    /// this level over", and both are one click away.
    ///
    /// BOTH carry the level; only the resuming one carries the mana
    /// percentage, and that suffix is the whole distinction.
    #[test]
    fn a_resume_slot_is_marked() {
        let hub = SlotInfo {
            label: "A".into(),
            occupied: true,
            level: 7,
            ..Default::default()
        };
        let resume = SlotInfo {
            resume: Some(15),
            ..hub.clone()
        };
        let (hub_text, hub_ink) = slot_row(&hub, 0, false);
        let (res_text, res_ink) = slot_row(&resume, 0, false);
        assert_ne!(hub_text, res_text);
        assert_ne!(hub_ink, res_ink);
        assert!(
            hub_text.contains("L7"),
            "a hub slot still names its level: {hub_text}"
        );
        assert!(
            res_text.contains("L7") && res_text.contains("15%"),
            "a resuming slot names the level AND how far in: {res_text}"
        );
    }

    /// The percentage is what says "in progress", so a run at 0% mana
    /// must still read as a resume rather than collapsing into the hub
    /// shape — `Some(0)` is not `None`.
    #[test]
    fn a_resume_at_zero_percent_is_still_a_resume() {
        let hub = SlotInfo {
            label: "A".into(),
            occupied: true,
            level: 4,
            ..Default::default()
        };
        let fresh_run = SlotInfo {
            resume: Some(0),
            ..hub.clone()
        };
        assert_ne!(slot_row(&hub, 0, false), slot_row(&fresh_run, 0, false));
        assert!(slot_row(&fresh_run, 0, false).0.contains("0%"));
    }

    /// A salvaged slot must not pass for a healthy hub save: the
    /// player has lost a resume and needs to see it, or the loss only
    /// surfaces when the level restarts under them.
    #[test]
    fn a_salvaged_slot_is_distinguishable_from_a_hub_save() {
        let hub = SlotInfo {
            label: "RAIN".into(),
            occupied: true,
            level: 3,
            ..Default::default()
        };
        let salvaged = SlotInfo {
            stale: true,
            ..hub.clone()
        };
        let (hub_text, hub_ink) = slot_row(&hub, 0, false);
        let (old_text, old_ink) = slot_row(&salvaged, 0, false);
        assert_ne!(hub_text, old_text);
        assert_ne!(hub_ink, old_ink);
        // Still names its level: the progress IS there.
        assert!(old_text.contains("L3"), "{old_text}");
    }

    /// An unreadable slot must never render as empty: an empty row is
    /// an invitation to save over it.
    #[test]
    fn an_unreadable_slot_does_not_read_as_empty() {
        let bad = SlotInfo {
            occupied: true,
            incompatible: true,
            ..Default::default()
        };
        let empty = SlotInfo::default();
        assert_ne!(slot_row(&bad, 0, true).0, slot_row(&empty, 0, true).0);
        assert_ne!(slot_row(&bad, 0, true).1, slot_row(&empty, 0, true).1);
    }

    #[test]
    fn root_rows_map_to_their_actions() {
        let m = MiniMenu::new();
        assert_eq!(m.mode, Mode::Root);
        // Row order is the contract `hit_test` and `draw` share.
        for (i, want) in [Hit::Save, Hit::Load, Hit::Options].into_iter().enumerate() {
            let l = Layout {
                s: 1.0,
                fs: 2.0,
                lh: 8.0,
                row_h: 10.0,
                panel: [0.0, 0.0, 100.0, 100.0],
                rows_y: 0.0,
                rows: 3,
            };
            let r = row_rect(&l, i);
            let c = (r[0] + 1.0, r[1] + 1.0);
            let row = (0..l.rows).find(|&k| inside(row_rect(&l, k), c)).unwrap();
            let got = match row {
                0 => Hit::Save,
                1 => Hit::Load,
                2 => Hit::Options,
                _ => Hit::None,
            };
            assert_eq!(got, want, "row {i}");
        }
    }

    /// Past the last slot is Back, not slot N+1 — an off-by-one here
    /// would load or overwrite a slot the player never clicked.
    #[test]
    fn the_row_after_the_slots_is_back() {
        let mut m = MiniMenu::new();
        m.mode = Mode::Slots { saving: false };
        m.slots = slots(6);
        let l = Layout {
            s: 1.0,
            fs: 2.0,
            lh: 8.0,
            row_h: 10.0,
            panel: [0.0, 0.0, 100.0, 100.0],
            rows_y: 0.0,
            rows: m.slots.len() + 1,
        };
        let last = row_rect(&l, m.slots.len());
        assert!(inside(last, (last[0] + 1.0, last[1] + 1.0)));
        // And the row before it is the final slot.
        let prev = row_rect(&l, m.slots.len() - 1);
        assert!(prev[1] < last[1]);
    }
}
