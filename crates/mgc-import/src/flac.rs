//! Thin wrappers over the pure-Rust `flacenc` encoder. Import-time
//! only; the runtime decodes with `claxon`.

use flacenc::component::BitRepr;
use flacenc::error::Verify;

/// Encode interleaved 16-bit PCM to a FLAC stream.
pub fn encode(samples: &[i16], channels: usize, sample_rate: u32) -> Result<Vec<u8>, String> {
    let widened: Vec<i32> = samples.iter().map(|&s| i32::from(s)).collect();
    let config = flacenc::config::Encoder::default()
        .into_verified()
        .map_err(|(_, e)| format!("flac config: {e}"))?;
    let source = flacenc::source::MemSource::from_samples(
        &widened,
        channels,
        16,
        sample_rate as usize,
    );
    let stream = flacenc::encode_with_fixed_block_size(&config, source, config.block_size)
        .map_err(|e| format!("flac encode: {e}"))?;
    let mut sink = flacenc::bitsink::ByteSink::new();
    stream
        .write(&mut sink)
        .map_err(|e| format!("flac write: {e}"))?;
    Ok(sink.as_slice().to_vec())
}

#[cfg(test)]
mod tests {
    #[test]
    fn encodes_a_tone() {
        let samples: Vec<i16> = (0..44100)
            .map(|i| ((i as f32 * 0.0628).sin() * 12000.0) as i16)
            .collect();
        let flac = super::encode(&samples, 1, 44100).unwrap();
        assert_eq!(&flac[..4], b"fLaC");
        assert!(flac.len() < samples.len() * 2, "no compression at all?");
    }
}
