use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// Top-level application configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpeekConfig {
    pub audio: AudioConfig,
    pub dsp: DspConfig,
    pub vad: VadConfig,
    pub mfcc: MfccConfig,
    pub recognizer: RecognizerConfig,
    pub paths: PathsConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AudioConfig {
    /// Sample rate in Hz.
    pub sample_rate: u32,
    /// Recording duration in seconds.
    pub record_duration_secs: f32,
    /// Maximum utterance length in seconds.
    pub max_utterance_secs: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DspConfig {
    /// Pre-emphasis coefficient (0.0 to 1.0).
    pub pre_emphasis: f32,
    /// Frame length in milliseconds.
    pub frame_length_ms: f32,
    /// Frame step (hop) in milliseconds.
    pub frame_step_ms: f32,
    /// FFT size (must be power of 2).
    pub fft_size: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VadConfig {
    /// Number of initial silence frames for threshold estimation.
    pub silence_frames: usize,
    /// Multiplier for silence threshold (mean + factor * stddev).
    pub threshold_factor: f32,
    /// Minimum utterance duration in milliseconds.
    pub min_utterance_ms: u32,
    /// Maximum utterance duration in milliseconds (caps VAD output).
    pub max_utterance_ms: u32,
    /// Hangover duration in milliseconds.
    pub hangover_ms: u32,
    /// Padding around detected speech in milliseconds.
    pub padding_ms: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MfccConfig {
    /// Number of mel filters.
    pub num_mel_filters: usize,
    /// Number of MFCC coefficients to keep.
    pub num_coefficients: usize,
    /// Lower frequency bound in Hz.
    pub low_freq: f32,
    /// Upper frequency bound in Hz.
    pub high_freq: f32,
    /// Append delta and delta-delta coefficients (13 → 39 dims).
    #[serde(default = "default_true")]
    pub use_deltas: bool,
    /// Apply Cepstral Mean Normalization per utterance.
    #[serde(default = "default_true")]
    pub use_cmn: bool,
}

fn default_true() -> bool {
    true
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecognizerConfig {
    /// Confidence threshold for rejection (0.0 to 1.0).
    pub confidence_threshold: f32,
    /// Maximum acceptable DTW distance before absolute rejection.
    pub max_distance: f32,
    /// Sakoe-Chiba band width as fraction of sequence length (0.0 to 1.0).
    pub sakoe_chiba_width: f32,
    /// Minimum number of training samples per word.
    pub min_samples_per_word: usize,
    /// Recommended number of training samples per word.
    pub recommended_samples_per_word: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PathsConfig {
    /// Directory for stored templates.
    pub templates_dir: PathBuf,
    /// Directory for raw WAV recordings.
    pub recordings_dir: PathBuf,
    /// Path to vocabulary file.
    pub vocabulary_file: PathBuf,
}

impl Default for SpeekConfig {
    fn default() -> Self {
        Self {
            audio: AudioConfig {
                sample_rate: 16000,
                record_duration_secs: 3.0,
                max_utterance_secs: 2.0,
            },
            dsp: DspConfig {
                pre_emphasis: 0.97,
                frame_length_ms: 25.0,
                frame_step_ms: 10.0,
                fft_size: 512,
            },
            vad: VadConfig {
                silence_frames: 10,
                threshold_factor: 3.5,
                min_utterance_ms: 200,
                max_utterance_ms: 1500,
                hangover_ms: 100,
                padding_ms: 50,
            },
            mfcc: MfccConfig {
                num_mel_filters: 26,
                num_coefficients: 13,
                low_freq: 0.0,
                high_freq: 8000.0,
                use_deltas: true,
                use_cmn: true,
            },
            recognizer: RecognizerConfig {
                confidence_threshold: 0.3,
                max_distance: 40.0,
                sakoe_chiba_width: 0.2,
                min_samples_per_word: 3,
                recommended_samples_per_word: 5,
            },
            paths: PathsConfig {
                templates_dir: PathBuf::from("data/templates"),
                recordings_dir: PathBuf::from("data/recordings"),
                vocabulary_file: PathBuf::from("vocabulary.txt"),
            },
        }
    }
}

impl SpeekConfig {
    /// Load config from a TOML file, falling back to defaults for missing fields.
    pub fn load(path: &Path) -> anyhow::Result<Self> {
        if path.exists() {
            let content = std::fs::read_to_string(path)?;
            let config: SpeekConfig = toml::from_str(&content)?;
            Ok(config)
        } else {
            log::info!("No config file found at {:?}, using defaults", path);
            Ok(Self::default())
        }
    }

    /// Computed: frame length in samples.
    pub fn frame_length_samples(&self) -> usize {
        (self.audio.sample_rate as f32 * self.dsp.frame_length_ms / 1000.0) as usize
    }

    /// Computed: frame step in samples.
    pub fn frame_step_samples(&self) -> usize {
        (self.audio.sample_rate as f32 * self.dsp.frame_step_ms / 1000.0) as usize
    }
}
