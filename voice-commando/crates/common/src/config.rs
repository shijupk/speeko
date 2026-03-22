use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// Audio configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AudioConfig {
    /// Sample rate in Hz.
    pub sample_rate: u32,
    /// Recording duration in seconds (optional, unused by game continuous capture).
    #[serde(default = "default_record_duration")]
    pub record_duration_secs: f32,
    /// Maximum utterance length in seconds (optional, unused by game continuous capture).
    #[serde(default = "default_max_utterance")]
    pub max_utterance_secs: f32,
}

fn default_record_duration() -> f32 {
    3.0
}
fn default_max_utterance() -> f32 {
    2.0
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
    /// Absolute minimum energy floor. Frames below this are always silence,
    /// regardless of the adaptive threshold. Prevents triggering on ambient noise.
    /// Typical mic noise floor: ~0.0001; speech: ~0.001-0.1.
    #[serde(default = "default_min_energy")]
    pub min_energy: f32,
}

fn default_min_energy() -> f32 {
    0.0005
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
pub struct ClassifierConfig {
    /// Fixed number of MFCC frames for CNN input (pad/truncate to this).
    #[serde(default = "default_max_frames")]
    pub max_frames: usize,
    /// Directory for CNN model files.
    #[serde(default = "default_model_dir")]
    pub model_dir: PathBuf,
}

impl Default for ClassifierConfig {
    fn default() -> Self {
        Self {
            max_frames: default_max_frames(),
            model_dir: default_model_dir(),
        }
    }
}

fn default_max_frames() -> usize {
    100
}
fn default_model_dir() -> PathBuf {
    PathBuf::from("../data/models")
}

/// Simplified top-level config for the game (no DTW, no paths for templates/recordings).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VoiceCommandoConfig {
    pub audio: AudioConfig,
    pub dsp: DspConfig,
    pub vad: VadConfig,
    pub mfcc: MfccConfig,
    #[serde(default)]
    pub classifier: ClassifierConfig,
    #[serde(default)]
    pub recognition: RecognitionConfig,
    #[serde(default)]
    pub game: GameConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecognitionConfig {
    /// Confidence threshold for rejection (0.0 to 1.0).
    #[serde(default = "default_confidence_threshold")]
    pub confidence_threshold: f32,
}

impl Default for RecognitionConfig {
    fn default() -> Self {
        Self {
            confidence_threshold: default_confidence_threshold(),
        }
    }
}

fn default_confidence_threshold() -> f32 {
    0.35
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GameConfig {
    #[serde(default = "default_debounce_ms")]
    pub debounce_ms: u32,
    #[serde(default = "default_energy_drain_per_sec")]
    pub energy_drain_per_sec: f32,
    #[serde(default = "default_energy_gain_per_prey")]
    pub energy_gain_per_prey: f32,
    #[serde(default = "default_prey_interaction_secs")]
    pub prey_interaction_secs: f32,
    #[serde(default = "default_initial_scroll_speed")]
    pub initial_scroll_speed: f32,
    #[serde(default = "default_max_scroll_speed")]
    pub max_scroll_speed: f32,
    #[serde(default)]
    pub audio: GameAudioConfig,
    #[serde(default)]
    pub display: DisplayConfig,
    #[serde(default)]
    pub ring_buffer: RingBufferConfig,
}

impl Default for GameConfig {
    fn default() -> Self {
        Self {
            debounce_ms: default_debounce_ms(),
            energy_drain_per_sec: default_energy_drain_per_sec(),
            energy_gain_per_prey: default_energy_gain_per_prey(),
            prey_interaction_secs: default_prey_interaction_secs(),
            initial_scroll_speed: default_initial_scroll_speed(),
            max_scroll_speed: default_max_scroll_speed(),
            audio: GameAudioConfig::default(),
            display: DisplayConfig::default(),
            ring_buffer: RingBufferConfig::default(),
        }
    }
}

fn default_debounce_ms() -> u32 { 300 }
fn default_energy_drain_per_sec() -> f32 { 0.8 }
fn default_energy_gain_per_prey() -> f32 { 25.0 }
fn default_prey_interaction_secs() -> f32 { 3.5 }
fn default_initial_scroll_speed() -> f32 { 0.15 }
fn default_max_scroll_speed() -> f32 { 0.6 }

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GameAudioConfig {
    #[serde(default = "default_sfx_volume")]
    pub sfx_volume: f32,
    #[serde(default = "default_bgm_volume")]
    pub bgm_volume: f32,
    #[serde(default = "default_true")]
    pub play_bgm: bool,
}

impl Default for GameAudioConfig {
    fn default() -> Self {
        Self {
            sfx_volume: default_sfx_volume(),
            bgm_volume: default_bgm_volume(),
            play_bgm: true,
        }
    }
}

fn default_sfx_volume() -> f32 { 0.7 }
fn default_bgm_volume() -> f32 { 0.3 }

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DisplayConfig {
    #[serde(default)]
    pub fullscreen: bool,
    #[serde(default = "default_resolution")]
    pub resolution: [u32; 2],
    #[serde(default)]
    pub debug_overlay: bool,
}

impl Default for DisplayConfig {
    fn default() -> Self {
        Self {
            fullscreen: false,
            resolution: default_resolution(),
            debug_overlay: false,
        }
    }
}

fn default_resolution() -> [u32; 2] { [1280, 720] }

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RingBufferConfig {
    #[serde(default = "default_buffer_seconds")]
    pub buffer_seconds: f32,
    #[serde(default = "default_recognizer_poll_ms")]
    pub recognizer_poll_ms: u64,
    #[serde(default = "default_post_recognition_cooldown_ms")]
    pub post_recognition_cooldown_ms: u64,
}

impl Default for RingBufferConfig {
    fn default() -> Self {
        Self {
            buffer_seconds: default_buffer_seconds(),
            recognizer_poll_ms: default_recognizer_poll_ms(),
            post_recognition_cooldown_ms: default_post_recognition_cooldown_ms(),
        }
    }
}

fn default_buffer_seconds() -> f32 { 2.0 }
fn default_recognizer_poll_ms() -> u64 { 50 }
fn default_post_recognition_cooldown_ms() -> u64 { 300 }

impl Default for VoiceCommandoConfig {
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
                min_utterance_ms: 150,
                max_utterance_ms: 1200,
                hangover_ms: 80,
                padding_ms: 30,
                min_energy: 0.0005,
            },
            mfcc: MfccConfig {
                num_mel_filters: 26,
                num_coefficients: 13,
                low_freq: 0.0,
                high_freq: 8000.0,
                use_deltas: true,
                use_cmn: true,
            },
            classifier: ClassifierConfig::default(),
            recognition: RecognitionConfig::default(),
            game: GameConfig::default(),
        }
    }
}

impl VoiceCommandoConfig {
    /// Load config from a TOML file, falling back to defaults for missing fields.
    pub fn load(path: &Path) -> anyhow::Result<Self> {
        if path.exists() {
            let content = std::fs::read_to_string(path)?;
            let config: VoiceCommandoConfig = toml::from_str(&content)?;
            Ok(config)
        } else {
            log::info!("No config file found at {:?}, using defaults", path);
            Ok(Self::default())
        }
    }

    /// Save config to a TOML file.
    pub fn save(&self, path: &Path) -> anyhow::Result<()> {
        let content = toml::to_string_pretty(self)?;
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(path, content)?;
        Ok(())
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
