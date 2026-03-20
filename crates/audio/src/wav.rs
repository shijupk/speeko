use anyhow::{Context, Result};
use hound::{SampleFormat, WavReader, WavSpec, WavWriter};
use std::path::Path;

/// Save f32 audio samples to a 16-bit PCM WAV file.
pub fn save_wav(path: &Path, samples: &[f32], sample_rate: u32) -> Result<()> {
    let spec = WavSpec {
        channels: 1,
        sample_rate,
        bits_per_sample: 16,
        sample_format: SampleFormat::Int,
    };

    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("Failed to create directory {:?}", parent))?;
    }

    let mut writer =
        WavWriter::create(path, spec).with_context(|| format!("Failed to create WAV file {:?}", path))?;

    for &sample in samples {
        let clamped = sample.clamp(-1.0, 1.0);
        let i16_sample = (clamped * i16::MAX as f32) as i16;
        writer.write_sample(i16_sample)?;
    }

    writer.finalize()?;
    log::info!("Saved WAV: {:?} ({} samples, {}Hz)", path, samples.len(), sample_rate);
    Ok(())
}

/// Load a WAV file and return f32 samples in [-1.0, 1.0].
/// Converts to mono if stereo; supports 16-bit and 32-bit float formats.
pub fn load_wav(path: &Path) -> Result<(Vec<f32>, u32)> {
    let reader =
        WavReader::open(path).with_context(|| format!("Failed to open WAV file {:?}", path))?;

    let spec = reader.spec();
    let sample_rate = spec.sample_rate;

    let samples: Vec<f32> = match spec.sample_format {
        SampleFormat::Int => {
            let max_val = (1i64 << (spec.bits_per_sample - 1)) as f32;
            reader
                .into_samples::<i32>()
                .filter_map(|s| s.ok())
                .map(|s| s as f32 / max_val)
                .collect()
        }
        SampleFormat::Float => reader
            .into_samples::<f32>()
            .filter_map(|s| s.ok())
            .collect(),
    };

    // Convert to mono by averaging channels if needed.
    let mono = if spec.channels > 1 {
        samples
            .chunks(spec.channels as usize)
            .map(|chunk| chunk.iter().sum::<f32>() / chunk.len() as f32)
            .collect()
    } else {
        samples
    };

    log::debug!(
        "Loaded WAV: {:?} ({} samples, {}Hz, {} channels)",
        path,
        mono.len(),
        sample_rate,
        spec.channels
    );
    Ok((mono, sample_rate))
}
