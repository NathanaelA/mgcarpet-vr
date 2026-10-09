//! The launcher: the first screen of a plain `mgcarpet` start (no
//! `--campaign`, `--level`, replay or headless instrument). Three
//! game cards, the option presets beneath them, Exit and Start.
//!
//! It must stand up with NO baked data. That is the whole point: the
//! game used to bake (or refuse to start) before it had a window. So
//! everything the screen needs ships with the binary: the DejaVu
//! Serif Bold face (`assets/launcher/`, Bitstream Vera licence) drawn
//! through `ab_glyph`, and plain shapes for the rest. The cards show
//! retail art only when the game's bake is current: the
//! player's own data, never redistributed. An unbaked game gets a
//! crossed-out placeholder and cannot be started. The art is each
//! game's title card: the last frame of its title movie.
//!
//! When the baked tree is missing or stale (a first run, or a build
//! with a newer bake epoch) and the original data can be found, the
//! launcher bakes in the background and the cards light up when it
//! finishes. Pointing the game at data it cannot find is the future
//! loader dialog's job; until then, when NO game is ready and nothing
//! is baking, a modal notice says so and where the installs go — a
//! first run without data must not look like a broken program.
//!
//! The screen composes on the CPU at the window's own resolution (the
//! text stays crisp at any size), from a 960×720 layout scaled to fit
//! and centred, and is re-composed only when something visible
//! changes.
//!
//! The option rows set PRESETS (`settings::PresetGroup`): a group of
//! options written together, Enhanced or Classic, and read back from
//! those options rather than stored. A group already hand-tuned when
//! the launcher opened also offers "Custom", which puts the hand-tuned
//! values back. Changes apply and persist the way the options menu's
//! do. The Display row picks borderless fullscreen or one of the fixed
//! window sizes that fit the monitor; the launcher itself stays a
//! window, and the choice takes effect when the game starts.

use std::path::{Path, PathBuf};
use std::thread::JoinHandle;
use std::time::Instant;

use ab_glyph::{Font, FontRef, PxScale, ScaleFont};
use mgc_render::UiQuad;

use crate::bakecheck::{self, BakeStatus};
use crate::campaign::CampaignId;
use crate::config::{Config, WindowSize};
use crate::{ui, IS_ANDROID};
use crate::settings::{self, Preset, PresetGroup};

const FONT: &[u8] = include_bytes!("../../../assets/launcher/DejaVuSerif-Bold.ttf");

/// The layout's virtual canvas.
const VW: f32 = 960.0;
const VH: f32 = 720.0;

const GAMES: [(CampaignId, &str); 3] = [
    (CampaignId::Mc1, "Magic Carpet"),
    (CampaignId::Mc1Hw, "Magic Carpet: The Hidden Worlds"),
    (CampaignId::Mc2, "Magic Carpet 2: The Netherworlds"),
];

type Rgba = [u8; 4];
const GOLD: Rgba = [232, 196, 110, 255];
const TEXT: Rgba = [222, 216, 204, 255];
const MUTED: Rgba = [138, 132, 150, 255];
const PANEL: Rgba = [30, 26, 46, 235];
const EDGE: Rgba = [88, 78, 116, 255];
const HOVER: Rgba = [172, 152, 214, 255];
const CROSS: Rgba = [176, 44, 44, 230];

/// (x, y, w, h) in the virtual canvas.
type Rect = (f32, f32, f32, f32);

const CARD_W: f32 = 280.0;
const ART_H: f32 = 175.0;
/// Room for a two-line caption: a title with a subtitle ("Magic
/// Carpet 2: The Netherworlds") sets the subtitle on its own line.
const CAPTION_H: f32 = 54.0;
const CARD_Y: f32 = 150.0;
const CARD_GAP: f32 = 35.0;

fn card_rect(i: usize) -> Rect {
    let left = (VW - 3.0 * CARD_W - 2.0 * CARD_GAP) / 2.0;
    (
        left + i as f32 * (CARD_W + CARD_GAP),
        CARD_Y,
        CARD_W,
        ART_H + CAPTION_H,
    )
}

const OPT_Y: [f32; 3] = [428.0, 480.0, 532.0];
const OPT_H: f32 = 40.0;
const OPT_LABEL_RIGHT: f32 = 330.0;
const ARROW_W: f32 = 40.0;
const VALUE_X: f32 = 395.0;
const VALUE_W: f32 = 270.0;

fn arrow_rect(row: usize, right: bool) -> Rect {
    let x = if right {
        VALUE_X + VALUE_W + 5.0
    } else {
        VALUE_X - 5.0 - ARROW_W
    };
    (x, OPT_Y[row], ARROW_W, OPT_H)
}

fn value_rect(row: usize) -> Rect {
    (VALUE_X, OPT_Y[row], VALUE_W, OPT_H)
}

const BUTTON_Y: f32 = 618.0;
const BUTTON_W: f32 = 200.0;
const BUTTON_H: f32 = 58.0;

fn button_rect(start: bool) -> Rect {
    (
        if start { 510.0 } else { 250.0 },
        BUTTON_Y,
        BUTTON_W,
        BUTTON_H,
    )
}

/// The no-games notice: a centred panel with one OK button.
const NOTICE: Rect = (110.0, 100.0, 740.0, 540.0);

fn notice_ok_rect() -> Rect {
    (
        VW / 2.0 - BUTTON_W / 2.0,
        NOTICE.1 + NOTICE.3 - BUTTON_H - 28.0,
        BUTTON_W,
        BUTTON_H,
    )
}

/// What an option row sets.
enum RowKind {
    /// Fullscreen or a window size (`render.preference.fullscreen` /
    /// `window_size`).
    Display,
    /// A preset group.
    Preset(&'static PresetGroup),
    /// A save slot.
    SaveSlot,
    /// A start level.
    StartLevel,
}

/// A Display row choice.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Mode {
    Fullscreen,
    Window(WindowSize),
}

impl Mode {
    fn of(cfg: &Config) -> Mode {
        let p = &cfg.render.preference;
        if p.fullscreen {
            Mode::Fullscreen
        } else {
            Mode::Window(p.window_size)
        }
    }

    fn label(self) -> String {
        match self {
            Mode::Fullscreen => "Fullscreen".into(),
            Mode::Window(w) => {
                let (x, y) = w.dims();
                format!("Window {x} × {y}")
            }
        }
    }
}

#[cfg(target_os = "android")]
const OPTIONS: [(&str, RowKind); 3] = [
    ("Visuals", RowKind::Preset(&settings::VISUALS_PRESET)),
    ("Save Slot", RowKind::SaveSlot),
    ("Start Level", RowKind::StartLevel),
];

#[cfg(not(target_os = "android"))]
const OPTIONS: [(&str, RowKind); 3] = [
    ("Display", RowKind::Display),
    ("Controls", RowKind::Preset(&settings::CONTROLS_PRESET)),
    ("Visuals", RowKind::Preset(&settings::VISUALS_PRESET)),
];

/// A preset row's choice.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Choice {
    Preset(Preset),
    /// The hand-tuned values the launcher opened with.
    Custom,
}

/// What the launcher asks of the app.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    Start(CampaignId),
    Exit,
}

/// Keyboard focus: the card row, an option row, or the button row.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Focus {
    Cards,
    Option(usize),
    Buttons { start: bool },
}

/// Something under the pointer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Hit {
    Card(usize),
    Arrow {
        row: usize,
        right: bool,
    },
    Value(usize),
    Button {
        start: bool,
    },
    /// The no-games notice's OK.
    NoticeOk,
}

/// A card's baked title art, RGB.
struct Art {
    w: usize,
    h: usize,
    rgb: Vec<u8>,
}

/// Everything the composed frame depends on; a change re-composes.
#[derive(Clone, PartialEq)]
struct ViewKey {
    size: (u32, u32),
    selected: Option<usize>,
    focus: Focus,
    hover: Option<Hit>,
    values: [String; 3],
    games: [bool; 3],
    status: String,
    notice: Option<Vec<(bool, String)>>,
}

pub struct Launcher {
    font: FontRef<'static>,
    baked_root: PathBuf,
    status: BakeStatus,
    /// The background bake and when it started.
    bake: Option<(JoinHandle<Result<(), String>>, Instant)>,
    /// A standing message (no data found, a failed bake).
    message: Option<String>,
    art: [Option<Art>; 3],
    selected: Option<usize>,
    focus: Focus,
    hover: Option<Hit>,
    /// The monitor the window is on (physical px), when known: a
    /// window size that does not fit is not offered.
    monitor: Option<(u32, u32)>,
    /// Per row: the config as the launcher found it, kept when that
    /// row's preset group was hand-tuned (its "Custom").
    custom: [Option<Config>; 3],
    /// Option paths changed since the app last looked (registry
    /// paths), to apply and persist.
    changed: Vec<&'static str>,
    pending: Option<Action>,
    last: Option<ViewKey>,
    /// The no-games notice is up (modal until dismissed).
    notice: bool,
    save_slot: usize,
    start_level: usize,
}

impl Launcher {
    /// Judge the baked tree and, when it needs (re)generating and the
    /// original data is found, start baking it in the background.
    pub fn new(baked_root: &Path, cfg: &Config) -> Self {
        let gamedata = cfg.gamedata.as_deref();
        let font = FontRef::try_from_slice(FONT).expect("the launcher font is compiled in");
        let status = bakecheck::status(baked_root);
        let mut l = Launcher {
            font,
            baked_root: baked_root.to_path_buf(),
            status,
            bake: None,
            message: None,
            art: [None, None, None],
            selected: None,
            focus: Focus::Cards,
            hover: None,
            monitor: None,
            custom: std::array::from_fn(|row| match &OPTIONS[row].1 {
                RowKind::Preset(g) if g.current(cfg).is_none() => Some(cfg.clone()),
                _ => None,
            }),
            changed: Vec::new(),
            pending: None,
            last: None,
            notice: false,
            save_slot: 1,
            start_level: 0,
        };
        if let Some(reason) = l.status.tree.clone() {
            match bakecheck::gamedata(gamedata) {
                Some(src) => {
                    println!("launcher: the baked data needs preparing ({reason})");
                    let root = l.baked_root.clone();
                    let handle = std::thread::spawn(move || bakecheck::bake(&src, &root));
                    l.bake = Some((handle, Instant::now()));
                }
                None => {
                    l.message = Some(
                        "No game data found. Place your GOG installs under gamedata/ \
                         (see gamedata/README.md)."
                            .into(),
                    );
                }
            }
        }
        l.refresh();
        l.notice = l.nothing_ready();
        l
    }

    /// No game can start and no bake will change that.
    fn nothing_ready(&self) -> bool {
        self.bake.is_none() && !self.status.games.iter().any(|&g| g)
    }

    /// The notice's text, one paragraph per entry, `(is_path, text)`;
    /// an empty text is a gap.
    fn notice_text(&self) -> Vec<(bool, String)> {
        let para = |t: &str| (false, t.to_string());
        let gap = || (false, String::new());
        let mut out = vec![
            para(
                "mgcarpet plays the original Magic Carpet games from your own \
                 copies, and it did not find any usable game data on this computer.",
            ),
            gap(),
        ];
        if let Some(m) = self
            .message
            .as_deref()
            .filter(|m| m.starts_with("Preparing"))
        {
            out.push(para(m));
            out.push(gap());
        }
        let dir = std::env::current_dir()
            .map(|d| d.join("gamedata").display().to_string())
            .unwrap_or_else(|_| "gamedata".into());
        out.push(para(
            "Copy your GOG installs of Magic Carpet Plus and/or Magic Carpet 2, \
             unchanged, into:",
        ));
        out.push((true, dir));
        out.push(gap());
        out.push(para(
            "gamedata/README.md explains the layout. The game prepares the \
             data by itself the next time it starts.",
        ));
        out
    }

    fn dismiss_notice(&mut self) {
        self.notice = false;
    }

    /// Re-read the art of every ready game and keep the selection on
    /// one of them.
    fn refresh(&mut self) {
        for i in 0..3 {
            self.art[i] = if self.status.games[i] {
                load_art(&self.baked_root, GAMES[i].0)
                    .map_err(|e| eprintln!("note: launcher art for {}: {e}", GAMES[i].1))
                    .ok()
            } else {
                None
            };
        }
        if self.selected.is_none_or(|i| !self.status.games[i]) {
            self.selected = (0..3).find(|&i| self.status.games[i]);
        }
    }

    /// Poll the background bake.
    pub fn tick(&mut self) {
        if self.bake.as_ref().is_some_and(|(h, _)| h.is_finished()) {
            let (handle, t0) = self.bake.take().expect("checked above");
            match handle.join() {
                Ok(Ok(())) => println!(
                    "launcher: game data ready ({:.0} s)",
                    t0.elapsed().as_secs_f32()
                ),
                Ok(Err(e)) => {
                    eprintln!("error: bake: {e}");
                    self.message = Some(format!("Preparing the game data failed: {e}"));
                }
                Err(_) => self.message = Some("Preparing the game data crashed.".into()),
            }
            self.status = bakecheck::status(&self.baked_root);
            self.refresh();
            self.notice = self.nothing_ready();
        }
    }

    /// Forget the last frame, so the next one re-composes (another
    /// screen took the uploaded image's slot).
    pub fn invalidate(&mut self) {
        self.last = None;
    }

    pub fn take_action(&mut self) -> Option<Action> {
        self.pending.take()
    }

    fn start(&mut self) {
        if let Some(i) = self.selected.filter(|&i| self.status.games[i]) {
            self.pending = Some(Action::Start(GAMES[i].0));
        }
    }

    fn step_selection(&mut self, right: bool) {
        let Some(cur) = self.selected else { return };
        let mut i = cur;
        for _ in 0..2 {
            i = if right { (i + 1) % 3 } else { (i + 2) % 3 };
            if self.status.games[i] {
                self.selected = Some(i);
                return;
            }
        }
    }

    /// A preset row's choices, in stepping order, and the current one.
    fn choices(&self, row: usize, g: &PresetGroup, cfg: &Config) -> (Vec<Choice>, usize) {
        let mut list: Vec<Choice> = Preset::ALL.into_iter().map(Choice::Preset).collect();
        if self.custom[row].is_some() {
            list.push(Choice::Custom);
        }
        let cur = match g.current(cfg) {
            Some(p) => Choice::Preset(p),
            None => Choice::Custom,
        };
        // Hand-tuned since the launcher opened can only be the
        // launcher's own "Custom"; otherwise it is on the list.
        let at = list
            .iter()
            .position(|&c| c == cur)
            .unwrap_or(list.len() - 1);
        (list, at)
    }

    pub fn set_monitor(&mut self, monitor: Option<(u32, u32)>) {
        self.monitor = monitor;
    }

    /// The Display row's choices: fullscreen, then each window size
    /// that fits the monitor (and the configured one regardless).
    fn modes(&self, cfg: &Config) -> Vec<Mode> {
        let cur = Mode::of(cfg);
        let mut list = vec![Mode::Fullscreen];
        for w in WindowSize::ALL {
            let (x, y) = w.dims();
            let fits = self.monitor.is_none_or(|(mw, mh)| x <= mw && y <= mh);
            if fits || cur == Mode::Window(w) {
                list.push(Mode::Window(w));
            }
        }
        list
    }

    /// The row's value as shown.
    fn value(&self, row: usize, cfg: &Config) -> String {
        match &OPTIONS[row].1 {
            RowKind::SaveSlot => self.save_slot.to_string(),
            RowKind::StartLevel => self.start_level.to_string(),
            RowKind::Display => Mode::of(cfg).label(),
            RowKind::Preset(g) => match g.current(cfg) {
                Some(p) => p.label().to_string(),
                None => "Custom".to_string(),
            },
        }
    }

    fn step_option(&mut self, row: usize, right: bool, cfg: &mut Config) {
        match &OPTIONS[row].1 {
            RowKind::SaveSlot => {
                let at = self.save_slot;
                let mut next = if right {
                    (at + 1)
                } else {
                    (at - 1)
                };
                if next < 1 {
                    next = 5;
                } else if next > 5 {
                    next = 1;
                }
                self.save_slot = next;
            }
            RowKind::StartLevel => {
                let at = self.start_level;
                let mut next = if right {
                    (at + 1)
                } else {
                    (at - 1)
                };
                if next < 1 {
                    next = 5;
                } else if next > 5 {
                    next = 1;
                }
                self.start_level = next;
            }
            RowKind::Display => {
                let list = self.modes(cfg);
                let n = list.len();
                let at = list.iter().position(|&m| m == Mode::of(cfg)).unwrap_or(0);
                let next = if right {
                    (at + 1) % n
                } else {
                    (at + n - 1) % n
                };
                let p = &mut cfg.render.preference;
                match list[next] {
                    Mode::Fullscreen => p.fullscreen = true,
                    Mode::Window(w) => {
                        p.fullscreen = false;
                        p.window_size = w;
                    }
                }
                self.changed.extend([
                    "render.preference.fullscreen",
                    "render.preference.window_size",
                ]);
            }
            RowKind::Preset(g) => {
                let (list, at) = self.choices(row, g, cfg);
                let n = list.len();
                let next = if right {
                    (at + 1) % n
                } else {
                    (at + n - 1) % n
                };
                match list[next] {
                    Choice::Preset(p) => (g.apply)(cfg, p),
                    Choice::Custom => {
                        if let Some(tuned) = &self.custom[row] {
                            g.copy_members(cfg, tuned);
                        }
                    }
                }
                self.changed.push(g.cfg_path);
            }
        }
    }

    /// The option paths changed since the last call (registry paths).
    pub fn take_changed(&mut self) -> Vec<&'static str> {
        std::mem::take(&mut self.changed)
    }

    /// Esc leaves (or closes the notice).
    pub fn escape(&mut self) {
        if self.notice {
            self.dismiss_notice();
            return;
        }
        self.pending = Some(Action::Exit);
    }

    /// Keyboard navigation: Up/Down (and Tab) walk the rows,
    /// Left/Right act within one, Enter starts (or presses the focused
    /// button).
    pub fn key(&mut self, key: &winit::keyboard::Key, cfg: &mut Config) {
        use winit::keyboard::{Key, NamedKey};
        let Key::Named(k) = key else { return };
        if self.notice {
            if matches!(k, NamedKey::Enter | NamedKey::Escape | NamedKey::Space) {
                self.dismiss_notice();
            }
            return;
        }
        let rows = |f: Focus| match f {
            Focus::Cards => 0,
            Focus::Option(r) => r + 1,
            Focus::Buttons { .. } => 4,
        };
        let at = |r: usize| match r {
            0 => Focus::Cards,
            1..=3 => Focus::Option(r - 1),
            _ => Focus::Buttons { start: true },
        };
        match k {
            NamedKey::ArrowUp => self.focus = at(rows(self.focus).saturating_sub(1)),
            NamedKey::ArrowDown | NamedKey::Tab => self.focus = at((rows(self.focus) + 1).min(4)),
            NamedKey::ArrowLeft | NamedKey::ArrowRight => {
                let right = *k == NamedKey::ArrowRight;
                match self.focus {
                    Focus::Cards => self.step_selection(right),
                    Focus::Option(r) => self.step_option(r, right, cfg),
                    Focus::Buttons { .. } => self.focus = Focus::Buttons { start: right },
                }
            }
            NamedKey::Enter => match self.focus {
                Focus::Buttons { start: false } => self.pending = Some(Action::Exit),
                _ => self.start(),
            },
            NamedKey::Escape => self.escape(),
            _ => {}
        }
    }

    /// A left click at a window position.
    pub fn click(&mut self, size: (f32, f32), cursor: (f32, f32), cfg: &mut Config) {
        if self.notice {
            if notice_hit(size, cursor) {
                self.dismiss_notice();
            }
            return;
        }
        match hit(size, cursor) {
            Some(Hit::Card(i)) if self.status.games[i] => {
                self.selected = Some(i);
                self.focus = Focus::Cards;
            }
            Some(Hit::Arrow { row, right }) => {
                self.step_option(row, right, cfg);
                self.focus = Focus::Option(row);
            }
            Some(Hit::Value(row)) => {
                self.step_option(row, true, cfg);
                self.focus = Focus::Option(row);
            }
            Some(Hit::Button { start: true }) => self.start(),
            Some(Hit::Button { start: false }) => self.pending = Some(Action::Exit),
            _ => {}
        }
    }

    fn status_line(&self) -> String {
        if let Some((_, t0)) = &self.bake {
            let s = t0.elapsed().as_secs();
            return format!(
                "Preparing the game data… {}:{:02}  (the first run also renders the music)",
                s / 60,
                s % 60
            );
        }
        if let Some(m) = &self.message {
            return m.clone();
        }
        if self.selected.is_none() {
            return "No game is ready to play.".into();
        }
        "Enter starts  ·  Esc quits".into()
    }

    /// This frame: a freshly composed window-sized RGBA image when
    /// anything visible changed (`None` = the uploaded one still
    /// stands), and the one quad that shows it.
    pub fn frame(
        &mut self,
        size: (f32, f32),
        cursor: (f32, f32),
        cfg: &Config,
    ) -> (Option<Vec<u8>>, Vec<UiQuad>) {
        let (w, h) = (size.0.max(1.0) as u32, size.1.max(1.0) as u32);
        self.hover = if self.notice {
            notice_hit(size, cursor).then_some(Hit::NoticeOk)
        } else {
            hit(size, cursor)
        };
        let key = ViewKey {
            size: (w, h),
            selected: self.selected,
            focus: self.focus,
            hover: self.hover,
            values: std::array::from_fn(|row| self.value(row, cfg)),
            games: self.status.games,
            status: self.status_line(),
            notice: self.notice.then(|| self.notice_text()),
        };
        let quad = UiQuad {
            rect: [0.0, 0.0, w as f32, h as f32],
            uv: [0.0, 0.0, w as f32, h as f32],
            tint: [1.0; 4],
        };
        let mut quads = vec![quad];
        if IS_ANDROID {
            let s = ui::HudFrame::new(size.0 as f32, size.1 as f32)
                .s
                .max(1.0);

            quads.extend(ui::cursor_quads(cursor.0, cursor.1, s));
        }

        if self.last.as_ref() == Some(&key) {
            return (None, quads);
        }
        let mut c = Canvas::new(w as usize, h as usize);
        self.compose(&mut c, &key);
        self.last = Some(key);
        (Some(c.buf), quads)

    }

    fn compose(&self, c: &mut Canvas, key: &ViewKey) {
        c.background();
        c.text(
            &self.font,
            "Magic Carpet",
            VW / 2.0,
            82.0,
            60.0,
            GOLD,
            Align::Center,
        );
        c.text(
            &self.font,
            "Choose your world",
            VW / 2.0,
            118.0,
            20.0,
            MUTED,
            Align::Center,
        );

        for i in 0..3 {
            self.compose_card(c, key, i);
        }
        c.text(
            &self.font,
            &key.status,
            VW / 2.0,
            406.0,
            17.0,
            MUTED,
            Align::Center,
        );

        for (row, (label, _)) in OPTIONS.iter().enumerate() {
            if IS_ANDROID && row > 0 {
               continue;
            }
            let focused = key.focus == Focus::Option(row);
            let (_, y, _, hh) = value_rect(row);
            let base = y + hh / 2.0 + 7.0;
            c.text(
                &self.font,
                label,
                OPT_LABEL_RIGHT,
                base,
                21.0,
                if focused { GOLD } else { TEXT },
                Align::Right,
            );
            // Bare arrows on the background; the whole rect clicks.
            for right in [false, true] {
                let r = arrow_rect(row, right);
                let hot = key.hover == Some(Hit::Arrow { row, right });
                c.arrow(
                    r,
                    right,
                    if hot {
                        HOVER
                    } else if focused {
                        GOLD
                    } else {
                        TEXT
                    },
                );
            }
            let r = value_rect(row);
            c.fill(r, PANEL);
            let edge = if focused {
                GOLD
            } else if key.hover == Some(Hit::Value(row)) {
                HOVER
            } else {
                EDGE
            };
            c.stroke(r, if focused { 2.0 } else { 1.5 }, edge);
            c.text(
                &self.font,
                &key.values[row],
                r.0 + r.2 / 2.0,
                base,
                19.0,
                TEXT,
                Align::Center,
            );
        }

        let can_start = key.selected.is_some();
        for start in [false, true] {
            let r = button_rect(start);
            let hot = key.hover == Some(Hit::Button { start });
            let focused = key.focus == Focus::Buttons { start };
            let (fill, ink) = match (start, can_start) {
                (true, true) => ([112, 80, 28, 245], GOLD),
                (true, false) => ([40, 38, 46, 235], MUTED),
                (false, _) => (PANEL, TEXT),
            };
            c.fill(r, fill);
            let edge = if focused {
                GOLD
            } else if hot && (can_start || !start) {
                HOVER
            } else {
                EDGE
            };
            c.stroke(r, if focused { 2.5 } else { 1.5 }, edge);
            let label = if start { "Start" } else { "Exit" };
            c.text(
                &self.font,
                label,
                r.0 + r.2 / 2.0,
                r.1 + r.3 / 2.0 + 9.0,
                26.0,
                ink,
                Align::Center,
            );
        }
        c.text(
            &self.font,
            concat!("mgcarpet ", env!("CARGO_PKG_VERSION")),
            VW - 12.0,
            VH - 12.0,
            13.0,
            MUTED,
            Align::Right,
        );
        if let Some(text) = &key.notice {
            self.compose_notice(c, key, text);
        }
    }

    /// The notice body laid out: (line, baseline, size, is_path).
    fn notice_lines(&self, text: &[(bool, String)]) -> Vec<(String, f32, f32, bool)> {
        let lead = 25.0;
        let mut base = NOTICE.1 + 105.0;
        let mut out = Vec::new();
        for (path, para) in text {
            if para.is_empty() {
                base += lead / 2.0;
                continue;
            }
            // A path is set smaller and broken by character, not word.
            let size = if *path { 15.0 } else { 18.0 };
            for line in wrap(&self.font, para, size, NOTICE.2 - 70.0, *path) {
                out.push((line, base, size, *path));
                base += lead;
            }
        }
        out
    }

    /// The no-games notice over a dimmed launcher.
    fn compose_notice(&self, c: &mut Canvas, key: &ViewKey, text: &[(bool, String)]) {
        c.dim(170);
        c.fill(NOTICE, [30, 26, 46, 252]);
        c.stroke(NOTICE, 2.0, GOLD);
        c.text(
            &self.font,
            "No games found",
            VW / 2.0,
            NOTICE.1 + 58.0,
            32.0,
            GOLD,
            Align::Center,
        );
        for (line, base, size, path) in self.notice_lines(text) {
            c.text(
                &self.font,
                &line,
                VW / 2.0,
                base,
                size,
                if path { GOLD } else { TEXT },
                Align::Center,
            );
        }
        let r = notice_ok_rect();
        c.fill(r, [112, 80, 28, 245]);
        let hot = key.hover == Some(Hit::NoticeOk);
        c.stroke(r, 2.5, if hot { HOVER } else { GOLD });
        c.text(
            &self.font,
            "OK",
            r.0 + r.2 / 2.0,
            r.1 + r.3 / 2.0 + 9.0,
            26.0,
            GOLD,
            Align::Center,
        );
    }

    fn compose_card(&self, c: &mut Canvas, key: &ViewKey, i: usize) {
        let (x, y, w, h) = card_rect(i);
        let art = (x, y, w, ART_H);
        let ready = key.games[i];
        let selected = key.selected == Some(i);
        let hot = ready && key.hover == Some(Hit::Card(i));
        c.fill((x, y, w, h), PANEL);
        match (&self.art[i], ready) {
            (Some(a), true) => c.image(art, a),
            _ => {
                c.fill(art, [20, 18, 30, 255]);
                c.text(
                    &self.font,
                    "Not available",
                    x + w / 2.0,
                    y + ART_H / 2.0 + 7.0,
                    20.0,
                    MUTED,
                    Align::Center,
                );
            }
        }
        if !ready {
            let inset = 18.0;
            c.line(
                (x + inset, y + inset),
                (x + w - inset, y + ART_H - inset),
                5.0,
                CROSS,
            );
            c.line(
                (x + w - inset, y + inset),
                (x + inset, y + ART_H - inset),
                5.0,
                CROSS,
            );
        }
        let name_ink = if !ready {
            MUTED
        } else if selected {
            GOLD
        } else {
            TEXT
        };
        match GAMES[i].1.split_once(": ") {
            Some((title, sub)) => {
                c.text(
                    &self.font,
                    title,
                    x + w / 2.0,
                    y + ART_H + 24.0,
                    22.0,
                    name_ink,
                    Align::Center,
                );
                c.text(
                    &self.font,
                    sub,
                    x + w / 2.0,
                    y + ART_H + 45.0,
                    16.0,
                    name_ink,
                    Align::Center,
                );
            }
            None => {
                let base = y + ART_H + CAPTION_H / 2.0 + 8.0;
                c.text(
                    &self.font,
                    GAMES[i].1,
                    x + w / 2.0,
                    base,
                    22.0,
                    name_ink,
                    Align::Center,
                );
            }
        }
        let (edge, t) = if selected && key.focus == Focus::Cards {
            (GOLD, 4.0)
        } else if selected {
            (GOLD, 3.0)
        } else if hot {
            (HOVER, 2.0)
        } else {
            (EDGE, 1.5)
        };
        c.stroke((x, y, w, h), t, edge);
    }
}

/// What lies under a window position.
fn hit(size: (f32, f32), cursor: (f32, f32)) -> Option<Hit> {
    let (s, ox, oy) = fit(size.0, size.1);
    let (vx, vy) = ((cursor.0 - ox) / s, (cursor.1 - oy) / s);
    let inside = |r: Rect| vx >= r.0 && vx < r.0 + r.2 && vy >= r.1 && vy < r.1 + r.3;
    for i in 0..3 {
        if inside(card_rect(i)) {
            return Some(Hit::Card(i));
        }
    }
    for row in 0..3 {
        for right in [false, true] {
            if inside(arrow_rect(row, right)) {
                return Some(Hit::Arrow { row, right });
            }
        }
        if inside(value_rect(row)) {
            return Some(Hit::Value(row));
        }
    }
    for start in [false, true] {
        if inside(button_rect(start)) {
            return Some(Hit::Button { start });
        }
    }
    None
}

/// Is the pointer on the notice's OK?
fn notice_hit(size: (f32, f32), cursor: (f32, f32)) -> bool {
    let (s, ox, oy) = fit(size.0, size.1);
    let (vx, vy) = ((cursor.0 - ox) / s, (cursor.1 - oy) / s);
    let r = notice_ok_rect();
    vx >= r.0 && vx < r.0 + r.2 && vy >= r.1 && vy < r.1 + r.3
}

/// A string's advance width in virtual units at `size`.
fn text_width(font: &FontRef, s: &str, size: f32) -> f32 {
    let scaled = font.as_scaled(PxScale::from(size));
    let mut width = 0.0;
    let mut prev = None;
    for ch in s.chars() {
        let id = scaled.glyph_id(ch);
        if let Some(p) = prev {
            width += scaled.kern(p, id);
        }
        width += scaled.h_advance(id);
        prev = Some(id);
    }
    width
}

/// Greedy word wrap to `max` virtual units. `hard` (a path) breaks
/// anywhere, by character, since it has no spaces worth keeping.
fn wrap(font: &FontRef, s: &str, size: f32, max: f32, hard: bool) -> Vec<String> {
    let mut lines = Vec::new();
    let mut cur = String::new();
    if hard {
        for ch in s.chars() {
            cur.push(ch);
            if text_width(font, &cur, size) > max {
                cur.pop();
                lines.push(std::mem::take(&mut cur));
                cur.push(ch);
            }
        }
    } else {
        for word in s.split_whitespace() {
            let next = if cur.is_empty() {
                word.to_string()
            } else {
                format!("{cur} {word}")
            };
            if !cur.is_empty() && text_width(font, &next, size) > max {
                lines.push(std::mem::replace(&mut cur, word.to_string()));
            } else {
                cur = next;
            }
        }
    }
    if !cur.is_empty() {
        lines.push(cur);
    }
    lines
}

/// The virtual canvas's scale and offset in a window.
fn fit(w: f32, h: f32) -> (f32, f32, f32) {
    let s = (w / VW).min(h / VH).max(0.01);
    (s, (w - VW * s) / 2.0, (h - VH * s) / 2.0)
}

/// A card's art from the bake: the finished title card of the game's
/// title movie — the one that animates the game's name after the
/// story intro (MC1 `TITLE-01`, Hidden Worlds' own `TITLE-03`, MC2
/// `INTRO2`). Its last SHOWN frame: retail's player stops at
/// `frame_count − 1` (see `movie.rs`); the titles fade in and hold, so
/// that frame is the whole title at full brightness.
fn load_art(baked_root: &Path, game: CampaignId) -> Result<Art, String> {
    let (dir, name) = match game {
        CampaignId::Mc1 => ("mc1-movies", "title-01"),
        CampaignId::Mc1Hw => ("mc1-movies", "title-03"),
        CampaignId::Mc2 => ("mc2-movies", "intro2"),
    };
    let dir = baked_root.join("assets").join(dir);
    let index: mgc_formats::bundle::MovieIndex = std::fs::read(dir.join("movies.json"))
        .map_err(|e| format!("movies.json: {e}"))
        .and_then(|b| serde_json::from_slice(&b).map_err(|e| format!("movies.json: {e}")))?;
    let entry = index
        .movies
        .iter()
        .find(|m| m.name == name)
        .ok_or_else(|| format!("no {name} movie baked"))?;
    let raw = std::fs::read(dir.join(&entry.file)).map_err(|e| format!("{}: {e}", entry.file))?;
    let mut cur = mgc_import::fmv::FmvCursor::new(raw, None)?;
    let last = cur.frame_count().saturating_sub(1);
    while cur.played() < last && cur.advance()? {}
    let pal = cur.palette().ok_or_else(|| format!("{name}: no palette"))?;
    // VGA DAC palette: 6 bits per channel.
    let rgb = cur
        .canvas()
        .iter()
        .flat_map(|&i| {
            let e = i as usize * 3;
            [pal[e], pal[e + 1], pal[e + 2]].map(|c| (c & 0x3F) << 2 | (c & 0x3F) >> 4)
        })
        .collect();
    Ok(Art {
        w: cur.width(),
        h: cur.height(),
        rgb,
    })
}

#[derive(Clone, Copy)]
enum Align {
    Center,
    Right,
}

/// The window-sized RGBA frame and the virtual → pixel mapping.
struct Canvas {
    w: usize,
    h: usize,
    s: f32,
    ox: f32,
    oy: f32,
    buf: Vec<u8>,
}

impl Canvas {
    fn new(w: usize, h: usize) -> Self {
        let (s, ox, oy) = fit(w as f32, h as f32);
        Canvas {
            w,
            h,
            s,
            ox,
            oy,
            buf: vec![0; w * h * 4],
        }
    }

    fn px(&self, vx: f32, vy: f32) -> (f32, f32) {
        (self.ox + vx * self.s, self.oy + vy * self.s)
    }

    /// Blend `c` at `alpha` (0..=1 on top of c's own alpha) into pixel (x, y).
    fn blend(&mut self, x: i64, y: i64, c: Rgba, alpha: f32) {
        if x < 0 || y < 0 || x >= self.w as i64 || y >= self.h as i64 {
            return;
        }
        let a = alpha * c[3] as f32 / 255.0;
        if a <= 0.0 {
            return;
        }
        let o = (y as usize * self.w + x as usize) * 4;
        for k in 0..3 {
            let d = self.buf[o + k] as f32;
            self.buf[o + k] = (d + (c[k] as f32 - d) * a).round() as u8;
        }
        self.buf[o + 3] = 255;
    }

    /// Darken the whole window (behind a modal panel).
    fn dim(&mut self, alpha: u8) {
        for px in self.buf.chunks_exact_mut(4) {
            for k in 0..3 {
                px[k] = (px[k] as u32 * (255 - alpha as u32) / 255) as u8;
            }
        }
    }

    /// The whole window: a dusk gradient behind the layout.
    fn background(&mut self) {
        let (top, bottom) = ([26, 18, 46], [6, 6, 14]);
        for y in 0..self.h {
            let t = y as f32 / self.h.max(1) as f32;
            let c: [u8; 3] = std::array::from_fn(|k| {
                (top[k] as f32 + (bottom[k] as f32 - top[k] as f32) * t) as u8
            });
            for x in 0..self.w {
                let o = (y * self.w + x) * 4;
                self.buf[o..o + 4].copy_from_slice(&[c[0], c[1], c[2], 255]);
            }
        }
    }

    /// A virtual rect's covered pixel span, with fractional edge
    /// coverage for anti-aliasing.
    fn fill(&mut self, r: Rect, c: Rgba) {
        let (x0, y0) = self.px(r.0, r.1);
        let (x1, y1) = self.px(r.0 + r.2, r.1 + r.3);
        for y in y0.floor() as i64..y1.ceil() as i64 {
            let cy = (y1.min(y as f32 + 1.0) - y0.max(y as f32)).clamp(0.0, 1.0);
            for x in x0.floor() as i64..x1.ceil() as i64 {
                let cx = (x1.min(x as f32 + 1.0) - x0.max(x as f32)).clamp(0.0, 1.0);
                self.blend(x, y, c, cx * cy);
            }
        }
    }

    /// A border `t` virtual units thick, inside the rect.
    fn stroke(&mut self, r: Rect, t: f32, c: Rgba) {
        let (x, y, w, h) = r;
        self.fill((x, y, w, t), c);
        self.fill((x, y + h - t, w, t), c);
        self.fill((x, y + t, t, h - 2.0 * t), c);
        self.fill((x + w - t, y + t, t, h - 2.0 * t), c);
    }

    /// An anti-aliased segment `t` virtual units wide.
    fn line(&mut self, a: (f32, f32), b: (f32, f32), t: f32, c: Rgba) {
        let (ax, ay) = self.px(a.0, a.1);
        let (bx, by) = self.px(b.0, b.1);
        let half = t * self.s / 2.0;
        let (dx, dy) = (bx - ax, by - ay);
        let len2 = (dx * dx + dy * dy).max(1e-6);
        let (lo_x, hi_x) = (ax.min(bx) - half - 1.0, ax.max(bx) + half + 1.0);
        let (lo_y, hi_y) = (ay.min(by) - half - 1.0, ay.max(by) + half + 1.0);
        for y in lo_y.floor() as i64..hi_y.ceil() as i64 {
            for x in lo_x.floor() as i64..hi_x.ceil() as i64 {
                let (px, py) = (x as f32 + 0.5, y as f32 + 0.5);
                let u = (((px - ax) * dx + (py - ay) * dy) / len2).clamp(0.0, 1.0);
                let (qx, qy) = (ax + u * dx - px, ay + u * dy - py);
                let d = (qx * qx + qy * qy).sqrt();
                self.blend(x, y, c, (half + 0.5 - d).clamp(0.0, 1.0));
            }
        }
    }

    /// A selector arrow (a filled triangle) centred in `r`, pointing
    /// right or left, anti-aliased by the distance to its nearest edge.
    fn arrow(&mut self, r: Rect, right: bool, c: Rgba) {
        let (cx, cy) = (r.0 + r.2 / 2.0, r.1 + r.3 / 2.0);
        let (hw, hh) = (7.0, 11.0);
        let (tip, back) = if right {
            (cx + hw, cx - hw)
        } else {
            (cx - hw, cx + hw)
        };
        let tri = [
            self.px(tip, cy),
            self.px(back, cy - hh),
            self.px(back, cy + hh),
        ];
        // Signed distance to the edge a→b, positive on the side the
        // triangle's third vertex lies on (so the winding is moot).
        let inside = |a: (f32, f32), b: (f32, f32), o: (f32, f32), p: (f32, f32)| {
            let (ex, ey) = (b.0 - a.0, b.1 - a.1);
            let len = (ex * ex + ey * ey).sqrt().max(1e-6);
            let side = |q: (f32, f32)| ((q.0 - a.0) * ey - (q.1 - a.1) * ex) / len;
            side(p) * side(o).signum()
        };
        let lo_x = tri.iter().map(|p| p.0).fold(f32::MAX, f32::min).floor() as i64;
        let hi_x = tri.iter().map(|p| p.0).fold(f32::MIN, f32::max).ceil() as i64;
        let lo_y = tri.iter().map(|p| p.1).fold(f32::MAX, f32::min).floor() as i64;
        let hi_y = tri.iter().map(|p| p.1).fold(f32::MIN, f32::max).ceil() as i64;
        for y in lo_y..=hi_y {
            for x in lo_x..=hi_x {
                let p = (x as f32 + 0.5, y as f32 + 0.5);
                let d = inside(tri[0], tri[1], tri[2], p)
                    .min(inside(tri[1], tri[2], tri[0], p))
                    .min(inside(tri[2], tri[0], tri[1], p));
                self.blend(x, y, c, (d + 0.5).clamp(0.0, 1.0));
            }
        }
    }

    /// Cover-fit `a` into `r` (cropped centred), bilinear.
    fn image(&mut self, r: Rect, a: &Art) {
        let (x0, y0) = self.px(r.0, r.1);
        let (x1, y1) = self.px(r.0 + r.2, r.1 + r.3);
        let (dw, dh) = (x1 - x0, y1 - y0);
        let k = (a.w as f32 / dw).min(a.h as f32 / dh);
        let (sx0, sy0) = ((a.w as f32 - dw * k) / 2.0, (a.h as f32 - dh * k) / 2.0);
        let texel = |x: usize, y: usize, ch: usize| a.rgb[(y * a.w + x) * 3 + ch] as f32;
        for y in y0.round() as i64..y1.round() as i64 {
            for x in x0.round() as i64..x1.round() as i64 {
                let u = (sx0 + (x as f32 + 0.5 - x0) * k - 0.5).clamp(0.0, a.w as f32 - 1.0);
                let v = (sy0 + (y as f32 + 0.5 - y0) * k - 0.5).clamp(0.0, a.h as f32 - 1.0);
                let (ui, vi) = (u as usize, v as usize);
                let (u1, v1) = ((ui + 1).min(a.w - 1), (vi + 1).min(a.h - 1));
                let (fu, fv) = (u - ui as f32, v - vi as f32);
                let c: [u8; 3] = std::array::from_fn(|ch| {
                    let top = texel(ui, vi, ch) * (1.0 - fu) + texel(u1, vi, ch) * fu;
                    let bot = texel(ui, v1, ch) * (1.0 - fu) + texel(u1, v1, ch) * fu;
                    (top * (1.0 - fv) + bot * fv).round() as u8
                });
                self.blend(x, y, [c[0], c[1], c[2], 255], 1.0);
            }
        }
    }

    /// Text on a baseline at virtual (x, base), `size` virtual px tall.
    fn text(
        &mut self,
        font: &FontRef,
        s: &str,
        x: f32,
        base: f32,
        size: f32,
        c: Rgba,
        align: Align,
    ) {
        let scaled = font.as_scaled(PxScale::from(size * self.s));
        let mut width = 0.0;
        let mut prev = None;
        for ch in s.chars() {
            let id = scaled.glyph_id(ch);
            if let Some(p) = prev {
                width += scaled.kern(p, id);
            }
            width += scaled.h_advance(id);
            prev = Some(id);
        }
        let (px, py) = self.px(x, base);
        let mut pen = match align {
            Align::Center => px - width / 2.0,
            Align::Right => px - width,
        };
        let mut prev = None;
        for ch in s.chars() {
            let id = scaled.glyph_id(ch);
            if let Some(p) = prev {
                pen += scaled.kern(p, id);
            }
            let glyph = id.with_scale_and_position(scaled.scale(), ab_glyph::point(pen, py));
            if let Some(g) = font.outline_glyph(glyph) {
                let b = g.px_bounds();
                g.draw(|gx, gy, cov| {
                    self.blend(
                        b.min.x as i64 + gx as i64,
                        b.min.y as i64 + gy as i64,
                        c,
                        cov,
                    );
                });
            }
            pen += scaled.h_advance(id);
            prev = Some(id);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_layout_fits_the_canvas_and_nothing_overlaps() {
        let mut rects: Vec<Rect> = (0..3).map(card_rect).collect();
        for row in 0..3 {
            rects.extend([
                arrow_rect(row, false),
                value_rect(row),
                arrow_rect(row, true),
            ]);
        }
        rects.extend([button_rect(false), button_rect(true)]);
        for (i, a) in rects.iter().enumerate() {
            assert!(
                a.0 >= 0.0 && a.1 >= 0.0 && a.0 + a.2 <= VW && a.1 + a.3 <= VH,
                "{a:?}"
            );
            for b in &rects[i + 1..] {
                let apart =
                    a.0 + a.2 <= b.0 || b.0 + b.2 <= a.0 || a.1 + a.3 <= b.1 || b.1 + b.3 <= a.1;
                assert!(apart, "{a:?} overlaps {b:?}");
            }
        }
    }

    #[test]
    fn the_selector_arrows_paint_their_centre() {
        for right in [false, true] {
            let mut c = Canvas::new(960, 720);
            let r = arrow_rect(0, right);
            c.arrow(r, right, [255, 255, 255, 255]);
            let (x, y) = ((r.0 + r.2 / 2.0) as usize, (r.1 + r.3 / 2.0) as usize);
            assert_eq!(
                c.buf[(y * 960 + x) * 4],
                255,
                "right={right}: centre painted"
            );
            assert_eq!(
                c.buf[(r.1 as usize * 960 + r.0 as usize) * 4],
                0,
                "corner left bare"
            );
        }
    }

    #[test]
    fn hits_land_on_what_is_drawn_at_any_window_shape() {
        for size in [(960.0, 720.0), (1920.0, 1080.0), (800.0, 1200.0)] {
            let (s, ox, oy) = fit(size.0, size.1);
            let at = |r: Rect| (ox + (r.0 + r.2 / 2.0) * s, oy + (r.1 + r.3 / 2.0) * s);
            assert_eq!(hit(size, at(card_rect(2))), Some(Hit::Card(2)));
            assert_eq!(
                hit(size, at(arrow_rect(1, true))),
                Some(Hit::Arrow {
                    row: 1,
                    right: true
                })
            );
            assert_eq!(
                hit(size, at(button_rect(true))),
                Some(Hit::Button { start: true })
            );
            assert_eq!(hit(size, (ox + 2.0, oy + 2.0)), None);
        }
    }

    #[test]
    fn an_unbaked_tree_leaves_every_game_unavailable_and_unstartable() {
        let root = std::env::temp_dir().join(format!("mgc-launcher-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let mut cfg = Config {
            gamedata: Some(root.join("no-such-gamedata")),
            ..Default::default()
        };
        let mut l = Launcher::new(&root, &cfg);
        assert_eq!(l.status.games, [false; 3]);
        assert!(l.status.tree.is_some());
        assert!(l.bake.is_none(), "no data found, nothing to bake");
        assert!(l.status_line().starts_with("No game data found"));
        assert_eq!(l.selected, None);
        // The notice is up and modal: Esc and clicks elsewhere do not
        // reach the launcher; OK (or Enter/Esc) dismisses it.
        assert!(l.notice, "no games: the notice opens");
        let (img, _) = l.frame((960.0, 720.0), (0.0, 0.0), &cfg);
        assert!(img.is_some());
        l.escape();
        assert!(!l.notice, "Esc closes the notice");
        assert_eq!(l.take_action(), None, "…and does not quit");
        l.notice = true;
        l.click(
            (960.0, 720.0),
            (button_rect(false).0 + 5.0, button_rect(false).1 + 5.0),
            &mut cfg,
        );
        assert_eq!(l.take_action(), None, "Exit is behind the notice");
        assert!(l.notice);
        let ok = notice_ok_rect();
        l.click(
            (960.0, 720.0),
            (ok.0 + ok.2 / 2.0, ok.1 + ok.3 / 2.0),
            &mut cfg,
        );
        assert!(!l.notice, "OK closes it");
        // The longest body (a failed bake, a deep working directory)
        // still clears the OK button.
        l.message = Some(format!(
            "Preparing the game data failed: {}",
            "x ".repeat(60)
        ));
        let mut text = l.notice_text();
        text.iter_mut()
            .filter(|(p, _)| *p)
            .for_each(|(_, t)| *t = "/very/deep/".repeat(12));
        let lines = l.notice_lines(&text);
        let last = lines.last().expect("a body").1;
        assert!(
            last + 12.0 < ok.1,
            "text bottom {last} runs into OK at {}",
            ok.1
        );
        l.click(
            (960.0, 720.0),
            (card_rect(0).0 + 10.0, card_rect(0).1 + 10.0),
            &mut cfg,
        );
        l.key(
            &winit::keyboard::Key::Named(winit::keyboard::NamedKey::Enter),
            &mut cfg,
        );
        assert_eq!(l.take_action(), None, "nothing to start");
        l.escape();
        assert_eq!(l.take_action(), Some(Action::Exit));
        let (img, quads) = l.frame((320.0, 240.0), (0.0, 0.0), &cfg);
        assert_eq!(img.map(|b| b.len()), Some(320 * 240 * 4));
        assert_eq!(quads.len(), 1);
        assert!(
            l.frame((320.0, 240.0), (0.0, 0.0), &cfg).0.is_none(),
            "unchanged: no re-compose"
        );
    }

    /// Row 0 is Display: fullscreen, then the window sizes that fit.
    #[test]
    fn the_display_row_offers_the_sizes_that_fit() {
        let root = std::env::temp_dir().join(format!("mgc-launcher-d-{}", std::process::id()));
        let mut cfg = Config {
            gamedata: Some(root.join("no-such-gamedata")),
            ..Default::default()
        };
        let mut l = Launcher::new(&root, &cfg);
        l.set_monitor(Some((1920, 1080)));
        assert_eq!(l.value(0, &cfg), "Fullscreen");
        l.step_option(0, true, &mut cfg);
        assert_eq!(l.value(0, &cfg), "Window 1280 × 960");
        assert!(!cfg.render.preference.fullscreen);
        l.step_option(0, true, &mut cfg);
        assert_eq!(
            l.value(0, &cfg),
            "Fullscreen",
            "1600 × 1200 does not fit 1080 rows"
        );
        l.set_monitor(Some((2560, 1440)));
        l.step_option(0, false, &mut cfg);
        assert_eq!(l.value(0, &cfg), "Window 1600 × 1200");
        assert_eq!(cfg.render.preference.window_size, WindowSize::W1600x1200);
        assert!(l.take_changed().contains(&"render.preference.window_size"));
    }

    /// Row 1 is Controls: stepping writes the preset, the value reads
    /// back from the config, and a group that was hand-tuned when the
    /// launcher opened can step back to exactly those values.
    #[test]
    fn the_controls_row_sets_presets_and_restores_custom() {
        let root = std::env::temp_dir().join(format!("mgc-launcher-p-{}", std::process::id()));
        let mut cfg = Config {
            gamedata: Some(root.join("no-such-gamedata")),
            ..Default::default()
        };
        let mut l = Launcher::new(&root, &cfg);
        assert_eq!(l.value(1, &cfg), "Enhanced");
        l.step_option(1, true, &mut cfg);
        assert_eq!(l.value(1, &cfg), "Classic");
        assert_eq!(
            cfg.controls.models.thrust,
            crate::config::ThrustModel::Classic
        );
        l.step_option(1, true, &mut cfg);
        assert_eq!(
            l.value(1, &cfg),
            "Enhanced",
            "no Custom to step to: it was not tuned"
        );
        assert_eq!(l.take_changed(), vec!["controls.preset", "controls.preset"]);

        cfg.controls.preferences.mouse_sensitivity_x = 0.8;
        let mut l = Launcher::new(&root, &cfg);
        assert_eq!(l.value(1, &cfg), "Custom");
        l.step_option(1, true, &mut cfg);
        assert_eq!(l.value(1, &cfg), "Enhanced");
        l.step_option(1, false, &mut cfg);
        assert_eq!(l.value(1, &cfg), "Custom");
        assert!((cfg.controls.preferences.mouse_sensitivity_x - 0.8).abs() < 1e-6);
    }
}
