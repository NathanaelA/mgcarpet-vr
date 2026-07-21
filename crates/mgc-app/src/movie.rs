//! The full-screen FMV player: intros, outros and the MC2 cutscenes.
//!
//! Retail streams these movies a frame at a time out of the original
//! file (`PlayInfoFmv_107C0`, remc1 sub_main.cpp:16159; remc2
//! `ReadFrame_75DB0`/`DrawFrame_75E70`, Animation.cpp:41) and so do we
//! — the bundle stores the raw stream and
//! [`mgc_import::fmv::FmvCursor`] decodes into one reusable 320×200
//! canvas. Pre-decoding is not an option: MC1's intro is 3165 frames,
//! which is ~200 MB of canvases for a 75 MB stream.
//!
//! Three laws transcribed from that player, each easy to get wrong:
//!
//! 1. **The last frame never shows.** Both games break the loop at
//!    `frame >= frameCount - 1`, so a 3165-frame movie plays 3164 of
//!    them. The final FLIC frame is the ring delta back to frame 0 and
//!    is never meant to be seen. (This applies to `PlayInfoFmv` only —
//!    the looping menu decorations run a different stepper.)
//!
//! 2. **Pacing comes from a compiled-in event script, not the file.**
//!    The FLIC per-frame delay field is never read. Each movie has a
//!    table of `(startFrame, key, index)` records; key `'A'` sets the
//!    inter-frame delay in ticks of the games' shared 120 Hz timer, and
//!    the player busy-waits on it (`sub_10300`, remc1:15894). Delay 5 =
//!    24 fps, and 5 is the default (`dword_9ADC4`, remc1:6457). Movies
//!    change rate constantly — MC1's intro does so ~40 times, holding
//!    single frames for seconds. See [`script`].
//!
//! 3. **Skip is per movie, not per chain.** The abort flag is cleared
//!    at the top of every `PlayInfoFmv`, so skipping MC2's INTRO still
//!    plays INTRO2 (MenusAndIntros.cpp:775-786). Several movies are not
//!    skippable at all — the outro, and MC2's cutscenes.
//!
//! 4. **The movies are NOT silent, though the container is.** There is
//!    no audio stream in the file, but that is not the same as having
//!    no soundtrack: the event script assembles one at playback time
//!    out of the games' ordinary sample banks — narration clips,
//!    effects, ambient loops — over a MIDI score, with subtitles
//!    against the narration. MC1's intro alone cues 51 samples, twelve
//!    voice clips and seventeen subtitle lines. See [`script`].

use std::collections::VecDeque;
use std::path::{Path, PathBuf};

use mgc_formats::bundle::MovieIndex;
use mgc_import::fmv::FmvCursor;
use mgc_render::UiQuad;

/// The games' shared timer rate; every delay in the event scripts is
/// denominated in its ticks (MC1 programs the PIT with divisor 9903 =
/// 120.48 Hz, remc1:67858; MC2 asks AIL for 120, EF:43027).
const TICK_HZ: f32 = 120.0;

/// The inter-frame delay a movie runs at until its script says
/// otherwise: 5 ticks = 24 fps (`dword_9ADC4 = 5`, remc1:6457).
const DEFAULT_DELAY: u16 = 5;

/// Playback runs this much SLOWER than the authored delays — a
/// deliberate, player-ruled deviation (docs/DEVIATIONS.md).
///
/// The scripts cue each narration clip and then load the next sample
/// bank a fixed number of frames later, and a bank load stops every
/// voice — so a clip that outlasts its scene is cut off mid-sentence.
/// Retail does this too; it simply authored the timing tight. But it
/// reads as a defect, and stretching every delay by the same factor
/// buys the voice room without disturbing the relative pacing at all.
///
/// The value is MEASURED, not guessed — see
/// `narration_clips_fit_before_their_bank_is_swapped`, which walks
/// every script against the real clip lengths. At the authored rate
/// `voc5a` is cut 0.15 s short; the binding constraint is `voc11`,
/// 6.13 s of speech in a 5.11 s scene, which needs **1.20**. Anything
/// below that clips a line. 1.25 is 1.20 plus headroom, and matches
/// the player's own "about 20%" call.
const RATE_SCALE: f32 = 1.25;

/// Fire each frame's script records this many frames EARLY.
///
/// Retail's order is events(N) → draw(N) → wait, which is what the port
/// implements, so the delay a record sets governs the frame it rides
/// on. Played back, the long scene holds still landed a frame or two
/// into the NEXT page-flip rather than on the settled page
/// (player-observed). Leading the script by one frame parks the hold
/// where the animation actually rests. Player-ruled; the transcription
/// itself is unchanged.
const SCRIPT_LEAD: u16 = 1;

/// Palette-ramp fade between movies, in seconds. Retail ramps the DAC
/// in 16 steps out and 32 in around every `PlayInfoFmv`
/// (`FadeInOut_61CC0_621D0(0, 0x10, 0)`), outside the player itself.
const FADE_OUT: f32 = 16.0 / TICK_HZ;
const FADE_IN: f32 = 32.0 / TICK_HZ;

/// One movie in a chain, with the properties retail's call sites give
/// it. See [`crate::movie::script`] for the pacing.
#[derive(Clone)]
pub struct Cue {
    /// Bundle movie name (the lowercased source stem).
    pub name: &'static str,
    /// Whether a keypress abandons it — `PlayInfoFmv`'s first
    /// argument. The intro chain is skippable; the endings are not.
    pub skippable: bool,
    /// Seconds to hold the last frame afterwards, or 0. Retail holds
    /// the logo for 8 s and the title for 6 s (`sub_4B480_4B7C0`),
    /// interruptible by any input.
    pub hold: f32,
}

impl Cue {
    pub const fn new(name: &'static str) -> Cue {
        Cue {
            name,
            skippable: true,
            hold: 0.0,
        }
    }

    /// An unskippable cue — retail's `PlayInfoFmv(0, ...)`.
    pub const fn unskippable(name: &'static str) -> Cue {
        Cue {
            name,
            skippable: false,
            hold: 0.0,
        }
    }

    pub const fn holding(mut self, seconds: f32) -> Cue {
        self.hold = seconds;
        self
    }
}

/// The per-movie event scripts, transcribed from the compiled-in
/// tables — MC1's at remc1 sub_main.cpp:1809-1953, MC2's at
/// MenusAndIntros.cpp:106-196 and :257-320.
///
/// **These scripts ARE the movies' soundtrack.** The FLIC container
/// has no audio stream, but the movies are not silent: each record is
/// `(startFrame, key, index)` and fires before its frame is drawn,
/// cueing sample-bank loads, one-shot and looping effects, the
/// narration voice clips, the MIDI score and the subtitle lines. A
/// player that honours only the frame delay gets silent movies.
///
/// The bank loads are the giveaway, and they cross-check the whole
/// transcription: MC1's intro loads sample banks 1, 2, 3, 4 and 12,
/// and those banks hold exactly `voc1`..`voc12` (the narration, one
/// clip per `'S'` cue, each paired with a `'Q'` subtitle at the same
/// frame) plus the dragon/fire/quake effects for the action reel. MC2
/// banks 5-9 are `viscut1`..`viscut5` — one narration clip per
/// cutscene. Every `'S'` index in every table lands inside its bank's
/// entry count.
///
/// Key meanings are NOT shared between the games and must not be
/// merged: `'Z'` loops music in MC1 but is a plain alias of `'M'` in
/// MC2, and `'O'`/`'P'` open and tear down MC1's subtitle overlay
/// while in MC2 they set a playing sample's volume. The tables below
/// are therefore per game, already resolved to game-neutral events.
mod script {
    /// What a ported record does when the playhead reaches it.
    #[derive(Clone, Copy, PartialEq, Eq)]
    pub enum Event {
        /// `'A'`: inter-frame delay in 120 Hz ticks.
        Delay(u16),
        /// `'M'`/`'D'`: start a baked music track.
        Music(&'static str),
        /// MC1 `'Z'`: start a track and re-trigger it when it ends.
        MusicLooped(&'static str),
        /// `'X'`: stop the music.
        StopMusic,
        /// `'E'`: select the sample bank later cues index into.
        Bank(u32),
        /// `'S'` with a nonzero index: one-shot sample.
        Sample(u32),
        /// `'R'`, and MC2's `'H'`: looping sample.
        SampleLoop(u32),
        /// `'T'` with a nonzero index: stop that sample.
        StopSample(u32),
        /// MC1 `'O'` / MC2 `'V'`: open the subtitle overlay.
        ///
        /// Two keys of the retail space have no variant because no
        /// transcribed table uses them: `'S'`/`'T'` with index 0
        /// (stop every sample) and `'K'`/`'W'` (clear the strip,
        /// always immediately followed by the line that replaces it).
        SubtitlesOn,
        /// MC1 `'P'` / MC2 `'Y'`: tear it down.
        SubtitlesOff,
        /// MC1 `'Q'` / MC2 `'U'`: show a subtitle line by string index.
        Subtitle(u32),
    }
    use Event::{
        Bank, Delay, Music, MusicLooped, Sample, SampleLoop, StopMusic, StopSample, Subtitle,
        SubtitlesOff, SubtitlesOn,
    };

    /// `(movie name, [(start_frame, event)])`, in frame order. A movie
    /// absent here runs at the default delay in silence.
    ///
    /// Retail's delay is a process-global that persists across movies
    /// rather than resetting (`dword_9ADC4`, remc1:6457 — only the `A`
    /// key writes it). Every table below opens with an `A` record, so
    /// resetting per movie is equivalent and cannot drift.
    pub const SCRIPTS: &[(&str, &[(u16, Event)])] = &[
        // --- MC1 ------------------------------------------------
        // `dword_4A1FC_4A53C`, the one complete MC1 table: 125
        // records, of which the music-bank load (`B 1`) is implicit in
        // our named tracks and one `I` record is a no-op even in
        // retail. It changes frame rate 39 times, opens on a 2.5 s
        // hold, and carries twelve narration clips against seventeen
        // subtitle lines.
        (
            "intro",
            &[
                (0, SubtitlesOn),
                (0, Bank(1)),
                (1, Music("cintro4")),
                (1, Delay(300)),
                (4, Delay(10)),
                (20, Sample(1)),
                (20, Subtitle(0)),
                (54, Delay(400)),
                (55, Delay(20)),
                (56, Sample(2)),
                (56, Subtitle(1)),
                (65, Delay(90)),
                (66, Subtitle(2)),
                (66, Delay(400)),
                (68, Delay(20)),
                (76, Sample(3)),
                (76, Subtitle(3)),
                (80, Delay(600)),
                (81, Delay(20)),
                (84, Subtitle(4)),
                (84, Sample(4)),
                (93, Delay(700)),
                (94, Delay(20)),
                (98, Sample(5)),
                (98, Subtitle(5)),
                (106, Delay(200)),
                (107, Delay(20)),
                (110, Subtitle(6)),
                (118, Delay(150)),
                (119, Bank(2)),
                (120, Delay(20)),
                (129, Sample(1)),
                (129, Subtitle(7)),
                (132, Delay(400)),
                (133, Delay(20)),
                (136, Subtitle(8)),
                (145, Delay(500)),
                (146, Delay(20)),
                (152, Sample(2)),
                (152, Subtitle(9)),
                (156, Delay(250)),
                (157, Subtitle(10)),
                (159, Delay(20)),
                (165, Sample(3)),
                (165, Subtitle(11)),
                (169, Delay(300)),
                (170, Subtitle(12)),
                (172, Delay(20)),
                (179, Sample(4)),
                (179, Subtitle(13)),
                (184, Delay(500)),
                (185, Delay(20)),
                (188, Sample(5)),
                (188, Subtitle(14)),
                (197, Delay(200)),
                (198, Delay(20)),
                (210, Delay(100)),
                (211, Delay(20)),
                (211, Subtitle(15)),
                (212, Sample(6)),
                (221, Delay(5)),
                (290, Music("cintro5")),
                (300, Delay(5)),
                (308, Bank(3)),
                (318, Sample(3)),
                (343, SampleLoop(2)),
                (371, Delay(10)),
                (371, Subtitle(16)),
                (372, Sample(1)),
                (420, Delay(2)),
                (425, SubtitlesOff),
                (642, StopSample(2)),
                (643, Sample(4)),
                (685, Delay(80)),
                (686, Delay(2)),
                (686, Bank(4)),
                (950, Sample(1)),
                (996, MusicLooped("cintro6")),
                (1059, Sample(2)),
                (1080, Sample(11)),
                (1214, Sample(3)),
                (1234, Sample(4)),
                (1318, SampleLoop(5)),
                (1534, StopSample(5)),
                (1534, Sample(12)),
                (1545, Sample(2)),
                (1655, Sample(14)),
                (1667, Sample(6)),
                (1720, Sample(7)),
                (1800, Sample(11)),
                (1893, Sample(8)),
                (1950, Sample(13)),
                (2027, Sample(2)),
                (2062, Sample(9)),
                (2087, Sample(6)),
                (2125, Sample(7)),
                (2180, Sample(15)),
                (2234, Sample(6)),
                (2282, Sample(10)),
                (2310, Sample(9)),
                (2372, Delay(100)),
                (2373, Bank(12)),
                (2374, Delay(2)),
                (2374, Sample(4)),
                (2395, Sample(5)),
                (2484, SampleLoop(2)),
                (2537, Sample(1)),
                (2594, StopSample(2)),
                (2594, SampleLoop(3)),
                (2674, StopSample(3)),
                (2697, Sample(9)),
                (2725, SampleLoop(3)),
                (2792, StopSample(3)),
                (2796, Sample(7)),
                (2818, Sample(10)),
                (2822, Sample(8)),
                (2831, Sample(6)),
                (2844, Sample(8)),
                (2844, Sample(11)),
                (2925, Sample(5)),
                (2930, Music("cintro5")),
                (2943, Delay(200)),
                (2944, Delay(5)),
            ],
        ),
        // `dword_4A620_4A960` — 12 fps; bank 11 is the single `logo`
        // sting.
        ("logo", &[(0, Delay(10)), (0, Bank(11)), (1, Sample(1))]),
        // `dword_4A568_4A8A8`. Bank 10 = `doorlite`/`carpblob`, struck
        // alternately under the title art.
        ("title-01", TITLE),
        ("title-03", TITLE),
        // `dword_4A5D8_4A918` and the table at 0x4A5FC that nothing
        // references. The two differ ONLY in bank (6 vs 7) and cue
        // frame (200 vs 180) — and banks 6 and 7 hold `win1` and
        // `win2`. That identifies the orphan as LEVELW2's script,
        // which retail loses by pointing both movies at one table:
        // played from retail, `levelw2` is scored with `win1` at the
        // wrong frame. We give each movie its own.
        (
            "levelw1",
            &[
                (0, Delay(10)),
                (0, Bank(6)),
                (1, Music("cintro5")),
                (200, Sample(1)),
            ],
        ),
        (
            "levelw2",
            &[
                (0, Delay(10)),
                (0, Bank(7)),
                (1, Music("cintro5")),
                (180, Sample(1)),
            ],
        ),
        // `dword_4A1C0_4A500` is TRUNCATED in the decompile — only the
        // delay survived. Bank 8 holds exactly one sample, `failed`,
        // so the bank load and cue are reconstructed rather than
        // transcribed; the frame is a guess and is left at 1.
        ("levelose", &[(0, Delay(10)), (0, Bank(8)), (1, Sample(1))]),
        // `dword_4A638_4A978` — 15 fps; bank 9 is door/speed effects.
        (
            "outro",
            &[
                (0, Delay(8)),
                (0, Bank(9)),
                (1, Music("cintro5")),
                (30, Sample(1)),
                (43, Sample(2)),
                (130, Sample(3)),
            ],
        ),
        // --- MC2 ------------------------------------------------
        // `str_E17CC_0x160`. Bank 4 again; `X` stops the intro score.
        (
            "intro2",
            &[
                (0, Delay(3)),
                (0, SubtitlesOff),
                (0, Bank(4)),
                (1, StopMusic),
                (1, Sample(18)),
                (10, Sample(19)),
                (96, Delay(100)),
            ],
        ),
        // `str_E1328` … `str_E1634`: the five cutscenes share one
        // shape — 24 fps for the establishing shot, 12 fps from frame
        // 124 for the dialogue, thunder stabs over the opening, then
        // the narration clip (`viscutN`, sample 1 of each cutscene's
        // own bank) against four subtitle lines.
        ("cut1", CUT1),
        ("cut2", CUT2),
        ("cut3", CUT3),
        ("cut4", CUT4),
        ("cut5", CUT5),
        // `str_E16B4`. Retail writes `A 10` then `A 5` on the same
        // frame; the last write wins, so 24 fps. Alone among the
        // cutscenes it scores off sub-song 1 and reuses bank 4.
        (
            "cut6",
            &[
                (0, Delay(5)),
                (0, Bank(4)),
                (1, Music("mc2-day")),
                (30, Sample(9)),
                (56, Sample(10)),
            ],
        ),
    ];

    /// MC2's INTRO, kept out of [`SCRIPTS`] because both games have a
    /// movie called `intro` — resolved by [`for_movie`].
    ///
    /// `H` starts a loop at volume 0 which a paired `O`/`P` then
    /// raises; we start the loop at full instead, so the ambience
    /// arrives without its fade-in (docs/FIDELITY.md).
    const MC2_INTRO: &[(u16, Event)] = &[
        (0, Delay(10)),
        (0, Bank(4)),
        (0, SubtitlesOn),
        (0, Subtitle(0x10)),
        (2, Music("mc2-intro")),
        (5, Sample(3)),
        (72, Subtitle(0x11)),
        (135, Subtitle(0x12)),
        (189, Delay(240)),
        (190, Delay(5)),
        (190, SampleLoop(1)),
        (260, Sample(2)),
        (350, Sample(11)),
        (365, Sample(13)),
        (400, StopSample(1)),
        (404, Delay(7)),
        (410, Sample(6)),
        (410, Subtitle(0x13)),
        (510, Sample(4)),
        (510, Subtitle(0x14)),
        (582, Sample(9)),
        (589, Delay(5)),
        (615, Sample(10)),
        (700, Sample(8)),
        (700, Subtitle(0x15)),
        (717, Sample(12)),
        (802, SampleLoop(14)),
        (910, Delay(9)),
        (1021, Delay(5)),
        (1021, StopSample(14)),
        (1023, Sample(7)),
        (1040, Subtitle(0x16)),
        (1047, Delay(10)),
        (1047, Sample(5)),
        (1104, Delay(5)),
        (1120, Sample(15)),
        (1225, Sample(16)),
        (1230, Sample(17)),
    ];

    const TITLE: &[(u16, Event)] = &[
        (0, Delay(5)),
        (0, Bank(10)),
        (2, Sample(1)),
        (12, Sample(2)),
        (18, Sample(2)),
        (24, Sample(1)),
        (34, Sample(2)),
        (40, Sample(1)),
        (48, Sample(1)),
        (56, Sample(2)),
        (62, Sample(1)),
        (80, Sample(2)),
        (86, Sample(1)),
        (92, Sample(1)),
        (100, Sample(2)),
    ];

    const CUT1: &[(u16, Event)] = &[
        (0, Bank(5)),
        (0, SubtitlesOn),
        (0, Delay(5)),
        (1, Music("mc2-cuts")),
        (24, Sample(2)),
        (66, Sample(2)),
        (70, Sample(2)),
        (72, Sample(2)),
        (92, Sample(2)),
        (94, Sample(2)),
        (124, Delay(10)),
        (124, Sample(1)),
        (124, Subtitle(0x10A)),
        (210, Subtitle(0x10B)),
        (280, Subtitle(0x10C)),
        (340, Subtitle(0x10D)),
        (450, SubtitlesOff),
        (460, Sample(3)),
        (470, Sample(3)),
        (474, Sample(3)),
        (478, Sample(3)),
    ];

    const CUT2: &[(u16, Event)] = &[
        (0, Bank(6)),
        (0, SubtitlesOn),
        (0, Delay(5)),
        (1, Music("mc2-cuts")),
        (24, Sample(2)),
        (66, Sample(2)),
        (70, Sample(2)),
        (72, Sample(2)),
        (92, Sample(2)),
        (94, Sample(2)),
        (124, Delay(10)),
        (124, Sample(1)),
        (124, Subtitle(0x10E)),
        (200, Subtitle(0x10F)),
        (280, Subtitle(0x110)),
        (340, Subtitle(0x111)),
        (440, Sample(3)),
        (450, SubtitlesOff),
        (464, Sample(3)),
        (478, Sample(3)),
        (515, Sample(3)),
    ];

    const CUT3: &[(u16, Event)] = &[
        (0, Bank(7)),
        (0, SubtitlesOn),
        (0, Delay(5)),
        (1, Music("mc2-cuts")),
        (24, Sample(2)),
        (66, Sample(2)),
        (70, Sample(2)),
        (72, Sample(2)),
        (92, Sample(2)),
        (94, Sample(2)),
        (124, Delay(10)),
        (124, Sample(1)),
        (124, Subtitle(0x112)),
        (222, Subtitle(0x113)),
        (350, SubtitlesOff),
        (354, Sample(3)),
        (364, Sample(3)),
        (374, Sample(3)),
        (384, Sample(3)),
        (394, Sample(3)),
        (404, Sample(3)),
        (414, Sample(3)),
        (424, Sample(3)),
        (434, Sample(3)),
    ];

    const CUT4: &[(u16, Event)] = &[
        (0, Bank(8)),
        (0, SubtitlesOn),
        (0, Delay(5)),
        (1, Music("mc2-cuts")),
        (24, Sample(2)),
        (66, Sample(2)),
        (70, Sample(2)),
        (72, Sample(2)),
        (92, Sample(2)),
        (94, Sample(2)),
        (124, Delay(10)),
        (124, Sample(1)),
        (124, Subtitle(0x114)),
        (200, Subtitle(0x115)),
        (280, Subtitle(0x116)),
        (350, SubtitlesOff),
        (360, Sample(3)),
        (365, Sample(3)),
        (370, Sample(3)),
        (380, Sample(3)),
        (385, Sample(3)),
        (390, Sample(3)),
        (400, Sample(3)),
    ];

    const CUT5: &[(u16, Event)] = &[
        (0, Bank(9)),
        (0, SubtitlesOn),
        (0, Delay(5)),
        (1, Music("mc2-cuts")),
        (24, Sample(2)),
        (66, Sample(2)),
        (70, Sample(2)),
        (72, Sample(2)),
        (92, Sample(2)),
        (94, Sample(2)),
        (124, Delay(10)),
        (124, Sample(1)),
        (124, Subtitle(0x117)),
        (200, Subtitle(0x118)),
    ];

    pub fn for_movie(name: &str, mc2: bool) -> &'static [(u16, Event)] {
        if mc2 && name == "intro" {
            return MC2_INTRO;
        }
        SCRIPTS
            .iter()
            .find(|(n, _)| *n == name)
            .map(|(_, s)| *s)
            .unwrap_or(&[])
    }
}

/// Nearest palette entry to a 6-bit VGA colour — how retail picks the
/// subtitle ink out of whatever palette the movie is currently using
/// (`sub_5CC70_5D180` / `getPaletteIndex_5BE80`).
fn nearest(pal: &[u8; 768], r: u8, g: u8, b: u8) -> u8 {
    let mut best = (u32::MAX, 0u8);
    for i in 0..256usize {
        let d = |a: u8, t: u8| {
            let d = (a & 0x3F) as i32 - t as i32;
            (d * d) as u32
        };
        let cost = d(pal[i * 3], r) + d(pal[i * 3 + 1], g) + d(pal[i * 3 + 2], b);
        if cost < best.0 {
            best = (cost, i as u8);
        }
    }
    best.1
}

/// Something the movie's script asked the audio layer to do. The app
/// owns the mixer, so the player just reports these in order.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Action {
    /// Start a baked music track; `looped` is MC1's `'Z'`.
    Music {
        track: &'static str,
        looped: bool,
    },
    StopMusic,
    /// Select the sample bank the cues below index into.
    Bank(u32),
    /// Play a sample from the selected bank.
    Sample {
        id: u32,
        looped: bool,
    },
    StopSample(u32),
    StopSamples,
}

/// A baked movie bundle: the index plus the directory to read streams
/// from. Streams are NOT held in memory — one is read when it starts
/// to play and dropped when it ends.
pub struct MovieSet {
    dir: PathBuf,
    index: MovieIndex,
    /// The subtitle strip's font and lines, absent on installs whose
    /// bundle predates them.
    subs: Option<Subtitles>,
}

/// The subtitle strip: SFONT1's glyph masks plus the game's string
/// table, both baked into the movie bundle.
struct Subtitles {
    /// Packed 8bpp glyph atlas; pixel value 1 is the body, 2 the
    /// outline (remc1 `sub_51650_51990`).
    atlas: Vec<u8>,
    atlas_width: usize,
    /// Glyph box by sprite id: `(x, y, w, h)`.
    glyphs: Vec<Option<(usize, usize, usize, usize)>>,
    lines: Vec<String>,
}

impl Subtitles {
    /// Sprite id of a character. SFONT1's records start at ASCII 32,
    /// and record 0 is the archive's header pseudo-entry — so space is
    /// id 1 and `A` is id 34, which the baked glyphs confirm.
    fn id(ch: char) -> usize {
        (ch as usize).wrapping_sub(31)
    }

    fn glyph(&self, ch: char) -> Option<(usize, usize, usize, usize)> {
        self.glyphs.get(Self::id(ch)).copied().flatten()
    }

    /// Pen advance for a glyph. Retail steps by the TAB record's width
    /// field MINUS ONE (`tabRecord[4] - 1`, remc1
    /// `DrawText_51560_518A0`) — the glyphs kern by a pixel. Advancing
    /// by the full width instead adds up fast: it pushed MC1's longest
    /// narration line to 353 px against a 300 px pen box, i.e. off the
    /// side of a 320 px screen.
    fn advance(&self, ch: char) -> usize {
        self.glyph(ch).map_or(4, |g| g.2.saturating_sub(1).max(1))
    }

    fn width(&self, s: &str) -> usize {
        s.chars().map(|c| self.advance(c)).sum()
    }

    fn line_height(&self) -> usize {
        self.glyph('A').map_or(14, |g| g.3)
    }

    fn load(dir: &Path) -> Option<Subtitles> {
        let atlas = std::fs::read(dir.join("font.bin")).ok()?;
        let index: mgc_formats::bundle::SpriteIndex =
            serde_json::from_slice(&std::fs::read(dir.join("font.json")).ok()?).ok()?;
        let lines: Vec<String> =
            serde_json::from_slice(&std::fs::read(dir.join("subtitles.json")).ok()?).ok()?;
        let mut glyphs = vec![None; 256];
        for sp in &index.sprites {
            if let (Some(f), true) = (sp.frames.first(), (sp.id as usize) < 256) {
                glyphs[sp.id as usize] = Some((
                    f.x as usize,
                    f.y as usize,
                    sp.width as usize,
                    sp.height as usize,
                ));
            }
        }
        Some(Subtitles {
            atlas,
            atlas_width: index.atlas_width as usize,
            glyphs,
            lines,
        })
    }

    /// Blit one glyph into an 8bpp canvas, mapping the mask's body and
    /// outline values to palette indices.
    ///
    /// `cw`/`ch_` are the canvas dimensions — `cw` is also its row
    /// STRIDE. `right` is the pen box's clip edge, which is narrower
    /// than the canvas; passing it as the stride instead scrambles the
    /// addressing and scatters glyph rows up the picture.
    #[allow(clippy::too_many_arguments)]
    fn blit(
        &self,
        canvas: &mut [u8],
        cw: usize,
        ch_: usize,
        right: usize,
        x: usize,
        y: usize,
        c: char,
        ink: (u8, u8),
    ) {
        let Some((gx, gy, gw, gh)) = self.glyph(c) else {
            return;
        };
        for row in 0..gh {
            let ty = y + row;
            if ty >= ch_ {
                break;
            }
            for col in 0..gw {
                let tx = x + col;
                if tx >= cw.min(right) {
                    break;
                }
                let v = self.atlas[(gy + row) * self.atlas_width + gx + col];
                let out = match v {
                    1 => ink.0,
                    2 => ink.1,
                    _ => continue,
                };
                canvas[ty * cw + tx] = out;
            }
        }
    }
}

impl MovieSet {
    pub fn load(dir: &Path) -> Result<MovieSet, String> {
        let path = dir.join("movies.json");
        let raw = std::fs::read(&path).map_err(|e| format!("{}: {e}", path.display()))?;
        let index: MovieIndex =
            serde_json::from_slice(&raw).map_err(|e| format!("movies.json: {e}"))?;
        Ok(MovieSet {
            subs: Subtitles::load(dir),
            dir: dir.to_path_buf(),
            index,
        })
    }

    pub fn has(&self, name: &str) -> bool {
        self.index.movies.iter().any(|m| m.name == name)
    }

    /// Read and open one stream. `None` if the bundle does not carry
    /// it — an install missing a movie must not be fatal, it just
    /// means that beat is skipped.
    fn open(&self, name: &str) -> Option<FmvCursor<Vec<u8>>> {
        let entry = self.index.movies.iter().find(|m| m.name == name)?;
        let raw = std::fs::read(self.dir.join(&entry.file))
            .map_err(|e| eprintln!("note: movie {name}: {e}"))
            .ok()?;
        FmvCursor::new(raw, None)
            .map_err(|e| eprintln!("note: movie {name}: {e}"))
            .ok()
    }
}

/// Where a movie is in its life: retail brackets each one with a
/// palette ramp and may hold its last frame afterwards.
enum Phase {
    /// Ramping up from black into the first frame.
    FadeIn,
    Playing,
    /// Holding the last frame (logo/title screens).
    Hold,
    /// Ramping down to black before the next movie.
    FadeOut,
}

/// The movie currently on screen.
struct Current {
    cue: Cue,
    cursor: FmvCursor<Vec<u8>>,
    /// Script records still ahead of the playhead.
    script: &'static [(u16, script::Event)],
    /// The delay in force, in 120 Hz ticks.
    delay: u16,
    phase: Phase,
    /// Seconds spent in the current phase.
    elapsed: f32,
}

impl Current {
    /// Retail's loop bound: frames `0 ..= frameCount - 2` are shown,
    /// so the last frame in the file never appears.
    fn frames_to_play(&self) -> usize {
        self.cursor.frame_count().saturating_sub(1)
    }

    fn exhausted(&self) -> bool {
        self.cursor.played() >= self.frames_to_play()
    }

    /// Fire every script record the playhead has reached. Retail runs
    /// this before drawing each frame and never rewinds — the resume
    /// index is persisted, so the scan is strictly forward.
    fn advance_script(
        &mut self,
        out: &mut Vec<Action>,
        subtitle: &mut Option<u32>,
        open: &mut bool,
    ) {
        use script::Event as E;
        let at = self.cursor.played() as u16 + SCRIPT_LEAD;
        while let Some(&(start, event)) = self.script.first() {
            if start > at {
                break;
            }
            self.script = &self.script[1..];
            match event {
                E::Delay(ticks) => self.delay = ticks,
                E::Music(track) => out.push(Action::Music {
                    track,
                    looped: false,
                }),
                E::MusicLooped(track) => out.push(Action::Music {
                    track,
                    looped: true,
                }),
                E::StopMusic => out.push(Action::StopMusic),
                E::Bank(bank) => out.push(Action::Bank(bank)),
                E::Sample(id) => out.push(Action::Sample { id, looped: false }),
                E::SampleLoop(id) => out.push(Action::Sample { id, looped: true }),
                E::StopSample(id) => out.push(Action::StopSample(id)),
                // The overlay is torn up and down around the lines;
                // either way the strip is empty until the next line.
                E::SubtitlesOn => {
                    *open = true;
                    *subtitle = None;
                }
                E::SubtitlesOff => {
                    *open = false;
                    *subtitle = None;
                }
                E::Subtitle(index) => *subtitle = Some(index),
            }
        }
    }

    fn frame_seconds(&self) -> f32 {
        self.delay.max(1) as f32 / TICK_HZ * RATE_SCALE
    }
}

/// A queued run of movies, played back to back.
pub struct MoviePlayer {
    set: MovieSet,
    queue: VecDeque<Cue>,
    cur: Option<Current>,
    /// Seconds owed to the current stream.
    accum: f32,
    /// The resolved 320×200 RGBA frame.
    rgba: Vec<u8>,
    /// Whether `rgba` needs re-resolving (the screen refreshes faster
    /// than any movie plays).
    dirty: bool,
    /// Brightness the fades multiply the frame by, 0..=1.
    level: f32,
    /// Whether this bundle is MC2's (the two games each have a movie
    /// called `intro`, with different scripts).
    mc2: bool,
    /// Audio actions the script has raised, awaiting collection.
    actions: Vec<Action>,
    /// The subtitle line on screen, as a string index, or none.
    subtitle: Option<u32>,
    /// Whether the strip is open. Retail lifts the picture for the
    /// whole time it is, not only while a line is up.
    subtitles_open: bool,
    /// Whether subtitles are wanted at all (the caller's setting).
    subtitles_wanted: bool,
    done: bool,
}

impl MoviePlayer {
    pub const W: usize = 320;
    pub const H: usize = 200;

    /// Queue `cues` (in order) from the bundle at `dir`. Returns
    /// `None` if the bundle is unreadable or none of the cues are in
    /// it, so callers fall straight through to whatever follows.
    pub fn new(dir: &Path, cues: &[Cue], mc2: bool, subtitles: bool) -> Option<MoviePlayer> {
        let set = MovieSet::load(dir)
            .map_err(|e| eprintln!("note: movies unavailable: {e}"))
            .ok()?;
        let queue: VecDeque<Cue> = cues.iter().filter(|c| set.has(c.name)).cloned().collect();
        if queue.is_empty() {
            return None;
        }
        let mut player = MoviePlayer {
            set,
            queue,
            cur: None,
            accum: 0.0,
            rgba: vec![0u8; Self::W * Self::H * 4],
            dirty: true,
            level: 0.0,
            mc2,
            actions: Vec::new(),
            subtitle: None,
            subtitles_open: false,
            subtitles_wanted: subtitles,
            done: false,
        };
        player.next_stream();
        Some(player)
    }

    pub fn done(&self) -> bool {
        self.done
    }

    /// Drain the audio actions the script has raised. The container
    /// holds no audio stream, so these — the narration clips, the
    /// effects and the MIDI cues — ARE the movie's soundtrack.
    pub fn take_actions(&mut self) -> Vec<Action> {
        std::mem::take(&mut self.actions)
    }

    /// The subtitle line currently up, as an index into the game's
    /// string table, or none. MC1's intro alone carries seventeen.
    #[cfg(test)]
    pub fn subtitle(&self) -> Option<u32> {
        self.subtitle
    }

    /// `(movie name, frames shown, frames it will show)` for the movie
    /// on screen — how the tests observe the frame budget and the
    /// per-movie skip, neither of which the running game reads.
    #[cfg(test)]
    pub fn progress(&self) -> Option<(&'static str, usize, usize)> {
        let cur = self.cur.as_ref()?;
        Some((cur.cue.name, cur.cursor.played(), cur.frames_to_play()))
    }

    /// Any key or mouse button: abandon the movie on screen and move
    /// to the next in the chain. Unskippable cues ignore it, and the
    /// skip does NOT clear the queue — retail's abort flag is reset
    /// at the top of each `PlayInfoFmv`.
    pub fn skip(&mut self) {
        match &self.cur {
            // A hold is always interruptible, even after an
            // unskippable movie (`sub_4B480_4B7C0` polls any input).
            Some(cur) if cur.cue.skippable || matches!(cur.phase, Phase::Hold) => {}
            Some(_) => return,
            None => return,
        }
        // Cut the score at once rather than at the end of the fade —
        // a skip should sound immediate. The transition itself stops
        // it too (see `next_stream`), for the natural case.
        self.actions.push(Action::StopMusic);
        self.begin_fade_out();
    }

    /// Advance playback by `dt` seconds. Decodes a bounded number of
    /// frames per call so a long stall drops frames instead of
    /// spinning the decoder — the movies hold single frames for
    /// seconds, so catching up would race through a scene.
    pub fn tick(&mut self, dt: f32) {
        if self.done {
            return;
        }
        let dt = dt.min(0.25);
        let Some(cur) = &mut self.cur else {
            self.done = true;
            return;
        };
        cur.elapsed += dt;
        let (elapsed, hold) = (cur.elapsed, cur.cue.hold);
        match cur.phase {
            Phase::FadeIn => {
                self.level = (elapsed / FADE_IN).min(1.0);
                self.dirty = true;
                if elapsed >= FADE_IN {
                    cur.phase = Phase::Playing;
                    cur.elapsed = 0.0;
                    self.accum = 0.0;
                }
            }
            Phase::Playing => {
                self.accum += dt;
                // Bounded so a stall drops frames rather than racing
                // through a scene the script meant to linger on. Stops
                // the moment the movie leaves Playing — the stream can
                // run out mid-budget, and stepping on would cancel the
                // hold that follows it.
                for _ in 0..8 {
                    let Some(cur) = &self.cur else { break };
                    if !matches!(cur.phase, Phase::Playing) || self.accum < cur.frame_seconds() {
                        break;
                    }
                    self.accum -= cur.frame_seconds();
                    self.step_frame();
                }
            }
            Phase::Hold => {
                if elapsed >= hold {
                    self.begin_fade_out();
                }
            }
            Phase::FadeOut => {
                self.level = (1.0 - elapsed / FADE_OUT).max(0.0);
                self.dirty = true;
                if elapsed >= FADE_OUT {
                    self.next_stream();
                }
            }
        }
    }

    /// Decode one frame, ending the movie when the stream runs out.
    fn step_frame(&mut self) {
        let Some(cur) = &mut self.cur else { return };
        if cur.exhausted() {
            self.end_of_movie();
            return;
        }
        // Retail fires a frame's events BEFORE drawing it, so the
        // delay a record sets governs the frame it rides on, not the
        // one before it (`sub_111B0` then `sub_103F0`/`sub_104D0`).
        let mut actions = std::mem::take(&mut self.actions);
        let mut subtitle = self.subtitle;
        let mut open = self.subtitles_open;
        cur.advance_script(&mut actions, &mut subtitle, &mut open);
        self.actions = actions;
        self.subtitle = subtitle;
        self.subtitles_open = open && self.subtitles_wanted;
        match cur.cursor.advance() {
            Ok(true) => self.dirty = true,
            Ok(false) => self.end_of_movie(),
            Err(e) => {
                eprintln!("note: movie {}: decode stopped: {e}", cur.cue.name);
                self.end_of_movie();
            }
        }
    }

    /// The stream is finished: hold the last frame if this cue asks
    /// for one, else start fading out.
    ///
    /// Only ever acts on a PLAYING movie. It is reachable more than
    /// once for the same stream (the frame-budget loop can call
    /// `step_frame` again after exhaustion), and without this guard a
    /// second call would cancel the hold it just started.
    fn end_of_movie(&mut self) {
        let Some(cur) = &mut self.cur else { return };
        if !matches!(cur.phase, Phase::Playing) {
            return;
        }
        if cur.cue.hold > 0.0 {
            cur.phase = Phase::Hold;
            cur.elapsed = 0.0;
        } else {
            self.begin_fade_out();
        }
    }

    fn begin_fade_out(&mut self) {
        if let Some(cur) = &mut self.cur {
            if matches!(cur.phase, Phase::FadeOut) {
                return;
            }
            cur.phase = Phase::FadeOut;
            cur.elapsed = 0.0;
        }
    }

    /// Open the next queued stream, or finish the chain.
    fn next_stream(&mut self) {
        // The score and any looping effects belong to the movie that
        // started them, and end with it — however it ended.
        //
        // Retail carries the music across: `M`/`Z` start a track and
        // nothing stops it, so MC1's intro theme plays on under the
        // title screen, which has no music cue of its own to replace
        // it. Player-reported as wrong on both the skip and the
        // natural transition; deliberate deviation. (Retail does
        // reload the sample bank per movie, which stops every voice,
        // so the sample half of this IS faithful.)
        if self.cur.is_some() {
            self.actions.push(Action::StopSamples);
            self.actions.push(Action::StopMusic);
        }
        self.cur = None;
        self.subtitle = None;
        self.subtitles_open = false;
        self.level = 0.0;
        self.accum = 0.0;
        self.dirty = true;
        while let Some(cue) = self.queue.pop_front() {
            if let Some(cursor) = self.set.open(cue.name) {
                let script = script::for_movie(cue.name, self.mc2);
                let mut cur = Current {
                    cue,
                    cursor,
                    script,
                    delay: DEFAULT_DELAY,
                    phase: Phase::FadeIn,
                    elapsed: 0.0,
                };
                // Frame 0's records fire before frame 0 is drawn, and
                // frame 0 is decoded up front so the fade-in has
                // picture to ramp.
                let mut actions = std::mem::take(&mut self.actions);
                let mut subtitle = None;
                let mut open = false;
                cur.advance_script(&mut actions, &mut subtitle, &mut open);
                let _ = cur.cursor.advance();
                self.actions = actions;
                self.subtitle = subtitle;
                self.subtitles_open = open && self.subtitles_wanted;
                self.cur = Some(cur);
                return;
            }
        }
        self.done = true;
    }

    /// The current frame as RGBA, plus the quad that presents it —
    /// letterboxed and centred at the window's best fit. Retail ran
    /// 320×200 fullscreen; the centring is ours, for windows that are
    /// not 4:3.
    pub fn frame(&mut self, size: (f32, f32)) -> (&[u8], Vec<UiQuad>) {
        if self.dirty {
            self.resolve();
            self.dirty = false;
        }
        let (scale, ox, oy) = crate::ui::letterbox(size, Self::W as f32, Self::H as f32);
        let (w, h) = (Self::W as f32 * scale, Self::H as f32 * scale);
        let quads = vec![UiQuad {
            rect: [ox, oy, w, h],
            uv: [0.0, 0.0, Self::W as f32, Self::H as f32],
            tint: [1.0, 1.0, 1.0, 1.0],
        }];
        (&self.rgba, quads)
    }

    /// Compose the 320×200 screen: the movie picture, shifted up if the
    /// subtitle strip is open, with the current line drawn under it.
    ///
    /// Retail decodes into a buffer TALLER than the screen and moves
    /// the blit window rather than the picture — MC1 blits from row 21
    /// of a 320×222 buffer (`WScreen + 6720`) and MC2 from row 31,
    /// which lifts the picture and exposes a band at the bottom for the
    /// strip. Composing the shift directly is the same result.
    fn compose(&self) -> Vec<u8> {
        let (w, h) = (Self::W, Self::H);
        let mut buf = vec![0u8; w * h];
        let Some(cur) = &self.cur else { return buf };
        let shift = if self.subtitles_open { self.shift() } else { 0 };
        let canvas = cur.cursor.canvas();
        for y in 0..h {
            let src = y + shift;
            if src >= h {
                break; // the exposed band, left black for the strip
            }
            buf[y * w..y * w + w].copy_from_slice(&canvas[src * w..src * w + w]);
        }
        if let (Some(index), Some(subs)) = (self.subtitle, self.set.subs.as_ref()) {
            let Some(text) = subs.lines.get(index as usize) else {
                return buf;
            };
            // Retail resolves the ink against the MOVIE's live palette
            // every time it changes — pure white for the body, black
            // for the outline (`sub_5CC70_5D180(pal, 0x3F,0x3F,0x3F)`).
            let ink = cur
                .cursor
                .palette()
                .map(|p| (nearest(p, 63, 63, 63), nearest(p, 0, 0, 0)))
                .unwrap_or((255, 0));
            self.draw_subtitle(&mut buf, subs, text, ink);
        }
        buf
    }

    /// How far the picture rides up while the strip is open: MC1 21
    /// rows, MC2 31 (the two games' blit offsets, 6720 and 0x26C0
    /// bytes at 320 wide).
    fn shift(&self) -> usize {
        if self.mc2 { 31 } else { 21 }
    }

    /// Lay one subtitle into the band. The games differ: MC1 pens the
    /// text left-aligned at x=10 with the line breaks authored into the
    /// string itself (`DrawText_51560_518A0` — no wrapping, `\n`
    /// resets x); MC2 word-wraps to the strip width and CENTRES each
    /// line (`DrawText_7FAE0`).
    fn draw_subtitle(&self, buf: &mut [u8], subs: &Subtitles, text: &str, ink: (u8, u8)) {
        let (w, h) = (Self::W, Self::H);
        let lh = subs.line_height();
        let mut lines: Vec<String> = Vec::new();
        if self.mc2 {
            // Word wrap into the 320-wide strip, with the same 3-glyph
            // right margin retail leaves.
            let limit = w.saturating_sub(24);
            let mut line = String::new();
            for word in text.split_whitespace() {
                let candidate = if line.is_empty() {
                    word.to_string()
                } else {
                    format!("{line} {word}")
                };
                if subs.width(&candidate) > limit && !line.is_empty() {
                    lines.push(std::mem::take(&mut line));
                    line = word.to_string();
                } else {
                    line = candidate;
                }
            }
            if !line.is_empty() {
                lines.push(line);
            }
        } else {
            // MC1's strings carry their own breaks as CRLF; `\r` is
            // ignored and the leading space of the next line is part of
            // the authored layout.
            lines = text.split('\n').map(|l| l.replace('\r', "")).collect();
        }
        // The pen: MC1 at buffer (10,180) under a 21-row lift = screen
        // (10,159); MC2's strip starts at buffer row 201 under a
        // 31-row lift = screen row 170.
        let top = if self.mc2 { 170 } else { 159 };
        for (i, line) in lines.iter().enumerate() {
            let y = top + i * lh;
            if y + lh > h {
                break;
            }
            let mut x = if self.mc2 {
                (w.saturating_sub(subs.width(line))) / 2
            } else {
                10
            };
            // Retail clips the strip to its pen box — MC1 x=10..310
            // (`sub_51360_516A0(10, 180, 300, 50)`), MC2 the full
            // 320-space width less a margin.
            let right = if self.mc2 { 315 } else { 310 };
            for c in line.chars() {
                subs.blit(buf, w, h, right, x, y, c, ink);
                x += subs.advance(c);
                if x >= right {
                    break;
                }
            }
        }
    }

    /// Resolve the composed screen under the stream's live palette,
    /// scaled by the fade level. The movies fade WITHIN themselves by
    /// rewriting palette entries between scenes, so the palette is
    /// re-read every frame rather than cached at open; `level` is the
    /// separate ramp retail runs around each movie.
    fn resolve(&mut self) {
        if self.cur.is_none() {
            self.rgba.fill(0);
            for a in self.rgba.chunks_exact_mut(4) {
                a[3] = 255;
            }
            return;
        }
        let canvas = self.compose();
        let Some(pal) = self.cur.as_ref().and_then(|c| c.cursor.palette()) else {
            self.rgba.fill(0);
            for a in self.rgba.chunks_exact_mut(4) {
                a[3] = 255;
            }
            return;
        };
        // 6-bit VGA components, as everywhere else in the frontend.
        // COLOR_256 and COLOR_64 chunks are both 6-bit here — retail
        // hands either straight to the DAC (remc1 sub_108C0 serves
        // both chunk ids with no scaling).
        let level = self.level.clamp(0.0, 1.0);
        let lut: [u8; 64] = std::array::from_fn(|v| ((v as f32 * 4.0) * level) as u8);
        for (i, &idx) in canvas.iter().enumerate() {
            let (o, e) = (i * 4, idx as usize * 3);
            self.rgba[o] = lut[(pal[e] & 0x3F) as usize];
            self.rgba[o + 1] = lut[(pal[e + 1] & 0x3F) as usize];
            self.rgba[o + 2] = lut[(pal[e + 2] & 0x3F) as usize];
            self.rgba[o + 3] = 255;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const BUNDLE: &str = "../../baked/assets/mc1-movies";

    fn set() -> Option<()> {
        Path::new(BUNDLE).is_dir().then_some(())
    }

    /// Drive a two-movie chain to completion and check the frame
    /// budget: retail shows frames `0 ..= frameCount - 2`, so each of
    /// these 4-frame title overlays contributes 3 frames.
    #[test]
    fn chain_plays_every_movie_one_frame_short() {
        if set().is_none() {
            return;
        }
        let cues = [Cue::new("title-02"), Cue::new("title-04")];
        let mut p = MoviePlayer::new(Path::new(BUNDLE), &cues, false, true).expect("bundle opens");
        let mut seen: Vec<(&str, usize)> = Vec::new();
        // 1/60 s steps, generously bounded — the chain is ~1 s of
        // fades plus 8 frames at 24 fps.
        for _ in 0..600 {
            if p.done() {
                break;
            }
            if let Some((name, shown, total)) = p.progress() {
                assert!(shown <= total, "{name}: overran {shown}/{total}");
                match seen.last_mut() {
                    Some((n, s)) if *n == name => *s = shown,
                    _ => seen.push((name, shown)),
                }
            }
            p.tick(1.0 / 60.0);
        }
        assert!(p.done(), "chain never finished");
        assert_eq!(
            seen,
            vec![("title-02", 3), ("title-04", 3)],
            "both movies play, each one frame short of its header count"
        );
    }

    /// Skip abandons the movie on screen and moves to the next — it
    /// does NOT clear the chain (retail resets the abort flag at the
    /// top of every `PlayInfoFmv`).
    #[test]
    fn skip_is_per_movie() {
        if set().is_none() {
            return;
        }
        // `intro` is 3165 frames: without the skip this chain could
        // not possibly reach `logo` inside the loop below.
        let cues = [Cue::new("intro"), Cue::new("logo")];
        let mut p = MoviePlayer::new(Path::new(BUNDLE), &cues, false, true).expect("bundle opens");
        p.tick(1.0 / 60.0);
        assert_eq!(p.progress().map(|x| x.0), Some("intro"));
        p.skip();
        for _ in 0..120 {
            p.tick(1.0 / 60.0);
            if p.progress().map(|x| x.0) == Some("logo") {
                return;
            }
        }
        panic!("skip did not hand over to the next movie");
    }

    /// A movie's score ends with the movie on a NATURAL transition
    /// too, not just a skip. Retail carries it across — nothing in the
    /// scripts stops the music — so MC1's intro theme played on under
    /// the flaming title, which has no music cue to replace it
    /// (player-reported, on the natural end specifically: the skip
    /// path had already been fixed and hid this one).
    #[test]
    fn a_natural_transition_stops_the_music() {
        if set().is_none() {
            return;
        }
        // `title-02` is 4 frames, so it ends on its own almost at once.
        let mut p = MoviePlayer::new(
            Path::new(BUNDLE),
            &[Cue::new("title-02"), Cue::new("title-01")],
            false,
            false,
        )
        .expect("bundle opens");
        let mut acts = Vec::new();
        for _ in 0..200 {
            p.tick(0.05);
            acts.extend(p.take_actions());
            if p.progress().map(|x| x.0) == Some("title-01") {
                break;
            }
        }
        assert_eq!(
            p.progress().map(|x| x.0),
            Some("title-01"),
            "never reached the second movie"
        );
        assert!(
            acts.contains(&Action::StopMusic),
            "the transition did not stop the music: {acts:?}"
        );
    }

    /// Skipping a movie takes its score with it. Retail leaves the
    /// music running — nothing in the scripts stops it — so a skipped
    /// intro played its theme on under the next movie (player-
    /// reported). Deliberate deviation.
    #[test]
    fn skip_stops_the_music() {
        if set().is_none() {
            return;
        }
        let mut p = MoviePlayer::new(
            Path::new(BUNDLE),
            &[Cue::new("intro"), Cue::new("title-01")],
            false,
            false,
        )
        .expect("bundle opens");
        // Run past the intro's frame-1 music cue, then bail out.
        for _ in 0..200 {
            p.tick(0.05);
        }
        let started = p
            .take_actions()
            .iter()
            .any(|a| matches!(a, Action::Music { .. }));
        assert!(started, "the intro never started its score");
        p.skip();
        let after = p.take_actions();
        assert!(
            after.contains(&Action::StopMusic),
            "skipping did not stop the music: {after:?}"
        );
        // `title-01` has no music cue of its own, so nothing would
        // have replaced it.
        assert!(
            !script::for_movie("title-01", false)
                .iter()
                .any(|(_, e)| matches!(e, script::Event::Music(_))),
            "title-01 gained a music cue — pick another silent movie"
        );
    }

    /// An unskippable cue ignores the keypress — the endings and MC2's
    /// cutscenes are `PlayInfoFmv(0, ..)`.
    #[test]
    fn unskippable_ignores_input() {
        if set().is_none() {
            return;
        }
        let cues = [Cue::unskippable("intro"), Cue::new("logo")];
        let mut p = MoviePlayer::new(Path::new(BUNDLE), &cues, false, true).expect("bundle opens");
        p.tick(1.0 / 60.0);
        p.skip();
        for _ in 0..120 {
            p.tick(1.0 / 60.0);
        }
        assert_eq!(
            p.progress().map(|x| x.0),
            Some("intro"),
            "an unskippable movie was skipped"
        );
    }

    /// Pacing is the 120 Hz tick clock, not the display rate: a movie
    /// with no script of its own runs at the default 5 ticks, and every
    /// delay is then stretched by [`RATE_SCALE`] (the player-ruled
    /// slowdown that keeps the narration from being clipped).
    #[test]
    fn default_pacing_follows_the_authored_rate() {
        if set().is_none() {
            return;
        }
        // `intel` has no entry in SCRIPTS (its retail table is
        // truncated in the decompile), so it runs on the default.
        assert!(
            script::for_movie("intel", false).is_empty(),
            "intel gained a script — pick another unscripted movie"
        );
        // Half a second: `intel` is only 41 frames long.
        let shown = frames_in(Cue::new("intel"), 0.5);
        let want = expected_frames(DEFAULT_DELAY, 0.5);
        assert!(
            shown.abs_diff(want) <= 1,
            "advanced {shown} frames in half a second, want ~{want} \
             (5 authored ticks = 24 fps, scaled by {RATE_SCALE})"
        );
    }

    /// An authored delay overrides that default: the logo's table
    /// opens `A 10`, half the default rate.
    #[test]
    fn authored_pacing_is_honoured() {
        if set().is_none() {
            return;
        }
        let shown = frames_in(Cue::new("logo"), 1.0);
        let want = expected_frames(10, 1.0);
        assert!(
            shown.abs_diff(want) <= 1,
            "logo advanced {shown} frames in a second, want ~{want} \
             (10 authored ticks, scaled by {RATE_SCALE})"
        );
        // ...and that IS half the default rate, scale or no scale
        // (±1 for the integer truncation).
        assert!(
            (want * 2).abs_diff(expected_frames(DEFAULT_DELAY, 1.0)) <= 1,
            "the authored ratio must survive the scaling"
        );
    }

    /// Frames playable in `seconds` at an authored delay, after the
    /// player-ruled slowdown.
    fn expected_frames(delay: u16, seconds: f32) -> usize {
        (seconds / (delay as f32 / TICK_HZ * RATE_SCALE)) as usize
    }

    /// The pacing is per-FRAME, not per-movie: MC1's intro holds an
    /// opening frame for 300 authored ticks before dropping to 12 fps.
    /// Getting this wrong is what makes the intro "run comically fast".
    ///
    /// The record sits at frame 1 and [`SCRIPT_LEAD`] fires it a frame
    /// early, so it is frame 0 that parks — the player-ruled correction
    /// that lands scene holds on the settled page rather than a frame
    /// or two into the next flip.
    #[test]
    fn intro_holds_its_opening_frame() {
        if set().is_none() {
            return;
        }
        let mut p = MoviePlayer::new(Path::new(BUNDLE), &[Cue::new("intro")], false, true)
            .expect("bundle opens");
        for _ in 0..60 {
            p.tick(1.0 / 60.0);
        }
        let at_1s = p.progress().expect("playing").1;
        assert_eq!(at_1s, 1, "the opening hold did not take effect");
        // The hold is 300 ticks scaled — a little over three seconds —
        // so the playhead is still parked at 3 s and moving again by 5.
        for _ in 0..120 {
            p.tick(1.0 / 60.0);
        }
        assert_eq!(p.progress().expect("playing").1, 1, "the hold ended early");
        for _ in 0..120 {
            p.tick(1.0 / 60.0);
        }
        let at_5s = p.progress().expect("playing").1;
        assert!(
            (2..=6).contains(&at_5s),
            "after the hold the playhead is at {at_5s}, want it just moving on"
        );
    }

    /// The script also carries the movies' soundtrack — the container
    /// has no audio, so this is the only thing that scores them.
    #[test]
    fn script_cues_the_music() {
        if set().is_none() {
            return;
        }
        let mut p = MoviePlayer::new(Path::new(BUNDLE), &[Cue::new("intro")], false, true)
            .expect("bundle opens");
        let mut cued = None;
        for _ in 0..120 {
            p.tick(1.0 / 60.0);
            if let Some(c) = p
                .take_actions()
                .into_iter()
                .find(|a| matches!(a, Action::Music { .. }))
            {
                cued = Some(c);
                break;
            }
        }
        assert!(
            cued == Some(Action::Music {
                track: "cintro4",
                looped: false
            }),
            "the intro did not cue its own score"
        );
    }

    /// A cue with a hold parks on its last frame afterwards — retail
    /// sits on the logo for 8 s and the title for 6 s
    /// (`sub_4B480_4B7C0`). Regression: the frame-budget loop used to
    /// keep stepping after the stream ran out, which set the hold and
    /// then immediately cancelled it in the same tick.
    #[test]
    fn a_hold_keeps_the_last_frame_up() {
        if set().is_none() {
            return;
        }
        // `title-02` is 4 frames. The steps are DELIBERATELY coarse —
        // one tick buys more frames than the movie has left, so the
        // budget loop runs past the end and `end_of_movie` is reached
        // more than once for the same stream.
        let cues = [Cue::new("title-02").holding(3.0), Cue::new("title-04")];
        let mut p = MoviePlayer::new(Path::new(BUNDLE), &cues, false, true).expect("bundle opens");
        for _ in 0..6 {
            p.tick(0.25);
        }
        assert_eq!(
            p.progress().map(|x| x.0),
            Some("title-02"),
            "the hold was cancelled — moved on inside a 3 s hold"
        );
        for _ in 0..12 {
            p.tick(0.25);
        }
        assert_eq!(
            p.progress().map(|x| x.0),
            Some("title-04"),
            "the hold never ended"
        );
    }

    /// The intro's soundtrack is real: run the whole thing and check
    /// it raises the narration and effect cues, the score, and the
    /// seventeen subtitle lines. A player that honours only the frame
    /// delay would produce silent movies, which is the bug this
    /// guards.
    #[test]
    fn the_intro_cues_its_own_soundtrack() {
        if set().is_none() {
            return;
        }
        let mut p = MoviePlayer::new(Path::new(BUNDLE), &[Cue::new("intro")], false, true)
            .expect("bundle opens");
        let (mut acts, mut subs) = (Vec::new(), Vec::new());
        // Big steps: we want the whole 3164-frame script walked, not
        // real-time playback.
        for _ in 0..4000 {
            p.tick(0.25);
            acts.extend(p.take_actions());
            if let Some(i) = p.subtitle()
                && subs.last() != Some(&i)
            {
                subs.push(i);
            }
            if p.done() {
                break;
            }
        }
        assert!(p.done(), "the intro never finished");
        let banks: Vec<u32> = acts
            .iter()
            .filter_map(|a| match a {
                Action::Bank(b) => Some(*b),
                _ => None,
            })
            .collect();
        assert_eq!(
            banks,
            vec![1, 2, 3, 4, 12],
            "sample banks loaded, in order (1-3 are the narration)"
        );
        let samples = acts
            .iter()
            .filter(|a| matches!(a, Action::Sample { .. }))
            .count();
        assert_eq!(samples, 51, "sample cues raised");
        let music: Vec<_> = acts
            .iter()
            .filter_map(|a| match a {
                Action::Music { track, looped } => Some((*track, *looped)),
                _ => None,
            })
            .collect();
        assert_eq!(
            music,
            vec![
                ("cintro4", false),
                ("cintro5", false),
                ("cintro6", true),
                ("cintro5", false)
            ],
            "the score, including the looped middle section"
        );
        assert_eq!(
            subs,
            (0..17).collect::<Vec<u32>>(),
            "the 17 narration lines"
        );
    }

    /// The narration must fit ACROSS the screen, too. MC1 does not
    /// wrap — the line breaks are authored into the strings — so a pen
    /// advance even one pixel too wide per glyph walks the longest
    /// lines off the side. Advancing by the full glyph width instead
    /// of retail's `width - 1` did exactly that.
    #[test]
    fn mc1_narration_lines_fit_the_screen() {
        if set().is_none() {
            return;
        }
        let subs = MovieSet::load(Path::new(BUNDLE))
            .expect("bundle")
            .subs
            .expect("font + text");
        // Lines 0..=16 are the intro narration; the rest of ETEXT is UI.
        let mut widest = (0usize, String::new());
        for line in subs.lines.iter().take(17) {
            for part in line.split('\n') {
                let part = part.replace('\r', "");
                let px = subs.width(&part);
                if px > widest.0 {
                    widest = (px, part);
                }
            }
        }
        // Pen origin is x=10; the screen is 320 wide.
        let right = 10 + widest.0;
        assert!(
            right <= MoviePlayer::W,
            "the widest narration line runs to x={right} on a {}px screen: {:?}",
            MoviePlayer::W,
            widest.1
        );
        // Sanity that the measurement is real, not an empty table.
        assert!(widest.0 > 250, "widest line is only {}px", widest.0);
    }

    /// The narration must FIT. A script cues a voice clip and then
    /// loads the next sample bank some frames later — and a bank load
    /// stops every voice, so a clip that outlasts its scene is cut off
    /// mid-sentence. This is what [`RATE_SCALE`] exists to fix (MC1's
    /// intro clipped its last book page by about a second at the
    /// authored rate), so it is also what pins the value.
    #[test]
    fn narration_clips_fit_before_their_bank_is_swapped() {
        let dir = Path::new("../../baked/assets/mc1-audio");
        if !dir.is_dir() {
            return;
        }
        let raw = std::fs::read(dir.join("sounds.json")).expect("sounds.json");
        let index: mgc_formats::bundle::SoundIndex =
            serde_json::from_slice(&raw).expect("sounds.json parses");
        let rate = index.sample_rate as f32;
        // The narration clips are named `vocN`; everything else in
        // these banks is an effect, which retail cuts on purpose when
        // the scene ends.
        let clip = |bank: u32, id: u32| -> Option<f32> {
            let e = index
                .banks
                .iter()
                .find(|b| b.bank == bank)?
                .entries
                .iter()
                .find(|e| e.id == id)?;
            e.name.starts_with("voc").then(|| e.len as f32 / rate)
        };

        for (name, events) in script::SCRIPTS {
            if name.starts_with("cut") || *name == "intro2" {
                continue; // MC2 banks
            }
            // Wall-clock time of each frame, at the delays in force.
            let last = events.iter().map(|(f, _)| *f).max().unwrap_or(0) + 600;
            let (mut time_of, mut delay, mut t) = (Vec::new(), DEFAULT_DELAY, 0.0f32);
            let mut i = 0usize;
            for frame in 0..=last {
                while let Some((start, ev)) = events.get(i) {
                    if *start > frame {
                        break;
                    }
                    if let script::Event::Delay(d) = ev {
                        delay = *d;
                    }
                    i += 1;
                }
                time_of.push(t);
                t += delay.max(1) as f32 / TICK_HZ * RATE_SCALE;
            }

            let mut bank = None;
            let mut pending: Vec<(u16, u32, u32, f32)> = Vec::new(); // frame, bank, id, len
            let check = |frame: u16, pending: &mut Vec<(u16, u32, u32, f32)>| {
                for (at, b, id, len) in pending.drain(..) {
                    let window = time_of[frame as usize] - time_of[at as usize];
                    assert!(
                        window + 0.05 >= len,
                        "{name}: sample {b}:{id} is {len:.2}s but bank {b} is \
                         swapped {window:.2}s after it starts — the narration is \
                         cut off (RATE_SCALE {RATE_SCALE} too low?)"
                    );
                }
            };
            for (frame, ev) in *events {
                match ev {
                    script::Event::Bank(b) => {
                        check(*frame, &mut pending);
                        bank = Some(*b);
                    }
                    script::Event::Sample(id) => {
                        if let Some(b) = bank
                            && let Some(len) = clip(b, *id)
                        {
                            pending.push((*frame, b, *id, len));
                        }
                    }
                    _ => {}
                }
            }
            // Whatever is still playing at the end runs to the end of
            // the movie; the tail is not clipped by a bank load.
            check(last, &mut pending);
        }
    }

    /// Every sample the scripts cue must exist in the bank they select
    /// — the transcription's strongest cross-check, since an index
    /// typo almost certainly lands outside its bank's entry count.
    #[test]
    fn every_cued_sample_exists() {
        let mc2_names = |n: &str| n.starts_with("cut") || n == "intro2";
        let mut checked = 0usize;
        for (game, dir) in [
            (false, "../../baked/assets/mc1-audio"),
            (true, "../../baked/assets/mc2-audio"),
        ] {
            let dir = Path::new(dir);
            if !dir.is_dir() {
                continue;
            }
            let raw = std::fs::read(dir.join("sounds.json")).expect("sounds.json");
            let index: mgc_formats::bundle::SoundIndex =
                serde_json::from_slice(&raw).expect("sounds.json parses");
            // MC2's INTRO lives outside SCRIPTS (both games have a
            // movie called `intro`), so walk it explicitly.
            let extra = [("intro", script::for_movie("intro", true))];
            let all =
                script::SCRIPTS
                    .iter()
                    .copied()
                    .chain(if game { extra } else { [("", &[][..])] });
            for (name, events) in all {
                if name.is_empty() || mc2_names(name) != game {
                    continue;
                }
                let mut bank = None;
                for (_, event) in events {
                    match event {
                        script::Event::Bank(b) => bank = Some(*b),
                        script::Event::Sample(id)
                        | script::Event::SampleLoop(id)
                        | script::Event::StopSample(id) => {
                            let b =
                                bank.unwrap_or_else(|| panic!("{name}: sample cue before a bank"));
                            let found = index
                                .banks
                                .iter()
                                .find(|x| x.bank == b)
                                .is_some_and(|x| x.entries.iter().any(|e| e.id == *id));
                            assert!(found, "{name}: no sample {id} in bank {b}");
                            checked += 1;
                        }
                        _ => {}
                    }
                }
            }
        }
        assert!(checked > 120, "only {checked} sample cues checked");
    }

    /// The narration lines must reach PIXELS, not just the cue list:
    /// the strip lifts the picture and the glyphs land in the band it
    /// clears. Counting entity-level cues cannot see a font that fails
    /// to load or a pen placed off-screen.
    #[test]
    fn subtitles_are_drawn_into_the_picture() {
        if set().is_none() {
            return;
        }
        let subs = MovieSet::load(Path::new(BUNDLE)).expect("bundle").subs;
        let subs = subs.expect("the movie bundle carries font + text");
        assert_eq!(subs.lines.len(), 80, "ETEXT.DAT's sentence bank");
        assert!(
            subs.lines[0].starts_with("In its infancy"),
            "line 0 is the intro's opening narration, got {:?}",
            subs.lines[0]
        );

        // Frame 20 raises line 0. Compose with it up and with it
        // cleared — same frame, same shift, so the ONLY difference is
        // the text. Comparing against a subtitles-off run instead would
        // be confounded: the lift changes which picture rows land in
        // the band.
        let mut p = MoviePlayer::new(Path::new(BUNDLE), &[Cue::new("intro")], false, true)
            .expect("bundle opens");
        for _ in 0..400 {
            p.tick(0.05);
            if p.subtitle() == Some(0) {
                break;
            }
        }
        assert_eq!(p.subtitle(), Some(0), "never reached the first line");
        assert!(p.subtitles_open, "the strip never opened");
        let with = p.compose();
        p.subtitle = None;
        let without = p.compose();
        let changed = with.iter().zip(&without).filter(|(a, b)| a != b).count();
        assert!(
            changed > 300,
            "only {changed} pixels differ with the line up — the text is not \
             being drawn"
        );
        // ...and it lands in the band retail clears for it, not over
        // the middle of the picture.
        let outside = with
            .iter()
            .zip(&without)
            .enumerate()
            .filter(|(i, (a, b))| a != b && *i < 159 * MoviePlayer::W)
            .count();
        assert_eq!(outside, 0, "{outside} subtitle pixels above the strip");
    }

    /// Clear the fade-in, then run `seconds` of playback and report
    /// how far the playhead moved.
    fn frames_in(cue: Cue, seconds: f32) -> usize {
        let mut p = MoviePlayer::new(Path::new(BUNDLE), &[cue], false, true).expect("bundle opens");
        for _ in 0..60 {
            p.tick(1.0 / 60.0);
        }
        let before = p.progress().expect("playing").1;
        for _ in 0..(seconds * 200.0) as usize {
            p.tick(1.0 / 200.0);
        }
        p.progress().expect("playing").1 - before
    }
}
