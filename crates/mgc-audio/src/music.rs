//! FLAC music playback: decode a bundle track fully (a few MB — MC1
//! songs and MC2's ~45 s redbook cues both decode in tens of ms) and
//! hand the PCM to the output stream.

use std::path::Path;
use std::sync::Arc;

pub struct DecodedTrack {
    pub pcm: Arc<Vec<i16>>,
    pub channels: u16,
    pub sample_rate: u32,
}

pub fn decode_flac(path: &Path) -> Result<DecodedTrack, String> {
    let mut reader =
        claxon::FlacReader::open(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let info = reader.streaminfo();
    if info.bits_per_sample != 16 {
        return Err(format!(
            "{}: {} bits per sample, engine expects 16",
            path.display(),
            info.bits_per_sample
        ));
    }
    let mut pcm: Vec<i16> =
        Vec::with_capacity(info.samples.unwrap_or(0) as usize * info.channels as usize);
    for s in reader.samples() {
        pcm.push(s.map_err(|e| format!("{}: {e}", path.display()))? as i16);
    }
    Ok(DecodedTrack {
        pcm: Arc::new(pcm),
        channels: info.channels as u16,
        sample_rate: info.sample_rate,
    })
}
