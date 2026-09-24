//! The MC1/HW end-of-level performance screen — retail PPERF
//! (`sub_4E5B0_4E8F0`, remc1 :59905-60152, menu state 5 after the
//! level FMV).
//!
//! Retail law reproduced:
//! - PPERF.DAT over PPERF.PAL, text in SFONT2 (`dword_96894 =
//!   SFont2Tab + 6`: glyph id = char − 31) at the hi-res coordinates,
//!   the title at (40, 8). SFONT2 is a 20 px face, so the screen
//!   composes at 640×400 (PPERF doubled).
//! - The lines appear one by one, 60 ticks of the 120 Hz clock (½ s)
//!   apart — the title with the first.
//! - Any key or a left click dismisses.
//!
//! Deliberate differences (player rulings 2026-09-24):
//! - The numbers are the ENHANCED ones of
//!   [`mgc_sim::engine::stats::LevelStats`]: "Creatures Killed" counts
//!   every death by anyone, and "Spells found" is counted per spell
//!   (retail MC1's census reads bytes as dwords).
//! - The layout. The title reads "Level N: Name". Retail showed the bare
//!   `dword_999B8` entry "N. Name". "Overall performance" is gone.
//!   Every row shows its absolute beside the percentage, "N (P %)",
//!   right-aligned to retail's x = 600. Accuracy reads as hits and
//!   misses, and leaves out the possession lob. The kill breakdown
//!   (you / rivals / nature / still alive) sits as bullet sub-points
//!   under "Creatures Killed". "Mana claimed" carries in castle / in
//!   dwellings as bullets, and "Mana unclaimed" closes its block as a
//!   plain line (player ruling 2026-09-25: unclaimed is not a part of
//!   what was claimed).
//! - Percentages carry one decimal, rounded half up, with strict ends
//!   (100.0 only when complete, 0.0 only when none), and each
//!   breakdown sums to exactly 100.0 (player ruling 2026-09-25).
//! - Mana leaves out the wizard's intrinsic 1000, which retail's
//!   census counts in the world total but no one can collect, so a
//!   fully collected level reads 100.0. Time Taken sits at the
//!   bottom, with the title's margin; retail spaced six rows 40 px
//!   apart from y = 80, right-aligned to x = 600.
//!
//! - No timeout: the screen waits for the key (retail leaves by
//!   itself after 1320 ticks, 11 s).
//! - `render.preference.stats_screen` (`--no-stats`) skips it.
//!
//! - Every line chimes as it appears, as retail's rows do
//!   (`sub_65F10(0, 3)` — sample 3 of the SNDS bank 13 PPERF loads
//!   on entry, `sub_5D070_5D580(0xD)`). The bullet sub-lines chime
//!   too.
//!
//! Not yet here: a CLASSIC switch (the option-preset work).

use std::path::Path;

use mgc_render::UiQuad;
use mgc_sim::engine::stats::{LevelStats, fmt10};

use crate::frontend_mc1::Bank;

const W: usize = 640;
const H: usize = 400;
const BG_W: usize = 320;
const BG_H: usize = 200;

/// The retail row cadence: 60 ticks of the 120 Hz clock.
const ROW_STEP_S: f32 = 0.5;

/// The row chime: sample 3 of SNDS bank 13 (see the module docs).
pub const CHIME: (u32, u32) = (13, 3);

/// What the performance screen shows — captured at the won edge,
/// because the session (and its World and ETEXT) is torn down before
/// the screen runs.
#[derive(Clone, Debug)]
pub struct StatsCard {
    pub stats: LevelStats,
    /// Level play time in sim ticks (24 Hz).
    pub ticks: u64,
    /// 0-based level number (the world-name index).
    pub level: usize,
    /// Hidden Worlds (its own world names).
    pub hidden: bool,
    /// ETEXT 66 / 67 / 68 / 59 / 79 — Creatures Killed, Accuracy,
    /// Spells found, Mana, Time Taken. (Retail's 69, "Overall
    /// performance", is dropped — player ruling 2026-09-24.)
    pub labels: [String; 5],
}

/// One line of the screen.
struct Line {
    label: String,
    value: String,
    /// A bullet sub-point (a breakdown row).
    sub: bool,
    y: i32,
}

/// The value column's right edge (retail's x = 600), the sub-point indent, and the
/// vertical rhythm: the title keeps retail's (40, 8), Time Taken sits
/// the same margin off the bottom, the other main rows spread evenly
/// between, each breakdown packed under its row.
const VALUE_RIGHT: i32 = 600;
const SUB_X: i32 = 72;
const BULLET_X: i32 = 58;
const TITLE_Y: i32 = 8;
const GLYPH_H: i32 = 20;
const FIRST_Y: i32 = 56;
const SUB_STEP: i32 = 24;
/// The kill breakdown's four sub-points, the mana breakdown's two,
/// and "Mana unclaimed" packed under them.
const SUBS: i32 = 7;
const LAST_Y: i32 = H as i32 - TITLE_Y - GLYPH_H;
/// Killed, Accuracy, Spells, Mana, Time: four equal main steps; the
/// sub-points take the rest.
const ROW_STEP: i32 = (LAST_Y - (FIRST_Y + SUBS * SUB_STEP)) / 4;

impl StatsCard {
    /// The label ETEXT ids, in row order.
    pub const LABEL_IDS: [usize; 5] = [66, 67, 68, 59, 79];
    const FALLBACK: [&'static str; 5] = [
        "Creatures Killed",
        "Accuracy",
        "Spells found",
        "Mana",
        "Time Taken",
    ];

    pub fn new(stats: LevelStats, ticks: u64, level: usize, hidden: bool, etext: &[String]) -> Self {
        let labels = std::array::from_fn(|i| {
            etext
                .get(Self::LABEL_IDS[i])
                .filter(|s| !s.trim().is_empty())
                .cloned()
                .unwrap_or_else(|| Self::FALLBACK[i].to_string())
        });
        StatsCard {
            stats,
            ticks,
            level,
            hidden,
            labels,
        }
    }

    /// "Level N: Name" (retail titles the screen with the world name
    /// alone, `dword_999B8[level]` = "N. Name"); "Level N" without a
    /// baked name table.
    fn title(&self, names: Option<&[String]>) -> String {
        let n = self.level + 1;
        match names.and_then(|v| v.get(self.level)) {
            Some(name) => format!("Level {n}: {name}"),
            None => format!("Level {n}"),
        }
    }

    /// Every line in reveal order: each row reads "N (P %)" — the
    /// absolute beside the percentage — and Accuracy splits its N into
    /// hits and misses.
    fn lines(&self) -> Vec<Line> {
        let s = &self.stats;
        let np = |n: u32, p: u32| format!("{n} ({} %)", fmt10(p));
        let secs = self.ticks / mgc_sim::TICK_RATE_HZ as u64;
        // Possession lobs are not shots here (player ruling
        // 2026-09-24) — see `Gen::mc1_note_shot`.
        let (hits, shots) = (s.offensive_hits, s.offensive_shots);
        let misses = shots.saturating_sub(hits);
        let k = s.kill_split10();
        let m = s.mana_split10();
        let mana = s.mana_rows();
        let claimed = format!("{} claimed", self.labels[3]);
        let unclaimed = format!("{} unclaimed", self.labels[3]);
        // (main-row label index, or a packed line: (label, bullet?)).
        let rows: Vec<(Result<usize, (&str, bool)>, String)> = vec![
            (Ok(0), np(s.deaths_total(), s.cleared_pct10())),
            (Err(("by you", true)), np(s.deaths_player(), k[0])),
            (Err(("by rivals", true)), np(s.deaths_rivals(), k[1])),
            (Err(("by nature", true)), np(s.deaths_environment(), k[2])),
            (Err(("still alive", true)), np(s.alive, k[3])),
            (Ok(1), format!("{hits} hits, {misses} misses ({} %)", fmt10(s.offensive_accuracy_pct10()))),
            (Ok(2), np(s.spells_found_fixed, s.spells_fixed_pct10())),
            (Ok(3), np(mana[0] + mana[1], s.mana_pct10())),
            (Err(("in castle", true)), np(mana[0], m[0])),
            (Err(("in dwellings", true)), np(mana[1], m[1])),
            (Err((&unclaimed, false)), np(mana[2], m[2])),
            (Ok(4), format!("{}h {:02}m {:02}s", secs / 3600, secs / 60 % 60, secs % 60)),
        ];
        let mut out = Vec::with_capacity(rows.len());
        let mut y = 0;
        for (label, value) in rows {
            // Time Taken pinned to the bottom margin (absorbs the
            // step's rounding).
            y = match label {
                Ok(0) => FIRST_Y,
                Ok(4) => LAST_Y,
                Ok(_) => y + ROW_STEP,
                Err(_) => y + SUB_STEP,
            };
            let (label, sub) = match label {
                Ok(3) => (claimed.clone(), false),
                Ok(k) => (self.labels[k].clone(), false),
                Err((l, bullet)) => (l.to_string(), bullet),
            };
            out.push(Line { label, value, sub, y });
        }
        out
    }
}

pub struct Mc1Stats {
    bg: Vec<u8>,
    pal: [u8; 768],
    font: Bank,
    title: String,
    lines: Vec<Line>,
    clock: f32,
    /// Lines already chimed for.
    chimed: usize,
    dismissed: bool,
    screen: Vec<u8>,
}

impl Mc1Stats {
    pub fn load(dir: &Path, card: StatsCard) -> Result<Self, String> {
        let read = |name: &str| -> Result<Vec<u8>, String> {
            std::fs::read(dir.join(name))
                .map_err(|e| format!("{}: {e} (rebake — the stats screen is new)", dir.join(name).display()))
        };
        let bg = read("pperf-bg.bin")?;
        let pal_v = read("pperf-pal.bin")?;
        if bg.len() != BG_W * BG_H || pal_v.len() != 768 {
            return Err("mc1-ui pperf members have unexpected sizes".into());
        }
        let pal: [u8; 768] = pal_v.try_into().unwrap();
        let font = Bank::load(dir, "sfont2.bin", "sfont2.json")?;
        // `level-names.json` (bake epoch 25): {"mc1": [50], "mc1hw": [50]}.
        let names: Option<Vec<String>> = std::fs::read(dir.join("level-names.json"))
            .ok()
            .and_then(|b| serde_json::from_slice::<serde_json::Value>(&b).ok())
            .and_then(|v| {
                let key = if card.hidden { "mc1hw" } else { "mc1" };
                serde_json::from_value(v.get(key)?.clone()).ok()
            });
        Ok(Mc1Stats {
            bg,
            pal,
            font,
            title: card.title(names.as_deref()),
            lines: card.lines(),
            clock: 0.0,
            chimed: 0,
            dismissed: false,
            screen: vec![0; W * H],
        })
    }

    pub fn tick(&mut self, dt: f32) {
        self.clock += dt;
    }

    /// Retail's cadence, one line per step, the title with the first.
    fn shown(&self) -> usize {
        ((self.clock / ROW_STEP_S) as usize).min(self.lines.len())
    }

    /// True once per newly revealed line: the caller plays [`CHIME`].
    /// Retail latches one flag per row, so a slow frame that reveals
    /// two lines still chimes once for each.
    pub fn take_chime(&mut self) -> bool {
        let due = self.chimed < self.shown();
        self.chimed += due as usize;
        due
    }

    /// Any key or a left click (retail clears the latches and leaves).
    pub fn dismiss(&mut self) {
        self.dismissed = true;
    }

    pub fn done(&self) -> bool {
        self.dismissed
    }

    fn text(&self, s: &str, x: i32, y: i32, buf: &mut [u8]) {
        let mut cx = x;
        for c in s.chars() {
            let id = (c as usize).saturating_sub(31);
            if c != ' ' {
                self.font.blit_in(id, cx, y, None, buf, (W, H));
            }
            cx += self.font.width_of(id) as i32;
        }
    }

    fn text_width(&self, s: &str) -> i32 {
        s.chars()
            .map(|c| self.font.width_of((c as usize).saturating_sub(31)) as i32)
            .sum()
    }

    pub fn frame(&mut self, size: (f32, f32)) -> (Vec<u8>, Vec<UiQuad>) {
        let (scale, ox, oy) = crate::ui::letterbox(size, W as f32, H as f32);
        let mut buf = std::mem::take(&mut self.screen);
        for y in 0..H {
            let src = &self.bg[(y / 2) * BG_W..(y / 2 + 1) * BG_W];
            let row = &mut buf[y * W..(y + 1) * W];
            for x in 0..W {
                row[x] = src[x / 2];
            }
        }
        let shown = self.shown();
        if shown >= 1 {
            self.text(&self.title, 40, TITLE_Y, &mut buf);
        }
        for line in &self.lines[..shown] {
            if line.sub {
                // The bullet: SFONT2's full stop lifted to mid-height,
                // so it wears the face's own colours.
                self.text(".", BULLET_X, line.y - 7, &mut buf);
                self.text(&line.label, SUB_X, line.y, &mut buf);
            } else {
                self.text(&line.label, 40, line.y, &mut buf);
            }
            let w = self.text_width(&line.value);
            self.text(&line.value, VALUE_RIGHT - w, line.y, &mut buf);
        }
        let mut rgba = vec![0u8; W * H * 4];
        for (i, &idx) in buf.iter().enumerate() {
            let idx = idx as usize;
            let o = i * 4;
            rgba[o] = self.pal[idx * 3] << 2;
            rgba[o + 1] = self.pal[idx * 3 + 1] << 2;
            rgba[o + 2] = self.pal[idx * 3 + 2] << 2;
            rgba[o + 3] = 255;
        }
        self.screen = buf;
        let quads = vec![UiQuad {
            rect: [ox, oy, W as f32 * scale, H as f32 * scale],
            uv: [0.0, 0.0, W as f32, H as f32],
            tint: [1.0, 1.0, 1.0, 1.0],
        }];
        (rgba, quads)
    }
}
