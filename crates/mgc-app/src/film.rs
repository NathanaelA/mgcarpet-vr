//! `--film`: deterministic frame capture of a level session.
//!
//! A filmed session runs on a FILM CLOCK instead of wall time: every
//! rendered frame advances the sim by exactly `1/rate` of a game turn
//! (`rate` frames per turn, so `rate * TICK_RATE_HZ` fps), and each
//! frame is read back and written as a numbered PNG. Nothing is
//! dropped, nothing is paced by the display, and the same
//! `(take, from, to)` renders the same frames on any machine — a
//! README clip is a replay shot, re-rendered after any visual change
//! by re-running the shot (`tools/film.py`).
//!
//! Before `from` the session SEEKS: bursts of turns with no capture
//! (the window still draws, so the seek is visible). Filming ends at
//! `to`, or when a `--replay` take runs out, and the app exits.
//! Encoding is not the binary's job — ffmpeg does it better.

use std::path::PathBuf;

use mgc_sim::{TICK_DT, TICK_RATE_HZ};

/// Turns advanced per frame while seeking to `from`.
const SEEK_BURST: u64 = 240;

pub struct Film {
    dir: PathBuf,
    from: u64,
    to: u64,
    rate: u32,
    /// Frame within the current turn, `0..rate`; `0` is the frame the
    /// turn steps on.
    subframe: u32,
    /// Frames written so far (= the next frame number).
    frames: u64,
    /// The take ended (or `to` passed): no more frames; the app exits
    /// after the current one.
    done: bool,
    /// The last `step` planned a filmed frame — capture it.
    armed: bool,
}

/// What the frame loop does this frame.
pub enum Step {
    /// Advance `turns` with no capture.
    Seek { turns: u64 },
    /// One filmed frame: set the accumulator to `accumulator` (a
    /// whole `TICK_DT` steps the turn, a fraction only interpolates)
    /// and advance the wall-time clocks by `dt`.
    Frame { accumulator: f32, dt: f32 },
    /// Finished — exit after this frame.
    Done,
}

impl Film {
    pub fn new(dir: PathBuf, from: u64, to: u64, rate: u32) -> Result<Film, String> {
        if to <= from {
            return Err(format!(
                "--film-to ({to}) must be after --film-from ({from})"
            ));
        }
        std::fs::create_dir_all(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
        Ok(Film {
            dir,
            from,
            to,
            rate: rate.max(1),
            subframe: 0,
            frames: 0,
            done: false,
            armed: false,
        })
    }

    pub fn fps(&self) -> u32 {
        self.rate * TICK_RATE_HZ
    }

    /// Plan this frame from the sim's current turn.
    pub fn step(&mut self, tick: u64) -> Step {
        self.armed = false;
        // `to` ends the film on a turn boundary: the last turn's
        // interpolated sub-frames are still filmed.
        if self.done || (tick >= self.to && self.subframe == 0) {
            self.done = true;
            return Step::Done;
        }
        if tick < self.from {
            return Step::Seek {
                turns: (self.from - tick).min(SEEK_BURST),
            };
        }
        let k = self.subframe;
        self.subframe = (k + 1) % self.rate;
        self.armed = true;
        Step::Frame {
            accumulator: if k == 0 {
                TICK_DT
            } else {
                TICK_DT * k as f32 / self.rate as f32
            },
            dt: TICK_DT / self.rate as f32,
        }
    }

    /// The input source ran out (a `--replay` take ended): stop after
    /// the frame in flight.
    pub fn end(&mut self) {
        self.done = true;
    }

    /// Whether the frame planned by the last `step` is captured.
    pub fn capturing(&self) -> bool {
        self.armed
    }

    /// Write one captured frame.
    pub fn write(&mut self, width: u32, height: u32, rgba: &[u8]) -> Result<(), String> {
        let path = self.dir.join(format!("{:06}.png", self.frames));
        super::write_png(&path, width, height, rgba)?;
        self.frames += 1;
        Ok(())
    }

    pub fn summary(&self) -> String {
        format!(
            "film: {} frame(s) at {} fps (turns {}..{}) -> {}",
            self.frames,
            self.fps(),
            self.from,
            self.to,
            self.dir.display()
        )
    }
}
