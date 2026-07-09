//! External General-MIDI renderer: fluidsynth CLI + a GM soundfont.
//!
//! The GM music bake is OPTIONAL — it upgrades the bundle when the
//! host has fluidsynth and a GM soundfont, and is skipped (FM render
//! only, the pre-existing behavior) when it doesn't. Discovery:
//! `MGC_FLUIDSYNTH` / `MGC_SOUNDFONT` env overrides first, then
//! `fluidsynth` on PATH and the usual distro soundfont locations.
//!
//! Rendering shells out per mix: SMF bytes → temp `.mid` → `fluidsynth
//! -ni -O float -F <wav>` → parsed float WAV. Float capture because
//! FluidR3-class fonts peak well past full scale at unity gain
//! (measured 2.3× on CGAME1); loudness normalization happens in the
//! caller, which must scale a base/danger-stem pair by ONE factor to
//! keep the overlay mix valid.

use std::path::{Path, PathBuf};
use std::process::Command;

/// Distro paths a GM soundfont plausibly lives at, tried in order.
const SOUNDFONT_CANDIDATES: &[&str] = &[
    "/usr/share/sounds/sf2/FluidR3_GM.sf2",
    "/usr/share/soundfonts/FluidR3_GM.sf2",
    "/usr/share/soundfonts/default.sf2",
    "/usr/share/sounds/sf2/default-GM.sf2",
    "/usr/share/sounds/sf2/TimGM6mb.sf2",
];

pub struct GmRenderer {
    fluidsynth: PathBuf,
    pub soundfont: PathBuf,
}

impl GmRenderer {
    /// Find fluidsynth + a soundfont; `Err` (with the reason) when the
    /// host can't render GM.
    pub fn locate() -> Result<GmRenderer, String> {
        let fluidsynth = match std::env::var_os("MGC_FLUIDSYNTH") {
            Some(p) => PathBuf::from(p),
            None => which("fluidsynth").ok_or("no fluidsynth on PATH")?,
        };
        let soundfont = match std::env::var_os("MGC_SOUNDFONT") {
            Some(p) => PathBuf::from(p),
            None => SOUNDFONT_CANDIDATES
                .iter()
                .map(Path::new)
                .find(|p| p.exists())
                .ok_or("no GM soundfont found (set MGC_SOUNDFONT)")?
                .to_path_buf(),
        };
        if !soundfont.exists() {
            return Err(format!("soundfont {} does not exist", soundfont.display()));
        }
        Ok(GmRenderer {
            fluidsynth,
            soundfont,
        })
    }

    /// Render SMF bytes to interleaved stereo f32 at `rate` Hz, unity
    /// gain (unnormalized). `scratch` hosts the temp `.mid`/`.wav`
    /// pair (removed on success and failure); `tag` keeps concurrent
    /// bakes apart.
    pub fn render(
        &self,
        midi: &[u8],
        rate: u32,
        scratch: &Path,
        tag: &str,
    ) -> Result<Vec<f32>, String> {
        let mid = scratch.join(format!("{tag}.mid"));
        let wav = scratch.join(format!("{tag}.wav"));
        std::fs::write(&mid, midi).map_err(|e| format!("{}: {e}", mid.display()))?;
        let out = Command::new(&self.fluidsynth)
            .arg("-ni") // no shell, no MIDI-in
            .args(["-r", &rate.to_string()])
            .args(["-O", "float", "-o", "synth.gain=1.0"])
            .arg("-F")
            .arg(&wav)
            .arg(&self.soundfont)
            .arg(&mid)
            .output();
        let _ = std::fs::remove_file(&mid);
        let out = out.map_err(|e| format!("spawn {}: {e}", self.fluidsynth.display()))?;
        let pcm = if out.status.success() {
            std::fs::read(&wav)
                .map_err(|e| format!("{}: {e}", wav.display()))
                .and_then(|w| parse_wav_f32_stereo(&w))
        } else {
            Err(format!(
                "fluidsynth failed: {}",
                String::from_utf8_lossy(&out.stderr)
            ))
        };
        let _ = std::fs::remove_file(&wav);
        pcm
    }
}

fn which(name: &str) -> Option<PathBuf> {
    let paths = std::env::var_os("PATH")?;
    std::env::split_paths(&paths)
        .map(|d| d.join(name))
        .find(|p| p.is_file())
}

/// Minimal RIFF/WAVE reader for fluidsynth's `-O float` output:
/// IEEE-float (format 3), 32-bit. Mono is widened to stereo so the
/// caller always gets interleaved L/R.
fn parse_wav_f32_stereo(data: &[u8]) -> Result<Vec<f32>, String> {
    if data.len() < 12 || &data[..4] != b"RIFF" || &data[8..12] != b"WAVE" {
        return Err("not a RIFF/WAVE stream".into());
    }
    let mut pos = 12;
    let mut channels = 0u16;
    let mut bits = 0u16;
    let mut fmt_tag = 0u16;
    let mut samples: Option<&[u8]> = None;
    while pos + 8 <= data.len() {
        let id = &data[pos..pos + 4];
        let len = u32::from_le_bytes(data[pos + 4..pos + 8].try_into().unwrap()) as usize;
        let body = data
            .get(pos + 8..pos + 8 + len)
            .ok_or("truncated WAV chunk")?;
        match id {
            b"fmt " if len >= 16 => {
                fmt_tag = u16::from_le_bytes(body[0..2].try_into().unwrap());
                channels = u16::from_le_bytes(body[2..4].try_into().unwrap());
                bits = u16::from_le_bytes(body[14..16].try_into().unwrap());
            }
            b"data" => samples = Some(body),
            _ => {}
        }
        pos += 8 + len + (len & 1);
    }
    if fmt_tag != 3 || bits != 32 || !(1..=2).contains(&channels) {
        return Err(format!(
            "unsupported WAV: fmt {fmt_tag}, {bits}-bit, {channels}ch (want IEEE-float mono/stereo)"
        ));
    }
    let body = samples.ok_or("WAV has no data chunk")?;
    let mono = channels == 1;
    let mut pcm = Vec::with_capacity(body.len() / 4 * if mono { 2 } else { 1 });
    for quad in body.chunks_exact(4) {
        let s = f32::from_le_bytes([quad[0], quad[1], quad[2], quad[3]]);
        pcm.push(s);
        if mono {
            pcm.push(s);
        }
    }
    Ok(pcm)
}

#[cfg(test)]
mod tests {
    #[test]
    fn parses_a_minimal_float_wav() {
        let mut w = Vec::new();
        w.extend_from_slice(b"RIFF");
        w.extend_from_slice(&(36u32 + 8).to_le_bytes());
        w.extend_from_slice(b"WAVEfmt ");
        w.extend_from_slice(&16u32.to_le_bytes());
        let fmt: &[u16] = &[3, 1, 0, 0, 0, 0, 4, 32];
        // fmt fields: tag, ch, rate lo/hi, byterate lo/hi, align, bits
        w.extend(fmt.iter().flat_map(|v| v.to_le_bytes()));
        w.extend_from_slice(b"data");
        w.extend_from_slice(&8u32.to_le_bytes());
        w.extend_from_slice(&0.5f32.to_le_bytes());
        w.extend_from_slice(&(-0.25f32).to_le_bytes());
        let pcm = super::parse_wav_f32_stereo(&w).unwrap();
        assert_eq!(pcm, [0.5, 0.5, -0.25, -0.25]); // mono widened
    }
}
