# Technical Design Document (v2)
## Project: Voice Commando (Command Rush)

---

## 1. Document Purpose

This document provides the detailed technical design for **Voice Commando**, a voice-controlled 2D endless runner game written in Rust. It translates the product requirements defined in `product-requirements-game.md` into concrete architecture, module design, data flow, integration patterns, and implementation guidance.

---

## 2. Project Strategy

### 2.1 Two-Project Model

The Speeko workspace contains two distinct deliverables:

| Project | Purpose | Location | Modifiable? |
|---|---|---|---|
| **Speeko CLI** | Training, testing, evaluation, diagnostics | `crates/` (original workspace) | Yes (owner project) |
| **Voice Commando** | Game that demonstrates the trained CNN model | `voice-commando/` (new standalone) | Yes (new project) |

**Workflow:**
1. Use the **Speeko CLI** (`speeko cnn-train`, `speeko test`, etc.) to record samples, train the CNN model, and validate accuracy.
2. Copy the trained model artifacts (`data/models/cnn_model.mpk`, `data/models/cnn_vocab.json`) into the Voice Commando `data/models/` directory.
3. Run **Voice Commando** as a standalone game binary.

### 2.2 Copied Crates — Not Path Dependencies

The game project **copies** the necessary speeko crate source code into its own `crates/` subdirectory. This resolves two critical problems:

1. **API constraints:** The existing `speeko-audio::capture::record_audio()` blocks for a fixed duration. The game needs a **continuous audio stream** writing to a ring buffer. Copying the `audio` crate allows adding a continuous capture API without modifying the original training project.

2. **Independence:** The game project can evolve its audio pipeline, VAD strategy, and feature pipeline independently. The original speeko crates remain untouched for training and testing.

### 2.3 Crates to Copy

Only crates required for **CNN inference at runtime** are copied. Training, DTW, and template storage are **not needed**.

| Original Crate | Copy? | Game Crate Name | Reason |
|---|---|---|---|
| `speeko-common` | **Yes** | `vc-common` | Types (`MfccSequence`, `RecognitionResult`, `WordScore`, `VadRegion`), config structs, error types |
| `speeko-audio` | **Yes** | `vc-audio` | `cpal` mic capture (will be extended with continuous streaming API) |
| `speeko-dsp` | **Yes** | `vc-dsp` | `preprocess`, `fft`, `framing` — required for MFCC pipeline |
| `speeko-vad` | **Yes** | `vc-vad` | `energy_vad` — required for utterance boundary detection |
| `speeko-features` | **Yes** | `vc-features` | `mfcc::MfccExtractor`, `mel`, `cmn`, `delta` — required for feature extraction |
| `speeko-classifier` | **Yes** | `vc-classifier` | `inference::CnnRecognizer`, `model::KeywordCnn`, `dataset::VocabMap` — CNN inference |
| `speeko-recognizer` | **No** | — | DTW-only — not needed for CNN mode |
| `speeko-store` | **No** | — | Template persistence for DTW training — not needed |
| `speeko-app-cli` | **No** | — | CLI app — stays in the original project for training |

**Why rename?** The copied crates are renamed from `speeko-*` to `vc-*` (voice-commando) to avoid Cargo package name collisions. If someone accidentally adds the `voice-commando/` directory to the root Speeko workspace, duplicate `speeko-*` package names would cause a build failure. Renaming eliminates this risk entirely.

### 2.4 Modifications to Copied Crates

The copied crates are modified **only where necessary** for game integration:

| Crate | Modification | Reason |
|---|---|---|
| `vc-audio` | Add `capture::ContinuousCapture` struct | Continuous cpal stream → ring buffer (game needs non-blocking real-time audio) |
| `vc-audio` | Add `capture::choose_input_config` as public | Expose config negotiation for reuse in continuous mode |
| `vc-common` | Make `AudioConfig::record_duration_secs` and `AudioConfig::max_utterance_secs` optional with defaults | These fields are required in the original for CLI recording but unused by the game's continuous capture. Made `Option<f32>` with `#[serde(default)]` so the game config can omit them. |
| `vc-common` | Remove `RecognizerConfig` DTW fields (`sakoe_chiba_width`, `max_distance`, etc.) | Game is CNN-only; DTW config fields are dead weight |
| `vc-common` | Remove `PathsConfig` template/recording path fields | Game doesn't use template storage or recording directories |
| `vc-classifier` | Remove `training.rs`, `augment.rs`, `dataset::load_samples_from_dir` | Game only needs inference — strip training code and heavy `burn` autodiff dependency |
| All | Rename package names from `speeko-*` to `vc-*` in `Cargo.toml` and update inter-crate `path` dependencies | Avoid package name collision with original workspace |

#### Classifier Inference-Only Trimming

The copied `vc-classifier` (originally `speeko-classifier`) is slimmed to inference-only:

**Keep:**
- `model.rs` — `KeywordCnn`, `KeywordCnnConfig`, `pad_or_truncate()`
- `inference.rs` — `CnnRecognizer`
- `dataset.rs` — only `VocabMap` (load/save), remove `load_samples_from_dir`, `prepare_batch`, `train_val_split`

**Remove:**
- `training.rs` — not needed at runtime
- `augment.rs` — training-only data augmentation

**Dependency change:**
```toml
# Original classifier Cargo.toml
burn = { version = "~0.20", features = ["ndarray", "train", "autodiff"] }

# Game classifier Cargo.toml (inference-only)
burn = { version = "~0.20", features = ["ndarray"] }
```

Removing `train` and `autodiff` features significantly reduces compile time and binary size.

### 2.5 Crate Sync Strategy

Copying crates creates a maintenance burden: bug fixes in the original speeko crates must be manually propagated to the game copies. The following process mitigates this:

**Sync policy:** Quarterly diff/merge, or on-demand when a speeko crate receives a bug fix relevant to the game.

**Process:**
1. Keep a `CRATE_SYNC.md` file in `voice-commando/crates/` documenting:
   - Date of last sync per crate
   - Source commit hash from the original speeko repo
   - List of game-specific modifications (delta from original)
2. To sync: run `diff -r` between the original crate and the game copy, excluding known modifications.
3. Apply upstream fixes manually, skipping files that only exist in one side (e.g., `training.rs` removed from `vc-classifier`).

**Which files are modified vs. pristine:**

| Game Crate | Modified Files | Pristine Files (direct copy) |
|---|---|---|
| `vc-common` | `config.rs` (optional fields, removed DTW config) | `types.rs`, `error.rs`, `lib.rs` |
| `vc-audio` | `capture.rs` (added `ContinuousCapture`, exposed helpers) | `wav.rs`, `lib.rs` |
| `vc-dsp` | None | All files |
| `vc-vad` | None | All files |
| `vc-features` | None | All files |
| `vc-classifier` | `lib.rs` (removed training module), `dataset.rs` (trimmed) | `model.rs`, `inference.rs` |

Pristine files can be synced with a direct copy. Modified files require manual merge.

---

## 3. High-Level Architecture

```
┌──────────────────────────────────────────────────────────────────────┐
│                        Voice Commando Game                           │
│                                                                      │
│  ┌───────────────┐   ┌────────────────┐   ┌──────────────────────┐  │
│  │ Audio Capture  │   │  Recognizer    │   │   Bevy App (ECS)     │  │
│  │   Thread       │   │    Thread      │   │                      │  │
│  │                │   │                │   │  CommandInputPlugin   │  │
│  │ cpal stream ──>│──>│ VAD + MFCC +  │──>│  GameplayPlugin      │  │
│  │ ring buffer    │   │ CNN predict   │   │  UIPlugin            │  │
│  │                │   │                │   │  AudioFeedbackPlugin │  │
│  └───────────────┘   └────────────────┘   │  DiagnosticsPlugin   │  │
│                                            │  CalibrationPlugin   │  │
│                                            └──────────────────────┘  │
│                                                                      │
│  ┌──────────────────────────────────────────────────────────────┐    │
│  │  Copied Crates (local to game, renamed vc-*)                 │    │
│  │  vc-audio · vc-dsp · vc-vad · vc-features                   │    │
│  │  vc-classifier (inference-only) · vc-common                  │    │
│  └──────────────────────────────────────────────────────────────┘    │
└──────────────────────────────────────────────────────────────────────┘

┌──────────────────────────────────────────────────────────────────────┐
│  Original Speeko Project (separate, unmodified)                      │
│  Used for: training, testing, evaluation, diagnostics                │
│  Output: trained CNN model files → copied into Voice Commando data/  │
└──────────────────────────────────────────────────────────────────────┘
```

### 3.1 Thread Model

| Thread | Responsibility | Blocking? |
|---|---|---|
| **Audio capture thread** | Runs a continuous `cpal` input stream via `ContinuousCapture`, writes PCM samples into a lock-free SPSC ring buffer | No (callback-driven) |
| **Recognizer thread** | Drains ring buffer, runs sliding-window VAD, DSP preprocessing, MFCC extraction + CMN + deltas, CNN inference. Sends `RecognizedWord` to ECS via `crossbeam` channel | Yes (processing loop) |
| **Bevy main thread** | ECS schedule: reads recognized words, applies command mapping, runs gameplay systems, renders UI | No (frame-driven) |

### 3.2 Data Flow

```
Microphone (cpal callback via ContinuousCapture)
    │
    ▼
Ring Buffer (lock-free SPSC, f32 samples)
    │
    ▼
Recognizer Thread
    ├── Accumulate samples into sliding window
    ├── energy_vad::detect_speech() — utterance boundary detection
    ├── preprocess::preprocess() — DC removal, pre-emphasis, normalization
    ├── MfccExtractor::extract() — MFCC feature extraction
    ├── cmn::normalize() — Cepstral Mean Normalization
    ├── delta::append_deltas_and_double_deltas() — Δ + ΔΔ (13→39 dims)
    ├── CnnRecognizer::predict() — CNN inference via burn NdArray
    │
    ▼
crossbeam::channel::Sender<RecognizedWord>
    │
    ▼
Bevy ECS (Receiver as Resource)
    ├── SpeechInputSystem    → reads channel, emits RecognizedWordEvent / RejectedWordEvent
    ├── CommandMappingSystem → debounce, cooldown, state-aware validation → GameCommandEvent
    ├── GameplaySystem       → applies commands to player state
    ├── UISystem             → updates HUD, feedback overlays
    ├── AudioFeedbackSystem  → plays accept/reject sounds
    └── DiagnosticsSystem    → logs latency, counters
```

---

## 4. Project Structure

```
voice-commando/
├── Cargo.toml                         # Root game manifest
├── config/
│   └── voice_commando.toml            # Game + recognition config
├── assets/
│   ├── sprites/                       # Player, obstacles, backgrounds
│   ├── sounds/                        # SFX and music
│   ├── fonts/                         # UI fonts
│   └── ui/                            # UI assets
├── data/
│   ├── models/                        # CNN model (copied from speeko training output)
│   │   ├── cnn_model.mpk              # Trained model weights
│   │   └── cnn_vocab.json             # Vocabulary map (word <-> class index)
│   └── vocabulary.txt                 # 10-word vocabulary (reference)
│
├── crates/                            # Copied crates (renamed vc-*, modifiable)
│   ├── CRATE_SYNC.md                 # Sync tracking (see Section 2.5)
│   ├── common/                        # vc-common (types, config, errors)
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── types.rs
│   │       ├── config.rs              # Modified: optional fields, no DTW config
│   │       └── error.rs
│   ├── audio/                         # vc-audio + ContinuousCapture
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── capture.rs             # Modified: + ContinuousCapture, exposed helpers
│   │       └── wav.rs
│   ├── dsp/                           # vc-dsp (pristine copy)
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── fft.rs
│   │       ├── framing.rs
│   │       └── preprocess.rs
│   ├── vad/                           # vc-vad (pristine copy)
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       └── energy_vad.rs
│   ├── features/                      # vc-features (pristine copy)
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── mfcc.rs
│   │       ├── mel.rs
│   │       ├── cmn.rs
│   │       └── delta.rs
│   └── classifier/                    # vc-classifier (inference-only trim)
│       ├── Cargo.toml                 # burn without autodiff/train features
│       └── src/
│           ├── lib.rs                 # Modified: removed training module
│           ├── model.rs               # KeywordCnn, pad_or_truncate
│           ├── inference.rs           # CnnRecognizer
│           └── dataset.rs             # Modified: VocabMap only
│
├── src/
│   ├── main.rs                        # Entry point, Bevy App builder
│   ├── app_config.rs                  # Game config loading (VoiceCommandoConfig)
│   ├── app_states.rs                  # Bevy AppState enum
│   │
│   ├── audio_bridge/
│   │   ├── mod.rs
│   │   ├── plugin.rs                  # AudioBridgePlugin
│   │   ├── continuous_capture.rs      # ContinuousCapture wrapper
│   │   ├── recognizer_thread.rs       # CNN recognition loop
│   │   └── ring_buffer.rs            # SPSC ring buffer setup
│   │
│   ├── speech/
│   │   ├── mod.rs
│   │   ├── plugin.rs                  # SpeechPlugin
│   │   ├── events.rs                  # RecognizedWordEvent, RejectedWordEvent
│   │   └── resources.rs               # SpeechReceiver resource
│   │
│   ├── commands/
│   │   ├── mod.rs
│   │   ├── plugin.rs                  # CommandPlugin
│   │   ├── mapping.rs                 # Word → GameCommand mapping
│   │   ├── debounce.rs                # Per-command debounce
│   │   ├── cooldown.rs                # Per-command cooldown timers
│   │   └── types.rs                   # GameCommand enum, GameCommandEvent
│   │
│   ├── game/
│   │   ├── mod.rs
│   │   ├── plugin.rs                  # GamePlugin
│   │   ├── player.rs                  # Player component, state machine
│   │   ├── lanes.rs                   # 3-lane model
│   │   ├── obstacles.rs               # Obstacle spawning, types
│   │   ├── interactions.rs            # Gates, choices, shield events
│   │   ├── collision.rs               # Collision detection
│   │   ├── scoring.rs                 # Score, combo, distance
│   │   ├── difficulty.rs              # Difficulty progression
│   │   └── world.rs                   # World scrolling, segment management
│   │
│   ├── ui/
│   │   ├── mod.rs
│   │   ├── plugin.rs                  # UIPlugin
│   │   ├── menu.rs                    # Main menu screen
│   │   ├── hud.rs                     # In-game HUD
│   │   ├── calibration.rs             # Calibration screen
│   │   ├── settings.rs                # Settings screen
│   │   ├── game_over.rs               # Game over screen
│   │   └── subtitles.rs               # Recognized word subtitles
│   │
│   ├── effects/
│   │   ├── mod.rs
│   │   ├── plugin.rs                  # EffectsPlugin
│   │   ├── sound_fx.rs                # SFX playback
│   │   └── visual_fx.rs              # Screen flash, shake
│   │
│   └── diagnostics/
│       ├── mod.rs
│       ├── plugin.rs                  # DiagnosticsPlugin
│       ├── overlay.rs                 # Debug overlay
│       └── counters.rs                # Performance counters, latency tracker
│
├── benches/
│   └── recognition_pipeline_latency.rs # Full pipeline benchmarks (VAD+DSP+MFCC+CNN)
└── tests/
    ├── command_mapping_tests.rs
    ├── debounce_tests.rs
    └── scoring_tests.rs
```

---

## 5. Cargo.toml Design

### 5.1 Game Root `voice-commando/Cargo.toml`

```toml
[package]
name = "voice-commando"
version = "0.1.0"
edition = "2021"
rust-version = "1.75.0"

[[bin]]
name = "voice-commando"
path = "src/main.rs"

[workspace]
members = [
    "crates/common",
    "crates/audio",
    "crates/dsp",
    "crates/vad",
    "crates/features",
    "crates/classifier",
]

[dependencies]
# --- Copied crates (renamed vc-*, local to game project) ---
vc-common     = { path = "crates/common" }
vc-audio      = { path = "crates/audio" }
vc-dsp        = { path = "crates/dsp" }
vc-vad        = { path = "crates/vad" }
vc-features   = { path = "crates/features" }
vc-classifier = { path = "crates/classifier" }

# --- Game engine ---
bevy = "0.15"

# --- Audio playback (SFX/music) ---
bevy_kira_audio = "0.21"

# --- Serialization & config ---
serde = { version = "1.0", features = ["derive"] }
toml = "0.8"

# --- Error handling ---
thiserror = "2.0"
anyhow = "1.0"

# --- Logging / diagnostics ---
tracing = "0.1"
tracing-subscriber = "0.3"

# --- CLI ---
clap = { version = "4.5", features = ["derive"] }

# --- Concurrency ---
crossbeam-channel = "0.5"
ringbuf = "0.4"

# --- Random (obstacle spawning, etc.) ---
rand = "0.8"

[dev-dependencies]
criterion = "0.5"

[[bench]]
name = "recognition_pipeline_latency"
harness = false
```

### 5.2 Copied Classifier `voice-commando/crates/classifier/Cargo.toml`

```toml
[package]
name = "vc-classifier"
version = "0.1.0"
edition = "2021"
rust-version = "1.75.0"

[dependencies]
vc-common = { path = "../common" }
burn = { version = "~0.20", features = ["ndarray"] }  # inference-only, no autodiff/train
serde = { version = "~1.0", features = ["derive"] }
serde_json = "~1.0"
anyhow = "~1.0"
log = "~0.4"
# rand removed — not needed for inference
```

### 5.3 Workspace Isolation

The Voice Commando project is a **completely separate workspace** from the original Speeko workspace. It is **not** added to the root `speeko/Cargo.toml`. This ensures:
- The original speeko project compiles and tests independently
- The game project has its own dependency resolution
- No accidental cross-contamination

---

## 6. Continuous Audio Capture Design

### 6.1 The Problem

The existing `speeko-audio::capture::record_audio()` is designed for CLI single-shot recording:
- Blocks the calling thread for `duration_secs`
- Returns a complete `Vec<f32>` buffer
- Requires an `AtomicBool` cancel signal to stop early

This is unsuitable for a game that needs continuous, non-blocking audio input.

### 6.2 The Solution: `ContinuousCapture`

Added to the copied `speeko-audio` crate as a new struct:

```rust
// In crates/audio/src/capture.rs (added to the copied vc-audio crate)

use ringbuf::traits::*;

/// Continuous audio capture that writes PCM samples into a ring buffer.
///
/// Unlike `record_audio()`, this runs indefinitely until stopped.
/// The cpal audio callback writes directly into the ring buffer producer,
/// making it safe to use from a real-time audio thread.
pub struct ContinuousCapture {
    _stream: cpal::Stream,
    cancel: Arc<AtomicBool>,
}

impl ContinuousCapture {
    /// Start continuous capture from the default input device.
    ///
    /// Samples are written to `producer` at the negotiated sample rate.
    /// If the ring buffer is full, new samples are dropped (overflow).
    /// Call `stop()` or drop the struct to end capture.
    pub fn start(
        desired_sample_rate: u32,
        mut producer: ringbuf::HeapProd<f32>,
    ) -> Result<Self> {
        let host = cpal::default_host();
        let device = host.default_input_device()
            .ok_or(SpeekError::NoAudioDevice)?;

        let (stream_config, native_channels, native_rate) =
            choose_input_config(&device, desired_sample_rate)?;

        let cancel = Arc::new(AtomicBool::new(false));
        let cancel_clone = Arc::clone(&cancel);

        let err_fn = |err: cpal::StreamError| {
            log::error!("Audio stream error: {}", err);
        };

        let stream = device.build_input_stream(
            &stream_config,
            move |data: &[f32], _: &cpal::InputCallbackInfo| {
                if cancel_clone.load(Ordering::Relaxed) {
                    return;
                }
                // Downmix to mono if needed
                let mono = if native_channels > 1 {
                    downmix_to_mono(data, native_channels)
                } else {
                    data.to_vec()
                };
                // Resample if needed
                let samples = if native_rate != desired_sample_rate {
                    resample(&mono, native_rate, desired_sample_rate)
                } else {
                    mono
                };
                // Write to ring buffer (drop if full)
                let written = producer.push_slice(&samples);
                if written < samples.len() {
                    log::warn!(
                        "Ring buffer overflow: dropped {} samples",
                        samples.len() - written
                    );
                }
            },
            err_fn,
            None,
        )?;

        stream.play()?;
        Ok(Self { _stream: stream, cancel })
    }

    /// Stop the audio capture stream.
    pub fn stop(&self) {
        self.cancel.store(true, Ordering::SeqCst);
    }
}

impl Drop for ContinuousCapture {
    fn drop(&mut self) {
        self.stop();
    }
}
```

### 6.3 Why This Requires a Copied Crate

- `choose_input_config()` is currently a private function in the original `capture.rs`. The game needs it exposed as `pub`.
- `downmix_to_mono()` and `resample()` are also private helpers. They need to be accessible for the continuous capture path.
- Adding `ContinuousCapture` introduces a dependency on `ringbuf`, which doesn't belong in the original training-focused crate.

By copying the crate, we make these minimal changes without touching the original.

---

## 7. Bevy Application States

```rust
#[derive(Debug, Clone, Copy, Default, Eq, PartialEq, Hash, States)]
pub enum AppState {
    #[default]
    Loading,
    MainMenu,
    Calibration,
    Settings,
    Playing,
    Paused,
    GameOver,
}
```

### 7.1 State Transition Diagram

```
Loading ──> MainMenu
               │
       ┌───────┼───────────┐
       ▼       ▼            ▼
  Calibration Settings   Playing
       │       │         │     │
       └───────┘    Paused ◄───┘
                    │
                    ▼
                 Playing
                    │
                    ▼
                 GameOver ──> MainMenu
```

---

## 8. Module Designs

### 8.1 Audio Bridge Module (`audio_bridge/`)

**Purpose:** Manage continuous audio capture and the CNN recognizer thread. Bridge the real-time audio world with the Bevy ECS.

#### 8.1.1 Ring Buffer (`ring_buffer.rs`)

```rust
use ringbuf::{HeapRb, traits::*};

pub fn create_ring_buffer(sample_rate: u32, buffer_secs: f32)
    -> (ringbuf::HeapProd<f32>, ringbuf::HeapCons<f32>)
{
    let capacity = (sample_rate as f32 * buffer_secs) as usize;
    let rb = HeapRb::<f32>::new(capacity);
    rb.split()
}
```

**Sizing rationale:** At 16 kHz mono and `buffer_seconds = 2.0`, the ring buffer holds **32,000 samples** (2 seconds of audio). This provides sufficient headroom for the recognizer thread's ~50ms poll interval plus VAD processing time, while keeping memory usage minimal (~128 KB).

**Overflow policy:** The ring buffer uses a **drop-new** policy: when the buffer is full, the `ContinuousCapture` callback drops incoming samples and logs a warning. This is preferable to dropping oldest samples because it preserves the temporal continuity of already-buffered audio, which VAD and MFCC extraction depend on. Overflow should only occur if the recognizer thread is stalled.

#### 8.1.2 Recognizer Thread (`recognizer_thread.rs`)

Runs a continuous CNN recognition loop:

```rust
pub struct RecognizerThread {
    handle: Option<std::thread::JoinHandle<()>>,
    cancel: Arc<AtomicBool>,
}

pub struct RecognizerConfig {
    pub sample_rate: u32,
    pub dsp: DspConfig,
    pub vad: VadConfig,
    pub mfcc: MfccConfig,
    pub classifier: ClassifierConfig,
    pub confidence_threshold: f32,
    pub poll_interval_ms: u64,
    pub post_recognition_cooldown_ms: u64,
}

impl RecognizerThread {
    pub fn start(
        mut consumer: ringbuf::HeapCons<f32>,
        config: RecognizerConfig,
        sender: crossbeam_channel::Sender<RecognizedWord>,
    ) -> Self {
        let cancel = Arc::new(AtomicBool::new(false));
        let cancel_clone = Arc::clone(&cancel);

        let handle = std::thread::spawn(move || {
            // Initialize CNN recognizer
            let cnn = CnnRecognizer::load(
                &config.classifier.model_dir,
                config.confidence_threshold,
                config.classifier.max_frames,
            ).expect("Failed to load CNN model");

            // Initialize MFCC extractor
            let mut mfcc_extractor = MfccExtractor::new(
                config.sample_rate, &config.dsp, &config.mfcc,
            );

            let frame_length = /* computed from config */;
            let frame_step   = /* computed from config */;

            // Sliding window buffer (max_utterance worth of audio)
            let window_capacity = (config.sample_rate as f32
                * config.vad.max_utterance_ms as f32 / 1000.0) as usize * 2;
            let mut window: Vec<f32> = Vec::with_capacity(window_capacity);

            let poll_interval = Duration::from_millis(config.poll_interval_ms);
            let cooldown_duration = Duration::from_millis(
                config.post_recognition_cooldown_ms
            );
            let mut last_recognition = Instant::now() - cooldown_duration;

            while !cancel_clone.load(Ordering::Relaxed) {
                // 1. Drain samples from ring buffer
                let mut temp = [0.0f32; 1600]; // ~100ms at 16kHz
                let count = consumer.pop_slice(&mut temp);
                if count > 0 {
                    window.extend_from_slice(&temp[..count]);
                    // Cap window size
                    if window.len() > window_capacity {
                        let drain = window.len() - window_capacity;
                        window.drain(..drain);
                    }
                }

                // 2. Check cooldown
                if last_recognition.elapsed() < cooldown_duration {
                    std::thread::sleep(poll_interval);
                    continue;
                }

                // 3. VAD — detect speech in current window
                if let Some(region) = energy_vad::detect_speech(
                    &window, config.sample_rate,
                    frame_length, frame_step, &config.vad,
                ) {
                    let speech = &window[region.start..region.end];

                    // 4. Preprocess
                    let mut samples = speech.to_vec();
                    preprocess::preprocess(&mut samples, config.dsp.pre_emphasis);

                    // 5. MFCC extraction
                    let mfcc = mfcc_extractor.extract(&samples);

                    // 6. Feature transforms (CMN + deltas)
                    let mfcc = if config.mfcc.use_cmn {
                        cmn::normalize(&mfcc)
                    } else { mfcc };
                    let mfcc = if config.mfcc.use_deltas {
                        delta::append_deltas_and_double_deltas(&mfcc)
                    } else { mfcc };

                    // 7. CNN inference
                    let timestamp = Instant::now();
                    let result = cnn.predict(&mfcc);

                    // 8. Send result via channel
                    let _ = sender.try_send(RecognizedWord {
                        result,
                        recognized_at: timestamp,
                    });

                    // 9. Clear window and enter cooldown
                    window.clear();
                    last_recognition = Instant::now();
                }

                std::thread::sleep(poll_interval);
            }
        });

        Self {
            handle: Some(handle),
            cancel,
        }
    }

    pub fn stop(&self) {
        self.cancel.store(true, Ordering::SeqCst);
    }
}
```

#### 8.1.3 Sliding Window VAD Strategy

```
Time ──────────────────────────────────────────────>
Audio: ░░░░░██████░░░░░░░░░░██████████░░░░░░░░░░░░
              ▲                  ▲
              │                  │
         Utterance 1        Utterance 2
              │                  │
              ▼                  ▼
       RecognizedWord      RecognizedWord
       { word: "up",       { word: "left",
         confidence: 0.87,   confidence: 0.92,
         timestamp }         timestamp }
```

1. **Accumulate:** Drain ring buffer into sliding window every `poll_interval_ms` (~50ms).
2. **Detect:** Run `energy_vad::detect_speech()` on the window.
3. **Process:** On speech detected → preprocess → MFCC → CMN → deltas → CNN predict.
4. **Cooldown:** Clear window, wait `post_recognition_cooldown_ms` (~300ms) before next detection.

#### 8.1.4 Plugin (`plugin.rs`)

```rust
pub struct AudioBridgePlugin;

impl Plugin for AudioBridgePlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(AppState::Loading), setup_audio_bridge);
    }
}

fn setup_audio_bridge(mut commands: Commands, config: Res<VoiceCommandoConfig>) {
    let sample_rate = config.audio.sample_rate;
    let buffer_secs = config.game.ring_buffer.buffer_seconds;

    // Create ring buffer
    let (producer, consumer) = create_ring_buffer(sample_rate, buffer_secs);

    // Start continuous audio capture
    let capture = ContinuousCapture::start(sample_rate, producer)
        .expect("Failed to start audio capture");

    // Create channel for recognizer → ECS communication
    let (sender, receiver) = crossbeam_channel::bounded(16);

    // Start recognizer thread
    let recognizer = RecognizerThread::start(consumer, /* config */, sender);

    // Insert resources
    commands.insert_resource(SpeechReceiver { receiver });
    commands.insert_resource(AudioBridgeState { capture, recognizer });
}

/// Capture Lifecycle and Bevy State Transitions
///
/// The ContinuousCapture and RecognizerThread run for the **entire lifetime**
/// of the application (from Loading to app exit). They are NOT paused during
/// state transitions (e.g., Playing → Paused → MainMenu). This is intentional:
///
/// - The mic stays hot so calibration mode works instantly.
/// - The recognizer thread continues producing RecognizedWordEvents.
/// - State-aware command validation in CommandMappingSystem filters out
///   commands that are invalid for the current AppState (e.g., Jump while Paused).
///
/// Cleanup: When the Bevy app exits, AudioBridgeState is dropped, which
/// drops ContinuousCapture (triggering its Drop impl → stop()) and
/// signals the recognizer thread to cancel.
```

---

### 8.2 Speech Module (`speech/`)

#### 8.2.1 Wire Types

```rust
/// Sent from recognizer thread to ECS via crossbeam channel.
pub struct RecognizedWord {
    pub result: RecognitionResult,  // from speeko-common
    pub recognized_at: Instant,
}
```

#### 8.2.2 Events

```rust
#[derive(Event, Debug, Clone)]
pub struct RecognizedWordEvent {
    pub word: String,
    pub confidence: f32,
    pub all_scores: Vec<(String, f32)>,
    pub recognized_at: Instant,
}

#[derive(Event, Debug, Clone)]
pub struct RejectedWordEvent {
    pub best_guess: Option<String>,
    pub confidence: f32,
    pub reason: RejectionReason,
    pub recognized_at: Instant,
}

#[derive(Debug, Clone)]
pub enum RejectionReason {
    LowConfidence,
    NoSpeechDetected,
}
```

#### 8.2.3 System

```rust
fn speech_input_system(
    receiver: Res<SpeechReceiver>,
    mut recognized_events: EventWriter<RecognizedWordEvent>,
    mut rejected_events: EventWriter<RejectedWordEvent>,
) {
    while let Ok(word) = receiver.receiver.try_recv() {
        match word.result.word {
            Some(w) => recognized_events.send(RecognizedWordEvent {
                word: w,
                confidence: word.result.confidence,
                all_scores: word.result.scores.iter()
                    .map(|s| (s.word.clone(), s.distance))
                    .collect(),
                recognized_at: word.recognized_at,
            }),
            None => rejected_events.send(RejectedWordEvent {
                best_guess: word.result.scores.first().map(|s| s.word.clone()),
                confidence: word.result.confidence,
                reason: RejectionReason::LowConfidence,
                recognized_at: word.recognized_at,
            }),
        }
    }
}
```

---

### 8.3 Commands Module (`commands/`)

#### 8.3.1 Types

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum GameCommand {
    Jump,
    Slide,
    MoveLeft,
    MoveRight,
    Freeze,
    Resume,
    OpenGate,
    CloseShield,
    AcceptChoice,
    RejectChoice,
}

#[derive(Event, Debug, Clone)]
pub struct GameCommandEvent {
    pub command: GameCommand,
    pub source_word: String,
    pub confidence: f32,
    pub issued_at: Instant,
}
```

#### 8.3.2 Mapping

```rust
pub fn map_word_to_command(word: &str) -> Option<GameCommand> {
    match word {
        "up"    => Some(GameCommand::Jump),
        "down"  => Some(GameCommand::Slide),
        "left"  => Some(GameCommand::MoveLeft),
        "right" => Some(GameCommand::MoveRight),
        "stop"  => Some(GameCommand::Freeze),
        "start" => Some(GameCommand::Resume),
        "open"  => Some(GameCommand::OpenGate),
        "close" => Some(GameCommand::CloseShield),
        "yes"   => Some(GameCommand::AcceptChoice),
        "no"    => Some(GameCommand::RejectChoice),
        _       => None,
    }
}
```

#### 8.3.3 Debounce

```rust
#[derive(Resource)]
pub struct DebounceState {
    last_accepted: HashMap<GameCommand, Instant>,
    pub debounce_duration: Duration,
}

impl DebounceState {
    pub fn can_accept(&self, command: GameCommand, now: Instant) -> bool {
        match self.last_accepted.get(&command) {
            Some(&last) => now.duration_since(last) >= self.debounce_duration,
            None => true,
        }
    }

    pub fn record_accept(&mut self, command: GameCommand, now: Instant) {
        self.last_accepted.insert(command, now);
    }
}
```

#### 8.3.4 Cooldown

```rust
#[derive(Resource)]
pub struct CooldownState {
    cooldowns: HashMap<GameCommand, Timer>,
}

impl CooldownState {
    pub fn is_ready(&self, command: GameCommand) -> bool {
        self.cooldowns.get(&command).map_or(true, |t| t.finished())
    }

    pub fn start_cooldown(&mut self, command: GameCommand, duration: Duration) {
        self.cooldowns.insert(command, Timer::new(duration, TimerMode::Once));
    }

    pub fn tick_all(&mut self, delta: Duration) {
        for timer in self.cooldowns.values_mut() {
            timer.tick(delta);
        }
    }
}
```

#### 8.3.5 State-Aware Validation Rules

| Command | Valid When |
|---|---|
| `Jump` | Playing, player is grounded |
| `Slide` | Playing, player is grounded |
| `MoveLeft` | Playing, current lane > 0 (Left) |
| `MoveRight` | Playing, current lane < 2 (Right) |
| `Freeze` | Playing (triggers freeze/brace or pause) |
| `Resume` | Paused, GameOver (restart), or MainMenu (start game) |
| `OpenGate` | Playing, near a gate entity |
| `CloseShield` | Playing, shield/barrier interaction active |
| `AcceptChoice` | Playing, choice prompt active |
| `RejectChoice` | Playing, choice prompt active |

---

### 8.4 Game Module (`game/`)

#### 8.4.1 Player State Machine

```rust
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlayerAction {
    Running,
    Jumping,
    Sliding,
    Frozen,
    LaneChanging,
    Dead,
}

#[derive(Component, Debug)]
pub struct Player {
    pub lane: Lane,
    pub action: PlayerAction,
    pub action_timer: Timer,
    pub invincible_timer: Option<Timer>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Lane {
    Left   = 0,
    Center = 1,
    Right  = 2,
}
```

**Action durations (configurable):**

| Action | Duration | Notes |
|---|---|---|
| Jump | 600ms | Arc up and back down |
| Slide | 500ms | Duck under obstacle |
| Lane change | 200ms | Smooth slide to adjacent lane |
| Freeze | 400ms | Brief invincibility window |

#### 8.4.2 Lane Model

```rust
pub const LANE_WIDTH: f32 = 2.0;
pub const LANE_POSITIONS: [f32; 3] = [-LANE_WIDTH, 0.0, LANE_WIDTH];

pub fn lane_x(lane: Lane) -> f32 {
    LANE_POSITIONS[lane as usize]
}
```

#### 8.4.3 Obstacles

```rust
#[derive(Component, Debug)]
pub enum ObstacleType {
    LowBarrier,                                       // Requires Jump (up)
    HighBarrier,                                      // Requires Slide (down)
    LaneBlocker { lane: Lane },                       // Requires lane change (left/right)
    Gate,                                             // Requires Open command
    Projectile,                                       // Requires Close command (shield)
    ChoiceFork { left_reward: i32, right_reward: i32 }, // Requires Yes/No
}

#[derive(Resource)]
pub struct ObstacleSpawner {
    pub spawn_timer: Timer,
    pub spawn_distance: f32,
    pub min_gap: f32,
    pub active_count_limit: u32,
}
```

#### 8.4.4 Scoring

```rust
#[derive(Resource, Debug)]
pub struct Score {
    pub distance: f32,
    pub points: i32,
    pub combo: u32,
    pub max_combo: u32,
    pub commands_accepted: u32,
    pub commands_rejected: u32,
    pub perfect_timings: u32,
}

impl Score {
    pub fn on_successful_command(&mut self, is_perfect: bool) {
        self.combo += 1;
        self.max_combo = self.max_combo.max(self.combo);
        self.commands_accepted += 1;
        let combo_multiplier = 1 + (self.combo / 5);
        self.points += 10 * combo_multiplier as i32;
        if is_perfect {
            self.points += 5;
            self.perfect_timings += 1;
        }
    }

    pub fn on_failed_command(&mut self) {
        self.combo = 0;
        self.commands_rejected += 1;
    }
}
```

#### 8.4.5 Difficulty

```rust
#[derive(Resource)]
pub struct Difficulty {
    pub scroll_speed: f32,
    pub obstacle_rate: f32,
    pub contextual_chance: f32,
    pub time_elapsed: f32,
}

impl Difficulty {
    pub fn update(&mut self, delta: f32) {
        self.time_elapsed += delta;
        self.scroll_speed = (5.0 + self.time_elapsed / 30.0 * 0.5).min(15.0);
        self.obstacle_rate = (0.8 + self.time_elapsed / 60.0 * 0.4).min(2.0);
        self.contextual_chance = if self.time_elapsed > 30.0 { 0.2 } else { 0.0 };
    }
}
```

---

### 8.5 UI Module (`ui/`)

#### 8.5.1 HUD Layout

```
┌────────────────────────────────────────────────────┐
│  Score: 1250    Combo: 7x    Distance: 420m        │
│                                                    │
│            [Game Viewport — 3 lanes]               │
│                                                    │
│  ┌──────────────────────────────────────────────┐  │
│  │  Heard: UP (94%) → Jump ✓                    │  │
│  └──────────────────────────────────────────────┘  │
└────────────────────────────────────────────────────┘
```

#### 8.5.2 Calibration Screen

```
┌────────────────────────────────────────────────────┐
│                   CALIBRATION                      │
│                                                    │
│  Mic: Default Input Device                         │
│  Activity: ████████░░░░░░░░░░░░░░░░░  (0.34)      │
│                                                    │
│  Last Word: "start"   Confidence: 87%  ✓ Accepted  │
│                                                    │
│  Word Test Matrix:                                 │
│  ┌────────┬─────────┬──────────┬────────────────┐  │
│  │ Word   │ Tested  │ Accepted │ Avg Confidence │  │
│  ├────────┼─────────┼──────────┼────────────────┤  │
│  │ start  │    3    │    3     │     91%        │  │
│  │ stop   │    2    │    2     │     85%        │  │
│  │ up     │    5    │    4     │     88%        │  │
│  └────────┴─────────┴──────────┴────────────────┘  │
│                                                    │
│  Threshold: [====|======] 0.30                     │
│  Debounce:  [==|========] 300ms                    │
│                                                    │
│  [Save Settings]    [Back to Menu]                 │
└────────────────────────────────────────────────────┘
```

#### 8.5.3 Settings Screen (`settings.rs`)

The settings screen allows the user to adjust runtime parameters and **persist them back to disk**.

**Configurable settings:**

| Setting | Control | Persisted? |
|---|---|---|
| Confidence threshold | Slider (0.10 — 0.80) | Yes |
| Debounce duration | Slider (100ms — 800ms) | Yes |
| SFX volume | Slider (0.0 — 1.0) | Yes |
| BGM volume | Slider (0.0 — 1.0) | Yes |
| BGM on/off | Toggle | Yes |
| Fullscreen / Windowed | Toggle | Yes |
| Debug overlay | Toggle | Yes |

**Persistence mechanism:**

```rust
impl VoiceCommandoConfig {
    /// Save the current config to the TOML file.
    pub fn save(&self, path: &Path) -> anyhow::Result<()> {
        let content = toml::to_string_pretty(self)?;
        std::fs::write(path, content)?;
        Ok(())
    }
}
```

When the user presses "Save Settings", the current `VoiceCommandoConfig` resource is serialized to `voice_commando.toml`. Settings take effect immediately (Bevy resources are updated in-place). The fullscreen toggle calls `window.mode = if fullscreen { WindowMode::Fullscreen } else { WindowMode::Windowed }` on the primary window.

---

### 8.6 Effects Module (`effects/`)

```rust
#[derive(Resource)]
pub struct GameSounds {
    pub command_accepted: Handle<AudioSource>,
    pub command_rejected: Handle<AudioSource>,
    pub jump: Handle<AudioSource>,
    pub slide: Handle<AudioSource>,
    pub gate_open: Handle<AudioSource>,
    pub shield_close: Handle<AudioSource>,
    pub collision: Handle<AudioSource>,
    pub game_over: Handle<AudioSource>,
    pub combo_milestone: Handle<AudioSource>,
    pub bgm: Handle<AudioSource>,
}
```

**SFX triggering rules:**

| Event | Sound | When |
|---|---|---|
| Command accepted | `command_accepted` (short chirp) | `GameCommandEvent` emitted |
| Command rejected (low confidence) | `command_rejected` (soft buzz) | `RejectedWordEvent` emitted during Playing state |
| Jump action | `jump` (whoosh) | Player enters `Jumping` state |
| Slide action | `slide` (swoosh) | Player enters `Sliding` state |
| Gate opened | `gate_open` (creak) | Gate entity transitions to open |
| Shield activated | `shield_close` (clang) | Shield buff applied |
| Collision / hit | `collision` (thud) | Collision system detects hit |
| Game over | `game_over` (descending tone) | Transition to `GameOver` state |
| Combo milestone (5x, 10x, etc.) | `combo_milestone` (chime) | `Score::combo` crosses a 5x boundary |
| Background music | `bgm` (loop) | Playing state, continuous |

**Design note:** SFX volumes default to low to minimize mic interference. Accepted/rejected sounds must be distinct and short (<200ms) to avoid overlapping with the next voice command.

---

### 8.7 Diagnostics Module (`diagnostics/`)

#### Debug Overlay (F3 toggle)

```
┌─ Debug ─────────────────────────────────────┐
│ FPS: 60.0  Frame: 1.2ms                     │
│ CNN Recognizer: idle (last: 32ms)            │
│ Ring buffer: 2400/32000 samples              │
│ Last word: "up" (0.94) → Jump [Accepted]    │
│ Commands: 24 accepted / 3 rejected           │
│ Player: Running, Lane: Center                │
│ Obstacles: 4 active                          │
│ Latency: voice→action ~120ms                 │
└──────────────────────────────────────────────┘
```

#### Latency Tracking

```rust
#[derive(Resource)]
pub struct LatencyTracker {
    samples: VecDeque<Duration>,
    capacity: usize,
}

impl LatencyTracker {
    pub fn record(&mut self, latency: Duration) { /* ... */ }
    pub fn average(&self) -> Duration { /* ... */ }
    pub fn p95(&self) -> Duration { /* ... */ }
}
```

---

## 9. Configuration Design

```toml
# voice_commando.toml

[audio]
sample_rate = 16000

[dsp]
pre_emphasis = 0.97
frame_length_ms = 25.0
frame_step_ms = 10.0
fft_size = 512

[vad]
silence_frames = 10
threshold_factor = 3.5
min_utterance_ms = 150
max_utterance_ms = 1200
hangover_ms = 80
padding_ms = 30

[mfcc]
num_mel_filters = 26
num_coefficients = 13
low_freq = 0.0
high_freq = 8000.0
use_deltas = true
use_cmn = true

[classifier]
max_frames = 100
model_dir = "data/models"

[recognition]
confidence_threshold = 0.35

[game]
debounce_ms = 300
jump_duration_ms = 600
slide_duration_ms = 500
lane_change_duration_ms = 200
freeze_duration_ms = 400
initial_scroll_speed = 5.0
max_scroll_speed = 15.0

[game.audio]
sfx_volume = 0.7
bgm_volume = 0.3
play_bgm = true

[game.display]
fullscreen = false
resolution = [1280, 720]
debug_overlay = false

[game.ring_buffer]
buffer_seconds = 2.0
recognizer_poll_ms = 50
post_recognition_cooldown_ms = 300
```

**Note:** The game config is a **simplified** version. It does not include DTW-specific fields (`sakoe_chiba_width`, `max_distance`, `min_samples_per_word`, `recommended_samples_per_word`) or path fields for templates/recordings. The game defines its own config struct rather than reusing `SpeekConfig` directly.

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VoiceCommandoConfig {
    pub audio: AudioConfig,
    pub dsp: DspConfig,
    pub vad: VadConfig,
    pub mfcc: MfccConfig,
    pub classifier: ClassifierConfig,
    pub recognition: RecognitionConfig,
    pub game: GameConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecognitionConfig {
    pub confidence_threshold: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GameConfig {
    pub debounce_ms: u32,
    pub jump_duration_ms: u32,
    pub slide_duration_ms: u32,
    pub lane_change_duration_ms: u32,
    pub freeze_duration_ms: u32,
    pub initial_scroll_speed: f32,
    pub max_scroll_speed: f32,
    pub audio: GameAudioConfig,
    pub display: DisplayConfig,
    pub ring_buffer: RingBufferConfig,
}
```

---

## 10. Bevy Plugin Registration and System Ordering

```rust
fn main() {
    let cli = Cli::parse();
    let config = VoiceCommandoConfig::load(&cli.config).unwrap_or_default();

    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "Voice Commando — Command Rush".into(),
                resolution: (1280.0, 720.0).into(),
                ..default()
            }),
            ..default()
        }))
        .init_state::<AppState>()
        .insert_resource(config)
        .add_plugins(AudioBridgePlugin)
        .add_plugins(SpeechPlugin)
        .add_plugins(CommandPlugin)
        .add_plugins(GamePlugin)
        .add_plugins(UIPlugin)
        .add_plugins(EffectsPlugin)
        .add_plugins(DiagnosticsPlugin)
        .run();
}
```

### System Ordering

```
Schedule: Update
│
├── 1. SpeechInputSystem        (reads channel → RecognizedWordEvent)
├── 2. CommandMappingSystem     (RecognizedWordEvent → GameCommandEvent)
├── 3. CooldownTickSystem       (tick all cooldowns)
├── 4. GameCommandSystem        (apply GameCommandEvent to player)
├── 5. PhysicsSystem            (movement, scrolling)
├── 6. ObstacleSpawnSystem      (spawn new obstacles)
├── 7. CollisionSystem          (check collisions)
├── 8. ScoringSystem            (update score, combo)
├── 9. DifficultySystem         (ramp difficulty)
├── 10. UIUpdateSystem          (update HUD, overlays)
├── 11. AudioFeedbackSystem     (play SFX based on events)
└── 12. DiagnosticsSystem       (update counters, latency)
```

---

## 11. CNN Recognition Pipeline

### 11.1 Pipeline (Game Only)

```rust
// In recognizer_thread.rs — CNN-only pipeline:

// 1. VAD: detect speech region in sliding window
let region = energy_vad::detect_speech(&window, sample_rate, ...);

// 2. Preprocess: DC removal → pre-emphasis → normalization
let mut samples = speech.to_vec();
preprocess::preprocess(&mut samples, dsp_config.pre_emphasis);

// 3. MFCC: extract 13-coefficient features per frame
let mfcc = mfcc_extractor.extract(&samples);

// 4. CMN: cepstral mean normalization
let mfcc = cmn::normalize(&mfcc);

// 5. Deltas: append Δ + ΔΔ → 39 dimensions per frame
let mfcc = delta::append_deltas_and_double_deltas(&mfcc);

// 6. CNN: pad/truncate to max_frames, run inference
let result = cnn.predict(&mfcc);
// result.word = Some("up") | None (rejected)
// result.confidence = 0.0..1.0
```

### 11.2 Model Loading

**Runtime-only loading:** The CNN model is loaded from disk at application startup by `CnnRecognizer::load()`. It is **not** embedded via `include_bytes!` or any compile-time mechanism. This means:
- The game compiles and builds successfully even if `data/models/` is empty.
- If the model files are missing at runtime, the game shows a clear error: *"CNN model not found. Train it using the speeko CLI: `speeko cnn-train`"*.

**No hot-reload:** The model is loaded once when the recognizer thread starts. If the user retrains the CNN model while the game is running, a **restart is required** to pick up the new model. Hot-reload is a future consideration.

### 11.3 Timing Budget

| Stage | Target Latency | Notes |
|---|---|---|
| VAD detection | < 5ms | Energy-based, simple arithmetic |
| DSP preprocessing | < 1ms | In-place operations on ~8000 samples |
| MFCC extraction | < 10ms | ~50-100 frames, reusable FFT planner |
| CMN + deltas | < 2ms | Simple per-frame arithmetic |
| CNN inference | < 15ms | NdArray backend, 3-conv + 2-FC, small model |
| **Total pipeline** | **< 35ms** | Well within frame budget |

### 11.4 End-to-End Latency Estimate

Latency measured **after the user finishes speaking** (utterance end to game action visible):

| Component | Latency | Cumulative |
|---|---|---|
| VAD trailing hangover | ~80ms | 80ms |
| Ring buffer → recognizer poll | ~50ms | 130ms |
| Recognition pipeline (VAD+DSP+MFCC+CNN) | ~35ms | 165ms |
| Channel → ECS read (next frame) | < 1ms | 166ms |
| Command mapping + action | < 1ms | 167ms |
| **Voice-to-action total** | | **~170ms** |

Note: Utterance duration (~300-600ms while the user speaks) is not included — that is user time, not system latency.

---

## 12. Interaction Mechanic Designs

### 12.1 Gate Interaction (open)

```
Player approaches gate → UI: "Say OPEN to pass"
  ├── "open" → Gate opens, player passes
  └── timeout → Player collides (game over)
```

### 12.2 Choice Interaction (yes/no)

```
Fork in path → UI: "YES (left: +50pts) or NO (right: shield)"
  ├── "yes" → Left path, +50 points
  └── "no"  → Right path, shield buff
```

### 12.3 Shield Interaction (close)

```
Projectile incoming → UI: "Say CLOSE for shield!"
  ├── "close" → Shield activates (400ms invincibility)
  └── timeout → Player hit (damage/game over)
```

---

## 13. Error Handling

```rust
#[derive(Debug, thiserror::Error)]
pub enum VoiceRunnerError {
    #[error("No audio input device found")]
    NoAudioDevice,

    #[error("Configuration error: {0}")]
    Config(String),

    #[error("CNN model loading failed: {0}")]
    ModelLoad(String),

    #[error("Recognizer thread panicked")]
    RecognizerPanic,

    #[error("Audio capture thread panicked")]
    CapturePanic,

    #[error("Ring buffer overflow — audio samples dropped")]
    RingBufferOverflow,
}
```

### Graceful Degradation

| Failure | Behavior |
|---|---|
| No microphone found | Show error screen, offer retry |
| Microphone disconnects | Pause game, show reconnect prompt |
| Ring buffer overflow | Log warning, new samples dropped (self-healing, preserves buffered continuity) |
| Recognizer thread panic | Show error, offer restart |
| CNN model missing | Show error: "Run speeko cnn-train first" |
| Config file missing | Use defaults, log warning |

---

## 14. Testing Strategy

### Unit Tests

| Module | Tests |
|---|---|
| `commands/mapping.rs` | All 10 word→command mappings, unknown returns None |
| `commands/debounce.rs` | Window enforcement, independent per-command |
| `commands/cooldown.rs` | Timer tick, ready/not-ready states |
| `game/scoring.rs` | Increment, combo multiplier, reset |
| `game/lanes.rs` | Bounds checking, positions |
| `game/difficulty.rs` | Ramp thresholds, caps |
| `game/player.rs` | State transitions, invalid transitions |

### Performance Tests (Criterion)

```rust
fn bench_cnn_inference(c: &mut Criterion) {
    let cnn = CnnRecognizer::load(model_dir, 0.35, 100).unwrap();
    let mfcc = /* synthetic 39-dim, 50-frame sequence */;
    c.bench_function("cnn_predict", |b| {
        b.iter(|| cnn.predict(&mfcc))
    });
}

fn bench_mfcc_extraction(c: &mut Criterion) {
    let samples = /* 0.5s of audio */;
    c.bench_function("mfcc_extract", |b| {
        b.iter(|| extractor.extract(&samples))
    });
}
```

### Manual Test Scenarios

| Scenario | Expected |
|---|---|
| Speak all 10 words | Each recognized and mapped correctly |
| Rapid speech ("up up up") | Debounce prevents duplicates |
| Noisy room | Threshold rejects false positives |
| Unplug microphone | Game pauses, shows message |
| 10+ minute session | Stable FPS, no memory growth |
| Unrecognized word ("hello") | Rejected, no action |

---

## 15. Library Selection Summary

| Concern | Library | Version | Justification |
|---|---|---|---|
| Game engine | `bevy` | 0.15 | Rust-native ECS, plugin architecture, 2D rendering |
| Mic input | `cpal` | 0.15 | Cross-platform, already in speeko-audio |
| CNN inference | `burn` (ndarray) | 0.20 | Already used by speeko-classifier, no-autodiff build |
| Game audio | `bevy_kira_audio` | 0.21 | Bevy-integrated SFX/music |
| Config | `serde` + `toml` | 1.0 / 0.8 | Already in speeko-common |
| Errors | `thiserror` + `anyhow` | 2.0 / 1.0 | Typed errors + boundary convenience |
| Logging | `tracing` | 0.1 | Structured spans for latency |
| CLI | `clap` | 4.5 | Launch flags (--config, --debug) |
| Ring buffer | `ringbuf` | 0.4 | Lock-free SPSC for audio callback |
| Channel | `crossbeam-channel` | 0.5 | Recognizer → ECS communication |
| RNG | `rand` | 0.8 | Obstacle spawning |

### 15.1 Alternatives Considered

| Concern | Primary | Alternative | Tradeoff |
|---|---|---|---|
| Game engine | `bevy` 0.15 | `macroquad` | Simpler API, but no ECS or plugin system — harder to scale modularly |
| Game audio | `bevy_kira_audio` 0.21 | `rodio` | Lower-level, not Bevy-integrated — requires manual channel management |
| Ring buffer | `ringbuf` 0.4 | `crossbeam` deque | More powerful work-stealing deque, but SPSC ring buffer is sufficient and simpler for audio callback |
| Logging | `tracing` 0.1 | `log` + `env_logger` | `log` is already used in speeko crates; `tracing` adds structured spans for latency measurement |
| Config format | `toml` 0.8 | `serde_json` | JSON is machine-friendly but worse for hand-editing config files |
| CNN framework | `burn` (ndarray) | `onnxruntime` | ONNX would decouple from training framework but adds a C++ dependency and complicates the Rust-only constraint |
| Channel | `crossbeam-channel` | `std::sync::mpsc` | std mpsc is adequate but crossbeam offers `try_recv` without `TryRecvError` complexity and better performance |

---

## 16. Risks and Mitigations

| Risk | Impact | Likelihood | Mitigation |
|---|---|---|---|
| SFX/BGM interferes with mic | Accuracy drops | High | Low default volume, separate output/input devices |
| False positives during gameplay | Unintended actions | High | State-aware validation, confidence threshold, debounce |
| Bevy API churn | Breakage on upgrade | Medium | Pin version, isolate in plugins |
| Ring buffer overflow | Missed words | Low | 2s buffer, log warnings, self-healing |
| Speech fatigue | User discomfort | Medium | Sparse command design, no rapid-fire required |
| CNN model not trained | Game won't start | Medium | Clear error message pointing to speeko CLI |

---

## 17. Phased Roadmap

### Phase 1: Technical Skeleton (Week 1-2)

- [ ] Create `voice-commando/` project with workspace and copied crates
- [ ] Trim classifier to inference-only (remove training, augment, autodiff)
- [ ] Add `ContinuousCapture` to copied audio crate
- [ ] Bevy app with `AppState` state machine and main menu
- [ ] Audio bridge: ring buffer + continuous capture + recognizer thread
- [ ] `crossbeam` channel → Bevy event bridge
- [ ] Basic HUD showing last recognized word + confidence
- [ ] Keyboard fallback for testing (press keys to simulate commands)

### Phase 2: Gameplay MVP (Week 3-4)

- [ ] 3-lane runner with auto-scrolling world
- [ ] Player: jump, slide, lane change
- [ ] Obstacle spawning: LowBarrier, HighBarrier, LaneBlocker
- [ ] Collision detection and game over
- [ ] `start` → begin run, `stop` → pause
- [ ] Score + distance + basic difficulty ramp

### Phase 3: Full Command Integration (Week 5-6)

- [ ] Gate mechanic (`open`)
- [ ] Shield mechanic (`close`)
- [ ] Choice fork mechanic (`yes`, `no`)
- [ ] Combo system
- [ ] Debounce and cooldown tuning
- [ ] SFX for commands
- [ ] Calibration screen with per-word stats

### Phase 4: Polish (Week 7-8)

- [ ] Visual polish: sprites, parallax, effects
- [ ] Settings screen with persistence
- [ ] Debug overlay (F3)
- [ ] Performance profiling
- [ ] Stability testing
- [ ] Windows packaging

---

## 18. Definition of Done (MVP)

1. Game runs as standalone binary on Windows desktop
2. Continuous mic input captured without blocking game loop
3. CNN model loaded from `data/models/` (trained via speeko CLI separately)
4. All 10 voice commands recognized and mapped to gameplay actions
5. Player can: start, navigate 3 lanes, jump, slide, interact with gates, make choices, freeze, die
6. Score, combo, distance tracked and displayed
7. Calibration mode available for testing word recognition
8. Debug overlay shows FPS, latency, recognition diagnostics
9. Debounce and cooldown prevent spam commands
10. Game degrades gracefully on mic errors or missing model
11. Gameplay is understandable and fun enough for repeated play sessions

---

## 19. Future Considerations

### 19.1 Streaming VAD

Replace polling-based VAD with frame-by-frame streaming VAD to reduce detection latency by ~50ms.

### 19.2 Replay System

Record `GameCommandEvent`s with timestamps for deterministic replay, ghost runs, and recognition quality analysis.

### 19.3 Embedded Target Path

Separate audio capture + CNN inference from game engine. For embedded: replace Bevy with lighter renderer, keep recognition pipeline, replace `cpal` with platform HAL.

### 19.4 DTW Fallback Mode

If desired in the future, copy `speeko-recognizer` and `speeko-store` into the game project to support DTW recognition alongside CNN.

---

## 20. Appendix: Model Deployment Workflow

```
┌─────────────────────────────────────────────────────────┐
│  Step 1: Train (using original speeko project)           │
│                                                          │
│  $ cd speeko                                             │
│  $ cargo run -- record-samples start stop up down ...    │
│  $ cargo run -- cnn-train data/recordings --epochs 50    │
│                                                          │
│  Output: data/models/cnn_model.mpk                       │
│          data/models/cnn_vocab.json                       │
└──────────────────────┬──────────────────────────────────┘
                       │ copy
                       ▼
┌─────────────────────────────────────────────────────────┐
│  Step 2: Deploy (copy model files to game project)       │
│                                                          │
│  $ cp speeko/data/models/* voice-commando/data/models/   │
└──────────────────────┬──────────────────────────────────┘
                       │
                       ▼
┌─────────────────────────────────────────────────────────┐
│  Step 3: Play                                            │
│                                                          │
│  $ cd voice-commando                                     │
│  $ cargo run                                             │
└─────────────────────────────────────────────────────────┘
```
