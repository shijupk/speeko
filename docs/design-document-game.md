# Technical Design Document
## Project: Voice Runner (Command Rush)

---

## 1. Document Purpose

This document provides the detailed technical design for **Voice Runner**, a voice-controlled 2D endless runner game written in Rust. It translates the product requirements defined in `product-requirements-game.md` into concrete architecture, module design, data flow, integration patterns, and implementation guidance.

**Key constraint:** The game is a **new standalone project** that consumes the existing `speeko` crates (`speeko-common`, `speeko-audio`, `speeko-dsp`, `speeko-vad`, `speeko-features`, `speeko-recognizer`, `speeko-store`, `speeko-classifier`) as library dependencies. The existing crates are **not modified**.

---

## 2. Existing Crate Inventory

The game project depends on the following speeko crates. Each crate is consumed as a path dependency.

| Crate | Purpose | Key Public API |
|---|---|---|
| `speeko-common` | Shared types, config, errors | `SpeekConfig`, `AudioConfig`, `DspConfig`, `VadConfig`, `MfccConfig`, `RecognizerConfig`, `ClassifierConfig`, `PathsConfig`, `SpeekError`, `AudioBuffer`, `MfccSequence`, `MfccFrame`, `Template`, `RecognitionResult`, `WordScore`, `VadRegion` |
| `speeko-audio` | Microphone capture, WAV I/O | `capture::record_audio()`, `capture::list_input_devices()`, `capture::has_input_device()`, `capture::is_clipped()`, `wav::load_wav()`, `wav::save_wav()` |
| `speeko-dsp` | Signal processing | `preprocess::preprocess()`, `preprocess::remove_dc()`, `preprocess::pre_emphasis()`, `preprocess::normalize()`, `fft::FftProcessor`, `fft::power_spectrum()`, `framing::*` |
| `speeko-vad` | Voice activity detection | `energy_vad::detect_speech()`, `energy_vad::trim_silence()` |
| `speeko-features` | Feature extraction | `mfcc::MfccExtractor`, `mel::MelFilterbank`, `delta::compute_deltas()`, `delta::append_deltas_and_double_deltas()`, `cmn::normalize()` |
| `speeko-recognizer` | DTW matching | `matcher::TemplateMatcher`, `dtw::dtw_distance()`, `confidence::compute_confidence()`, `averaging::compute_mean_templates()` |
| `speeko-store` | Template & vocabulary persistence | `templates::TemplateStore`, `vocabulary::load_vocabulary()`, `vocabulary::is_in_vocabulary()` |
| `speeko-classifier` | CNN-based classification | `inference::CnnRecognizer`, `model::KeywordCnn`, `model::pad_or_truncate()` |

### 2.1 Existing API Constraints

The existing `speeko-audio::capture::record_audio()` function records a **fixed-duration** audio buffer synchronously (blocking the calling thread until `duration_secs` elapses or `cancel` is set). It returns a complete `Vec<f32>` buffer.

**Implication for the game:** The game cannot call `record_audio()` directly on the main thread or the ECS thread because it blocks. The game must run audio capture in a **dedicated background thread** and stream recognized words into the ECS via a channel or shared queue.

The `TemplateMatcher::recognize()` and `CnnRecognizer::predict()` functions are synchronous and operate on a single complete MFCC sequence. They are fast enough for real-time use on a desktop.

---

## 3. High-Level Architecture

```
┌─────────────────────────────────────────────────────────────────────┐
│                        Voice Runner Game                            │
│                                                                     │
│  ┌──────────────┐   ┌──────────────┐   ┌──────────────────────┐    │
│  │  Audio Thread │   │ Recognizer   │   │  Bevy App (ECS)      │    │
│  │              │   │  Thread      │   │                      │    │
│  │ cpal stream  │──>│ VAD+MFCC+   │──>│ CommandInputPlugin   │    │
│  │ ring buffer  │   │ DTW/CNN     │   │ GameplayPlugin       │    │
│  │              │   │              │   │ UIPlugin             │    │
│  └──────────────┘   └──────────────┘   │ AudioFeedbackPlugin  │    │
│                                         │ DiagnosticsPlugin    │    │
│                                         │ CalibrationPlugin    │    │
│                                         └──────────────────────┘    │
│                                                                     │
│  ┌─────────────────────────────────────────────────────────────┐    │
│  │  Speeko Crates (Library Dependencies — unmodified)          │    │
│  │  speeko-audio · speeko-dsp · speeko-vad · speeko-features   │    │
│  │  speeko-recognizer · speeko-classifier · speeko-store       │    │
│  │  speeko-common                                              │    │
│  └─────────────────────────────────────────────────────────────┘    │
└─────────────────────────────────────────────────────────────────────┘
```

### 3.1 Thread Model

| Thread | Responsibility | Blocking? |
|---|---|---|
| **Audio capture thread** | Runs a `cpal` input stream, writes raw PCM samples into a lock-free ring buffer | No (callback-driven) |
| **Recognizer thread** | Drains ring buffer, runs VAD segmentation, DSP preprocessing, MFCC extraction, DTW/CNN recognition. Sends `RecognizedWord` events to ECS via `crossbeam` channel | Yes (processing loop) |
| **Bevy main thread** | ECS schedule: reads recognized words, applies command mapping, runs gameplay systems, renders UI | No (frame-driven) |

### 3.2 Data Flow

```
Microphone (cpal callback)
    │
    ▼
Ring Buffer (lock-free, f32 samples)
    │
    ▼
Recognizer Thread
    ├── Continuous VAD segmentation (energy_vad::detect_speech)
    ├── DSP preprocessing (preprocess::preprocess)
    ├── MFCC extraction (MfccExtractor::extract)
    ├── Feature transforms (cmn::normalize, delta::append_deltas_and_double_deltas)
    ├── Recognition (TemplateMatcher::recognize or CnnRecognizer::predict)
    │
    ▼
crossbeam::channel::Sender<RecognizedWord>
    │
    ▼
Bevy ECS (Receiver<RecognizedWord> as NonSend resource)
    ├── SpeechInputSystem   → reads channel, emits GameCommand events
    ├── CommandMappingSystem → debounce, cooldown, state-aware validation
    ├── GameplaySystem       → applies commands to player state
    ├── UISystem             → updates HUD, feedback overlays
    ├── AudioFeedbackSystem  → plays accept/reject sounds
    └── DiagnosticsSystem    → logs latency, counters
```

---

## 4. Project Structure

```
voice-runner/
├── Cargo.toml
├── config/
│   └── voice_runner.toml          # Game + recognition config
├── assets/
│   ├── sprites/                   # Player, obstacles, backgrounds
│   ├── sounds/                    # SFX and music
│   ├── fonts/                     # UI fonts
│   └── ui/                        # UI assets
├── data/
│   ├── templates/                 # Trained word templates (DTW)
│   ├── models/                    # CNN model files
│   └── vocabulary.txt             # 10-word vocabulary
├── src/
│   ├── main.rs                    # Entry point, Bevy App builder
│   ├── app_config.rs              # Game-specific config loading
│   ├── app_states.rs              # Bevy AppState enum
│   │
│   ├── audio_bridge/
│   │   ├── mod.rs
│   │   ├── plugin.rs              # AudioBridgePlugin
│   │   ├── capture_thread.rs      # Audio capture thread management
│   │   ├── recognizer_thread.rs   # Recognizer thread management
│   │   └── ring_buffer.rs         # Lock-free ring buffer
│   │
│   ├── speech/
│   │   ├── mod.rs
│   │   ├── plugin.rs              # SpeechPlugin
│   │   ├── events.rs              # RecognizedWord event type
│   │   └── resources.rs           # SpeechReceiver resource
│   │
│   ├── commands/
│   │   ├── mod.rs
│   │   ├── plugin.rs              # CommandPlugin
│   │   ├── mapping.rs             # Word-to-GameCommand mapping
│   │   ├── debounce.rs            # Per-command debounce logic
│   │   ├── cooldown.rs            # Per-command cooldown timers
│   │   └── types.rs               # GameCommand enum, CommandEvent
│   │
│   ├── game/
│   │   ├── mod.rs
│   │   ├── plugin.rs              # GamePlugin
│   │   ├── player.rs              # Player component, state machine
│   │   ├── lanes.rs               # Lane model (3-lane)
│   │   ├── obstacles.rs           # Obstacle spawning, types
│   │   ├── interactions.rs        # Gates, choices, shield events
│   │   ├── collision.rs           # Collision detection
│   │   ├── scoring.rs             # Score, combo, distance
│   │   ├── difficulty.rs          # Difficulty progression
│   │   └── world.rs               # World scrolling, segment management
│   │
│   ├── ui/
│   │   ├── mod.rs
│   │   ├── plugin.rs              # UIPlugin
│   │   ├── menu.rs                # Main menu screen
│   │   ├── hud.rs                 # In-game HUD
│   │   ├── calibration.rs         # Calibration screen
│   │   ├── settings.rs            # Settings screen
│   │   ├── game_over.rs           # Game over screen
│   │   └── subtitles.rs           # Recognized word subtitles
│   │
│   ├── effects/
│   │   ├── mod.rs
│   │   ├── plugin.rs              # EffectsPlugin
│   │   ├── sound_fx.rs            # SFX playback
│   │   └── visual_fx.rs           # Screen flash, shake
│   │
│   └── diagnostics/
│       ├── mod.rs
│       ├── plugin.rs              # DiagnosticsPlugin
│       ├── overlay.rs             # Debug overlay
│       └── counters.rs            # Performance counters, latency tracker
│
├── benches/
│   └── recognition_latency.rs     # End-to-end recognition benchmarks
└── tests/
    ├── command_mapping_tests.rs
    ├── debounce_tests.rs
    └── scoring_tests.rs
```

---

## 5. Cargo.toml Design

```toml
[package]
name = "voice-runner"
version = "0.1.0"
edition = "2021"
rust-version = "1.75.0"

[[bin]]
name = "voice-runner"
path = "src/main.rs"

[dependencies]
# --- Speeko crates (path dependencies, unmodified) ---
speeko-common    = { path = "../crates/common" }
speeko-audio     = { path = "../crates/audio" }
speeko-dsp       = { path = "../crates/dsp" }
speeko-vad       = { path = "../crates/vad" }
speeko-features  = { path = "../crates/features" }
speeko-recognizer = { path = "../crates/recognizer" }
speeko-store     = { path = "../crates/store" }
speeko-classifier = { path = "../crates/classifier" }

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
name = "recognition_latency"
harness = false
```

### 5.1 Workspace Integration

The game project is added to the root `Cargo.toml` workspace:

```toml
[workspace]
members = [
    "crates/common",
    "crates/audio",
    "crates/dsp",
    "crates/vad",
    "crates/features",
    "crates/recognizer",
    "crates/store",
    "crates/app-cli",
    "crates/classifier",
    "voice-runner",          # <-- new game project
]
```

---

## 6. Bevy Application States

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

### 6.1 State Transition Diagram

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

## 7. Module Designs

### 7.1 Audio Bridge Module (`audio_bridge/`)

**Purpose:** Manage the lifecycle of the audio capture thread and recognizer thread. Bridge the gap between the blocking `speeko-audio` APIs and the non-blocking Bevy ECS.

#### 7.1.1 Ring Buffer (`ring_buffer.rs`)

Uses `ringbuf` crate for a single-producer single-consumer lock-free ring buffer.

```rust
use ringbuf::{HeapRb, traits::*};

pub struct AudioRingBuffer {
    pub producer: ringbuf::HeapProd<f32>,
    pub consumer: ringbuf::HeapCons<f32>,
}

impl AudioRingBuffer {
    /// Create a ring buffer sized for `buffer_secs` at `sample_rate`.
    pub fn new(sample_rate: u32, buffer_secs: f32) -> Self {
        let capacity = (sample_rate as f32 * buffer_secs) as usize;
        let rb = HeapRb::<f32>::new(capacity);
        let (producer, consumer) = rb.split();
        Self { producer, consumer }
    }
}
```

**Design rationale:** The `cpal` audio callback must be non-blocking. Using a lock-free SPSC ring buffer avoids mutex contention in the audio callback. The producer side lives in the cpal callback closure; the consumer side lives in the recognizer thread.

#### 7.1.2 Capture Thread (`capture_thread.rs`)

Instead of using `speeko-audio::capture::record_audio()` (which blocks for a fixed duration), the capture thread sets up a continuous `cpal` input stream that writes into the ring buffer indefinitely.

```rust
pub struct CaptureThread {
    _stream: cpal::Stream,  // kept alive to maintain the audio stream
    cancel: Arc<AtomicBool>,
}

impl CaptureThread {
    pub fn start(
        sample_rate: u32,
        producer: ringbuf::HeapProd<f32>,
    ) -> Result<Self> {
        let host = cpal::default_host();
        let device = host.default_input_device()
            .ok_or(SpeekError::NoAudioDevice)?;

        // Negotiate config (reuse logic similar to speeko-audio::capture)
        // Build continuous input stream writing to producer
        // ...
    }

    pub fn stop(&self) {
        self.cancel.store(true, Ordering::SeqCst);
    }
}
```

**Note:** We directly use `cpal` (already a transitive dependency via `speeko-audio`) for the continuous stream. The existing `record_audio()` is designed for single-shot recording and is not suitable for continuous game input.

#### 7.1.3 Recognizer Thread (`recognizer_thread.rs`)

Runs a continuous recognition loop:

```rust
pub struct RecognizerThread {
    handle: Option<std::thread::JoinHandle<()>>,
    cancel: Arc<AtomicBool>,
}

impl RecognizerThread {
    pub fn start(
        consumer: ringbuf::HeapCons<f32>,
        config: SpeekConfig,
        sender: crossbeam_channel::Sender<RecognizedWord>,
    ) -> Self {
        // Spawn thread that:
        // 1. Accumulates samples from ring buffer into a sliding window
        // 2. Runs energy-based VAD to detect utterance boundaries
        // 3. On utterance detected:
        //    a. preprocess::preprocess()
        //    b. MfccExtractor::extract()
        //    c. apply CMN + deltas
        //    d. TemplateMatcher::recognize() or CnnRecognizer::predict()
        //    e. Send RecognizedWord via channel
        // 4. Loop with small sleep between iterations
    }
}
```

##### Sliding Window VAD Strategy

The recognizer thread maintains a circular sample buffer of the last `max_utterance_secs` worth of audio. It periodically checks for speech activity:

1. **Accumulation phase:** Drain samples from ring buffer into local sliding window.
2. **Detection phase:** Run `energy_vad::detect_speech()` on the current window.
3. **Extraction phase:** If speech detected, extract the speech region, process it, and recognize.
4. **Cooldown phase:** After a successful recognition, skip a short cooldown period (~300ms) to avoid re-triggering on the same utterance.

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

#### 7.1.4 Plugin (`plugin.rs`)

```rust
pub struct AudioBridgePlugin;

impl Plugin for AudioBridgePlugin {
    fn build(&self, app: &mut App) {
        // On entering Loading state:
        //   - Create ring buffer
        //   - Start capture thread
        //   - Start recognizer thread
        //   - Insert Receiver<RecognizedWord> as resource
        //
        // On exiting app:
        //   - Signal cancel to both threads
        //   - Join threads
    }
}
```

---

### 7.2 Speech Module (`speech/`)

**Purpose:** Receive recognized words from the recognizer thread and emit them as Bevy events.

#### 7.2.1 Events (`events.rs`)

```rust
#[derive(Event, Debug, Clone)]
pub struct RecognizedWordEvent {
    /// The recognized word (e.g., "up", "left", "open").
    pub word: String,
    /// Confidence score in [0.0, 1.0].
    pub confidence: f32,
    /// DTW distance or (1 - probability) depending on mode.
    pub distance: f32,
    /// All scored words for diagnostics.
    pub all_scores: Vec<(String, f32)>,
    /// Timestamp of recognition (Instant).
    pub recognized_at: std::time::Instant,
}

#[derive(Event, Debug, Clone)]
pub struct RejectedWordEvent {
    /// Best guess word that was rejected.
    pub best_guess: Option<String>,
    /// Confidence that failed threshold.
    pub confidence: f32,
    /// Rejection reason.
    pub reason: RejectionReason,
    pub recognized_at: std::time::Instant,
}

#[derive(Debug, Clone)]
pub enum RejectionReason {
    LowConfidence,
    HighDistance,
    NoSpeechDetected,
}
```

#### 7.2.2 Resources (`resources.rs`)

```rust
#[derive(Resource)]
pub struct SpeechReceiver {
    pub receiver: crossbeam_channel::Receiver<RecognizedWord>,
}
```

#### 7.2.3 System

```rust
fn speech_input_system(
    receiver: Res<SpeechReceiver>,
    mut recognized_events: EventWriter<RecognizedWordEvent>,
    mut rejected_events: EventWriter<RejectedWordEvent>,
) {
    // Drain all pending recognized words from channel (non-blocking)
    while let Ok(word) = receiver.receiver.try_recv() {
        match word.result.word {
            Some(w) => recognized_events.send(RecognizedWordEvent { ... }),
            None => rejected_events.send(RejectedWordEvent { ... }),
        }
    }
}
```

---

### 7.3 Commands Module (`commands/`)

**Purpose:** Map recognized words to gameplay commands with debounce, cooldown, and state-aware validation.

#### 7.3.1 Types (`types.rs`)

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
    pub issued_at: std::time::Instant,
}
```

#### 7.3.2 Mapping (`mapping.rs`)

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

#### 7.3.3 Debounce (`debounce.rs`)

Prevents the same command from firing repeatedly due to audio artifacts or user repetition.

```rust
#[derive(Resource)]
pub struct DebounceState {
    /// Per-command last-accepted timestamp.
    last_accepted: HashMap<GameCommand, Instant>,
    /// Minimum interval between same-command accepts.
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

**Default debounce:** 300ms per command. Configurable via `voice_runner.toml`.

#### 7.3.4 Cooldown (`cooldown.rs`)

Per-command cooldown is separate from debounce. Cooldowns are gameplay-driven (e.g., you can't jump again for 500ms after landing).

```rust
#[derive(Resource)]
pub struct CooldownState {
    /// Per-command cooldown remaining.
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

#### 7.3.5 Command Validation System

```rust
fn command_mapping_system(
    mut recognized_events: EventReader<RecognizedWordEvent>,
    mut command_events: EventWriter<GameCommandEvent>,
    mut debounce: ResMut<DebounceState>,
    cooldown: Res<CooldownState>,
    player_query: Query<&PlayerState>,
    game_state: Res<State<AppState>>,
) {
    let now = Instant::now();

    for event in recognized_events.read() {
        let Some(command) = map_word_to_command(&event.word) else {
            continue;
        };

        // State-aware validation
        if !is_command_valid_in_state(command, &game_state, &player_query) {
            continue;
        }

        // Debounce check
        if !debounce.can_accept(command, now) {
            continue;
        }

        // Cooldown check
        if !cooldown.is_ready(command) {
            continue;
        }

        debounce.record_accept(command, now);
        command_events.send(GameCommandEvent {
            command,
            source_word: event.word.clone(),
            confidence: event.confidence,
            issued_at: now,
        });
    }
}
```

##### State-Aware Validation Rules

| Command | Valid When |
|---|---|
| `Jump` | Playing, player is grounded |
| `Slide` | Playing, player is grounded |
| `MoveLeft` | Playing, current lane > 0 |
| `MoveRight` | Playing, current lane < 2 |
| `Freeze` | Playing (triggers freeze/brace action or pause) |
| `Resume` | Paused, or GameOver (restart), or MainMenu (start game) |
| `OpenGate` | Playing, near a gate entity |
| `CloseShield` | Playing, shield/barrier interaction active |
| `AcceptChoice` | Playing, choice prompt active |
| `RejectChoice` | Playing, choice prompt active |

---

### 7.4 Game Module (`game/`)

#### 7.4.1 Player State Machine (`player.rs`)

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

#### 7.4.2 Lane Model (`lanes.rs`)

Three-lane model with world-space x-coordinates:

```rust
pub const LANE_WIDTH: f32 = 2.0;
pub const LANE_POSITIONS: [f32; 3] = [-LANE_WIDTH, 0.0, LANE_WIDTH];

pub fn lane_x(lane: Lane) -> f32 {
    LANE_POSITIONS[lane as usize]
}
```

#### 7.4.3 Obstacles (`obstacles.rs`)

```rust
#[derive(Component, Debug)]
pub enum ObstacleType {
    /// Requires Jump (up)
    LowBarrier,
    /// Requires Slide (down)
    HighBarrier,
    /// Requires lane change (left/right)
    LaneBlocker { lane: Lane },
    /// Requires Open command
    Gate,
    /// Requires Close command (activate shield)
    Projectile,
    /// Requires Yes/No choice
    ChoiceFork { left_reward: i32, right_reward: i32 },
}

#[derive(Component)]
pub struct Obstacle {
    pub obstacle_type: ObstacleType,
    pub z_position: f32,  // distance ahead of player
    pub active: bool,
}
```

##### Spawning Strategy

Obstacles spawn at a distance ahead of the player and scroll toward the player. Spawn rate and composition are controlled by the difficulty system.

```rust
#[derive(Resource)]
pub struct ObstacleSpawner {
    pub spawn_timer: Timer,
    pub spawn_distance: f32,     // how far ahead to spawn
    pub min_gap: f32,            // minimum gap between obstacles
    pub active_count_limit: u32, // max simultaneous obstacles
}
```

#### 7.4.4 Collision (`collision.rs`)

Simple AABB collision between the player's hitbox and obstacle hitboxes. Collision checks run each frame for obstacles within a small z-range around the player.

```rust
fn collision_system(
    player_query: Query<(&Player, &Transform)>,
    obstacle_query: Query<(&Obstacle, &ObstacleType, &Transform)>,
    mut game_over_events: EventWriter<GameOverEvent>,
    mut score: ResMut<Score>,
) {
    // For each active obstacle near player z:
    //   If obstacle is avoidable and player has correct action → score bonus
    //   If collision and no correct action → damage/game over
}
```

#### 7.4.5 Scoring (`scoring.rs`)

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

#### 7.4.6 Difficulty (`difficulty.rs`)

```rust
#[derive(Resource)]
pub struct Difficulty {
    pub scroll_speed: f32,
    pub obstacle_rate: f32,       // obstacles per second
    pub contextual_chance: f32,   // probability of gate/choice/shield obstacles
    pub time_elapsed: f32,
}

impl Difficulty {
    pub fn update(&mut self, delta: f32) {
        self.time_elapsed += delta;
        // Ramp speed: start at 5.0, increase 0.5 per 30 seconds, cap at 15.0
        self.scroll_speed = (5.0 + self.time_elapsed / 30.0 * 0.5).min(15.0);
        // Ramp obstacle rate: start at 0.8/s, increase to 2.0/s
        self.obstacle_rate = (0.8 + self.time_elapsed / 60.0 * 0.4).min(2.0);
        // Contextual obstacles appear after 30s
        self.contextual_chance = if self.time_elapsed > 30.0 { 0.2 } else { 0.0 };
    }
}
```

---

### 7.5 UI Module (`ui/`)

#### 7.5.1 HUD Design (`hud.rs`)

The HUD displays critical gameplay and recognition feedback:

```
┌────────────────────────────────────────────────────┐
│  Score: 1250    Combo: 7x    Distance: 420m        │
│                                                    │
│                                                    │
│            [Game Viewport — 3 lanes]               │
│                                                    │
│                                                    │
│  ┌──────────────────────────────────────────────┐  │
│  │  Heard: UP (94%) → Jump ✓                    │  │
│  └──────────────────────────────────────────────┘  │
└────────────────────────────────────────────────────┘
```

**HUD components:**
- **Top bar:** Score, combo counter, distance
- **Center:** Game viewport
- **Bottom bar:** Last recognized word, confidence, mapped action, accept/reject indicator

```rust
#[derive(Component)]
pub struct HudLastWord;

#[derive(Component)]
pub struct HudScore;

#[derive(Component)]
pub struct HudCombo;

#[derive(Component)]
pub struct HudDistance;

fn update_hud_system(
    score: Res<Score>,
    last_recognition: Res<LastRecognition>,
    mut word_text: Query<&mut Text, With<HudLastWord>>,
    mut score_text: Query<&mut Text, (With<HudScore>, Without<HudLastWord>)>,
    // ...
) { /* update text nodes */ }
```

#### 7.5.2 Calibration Screen (`calibration.rs`)

The calibration screen provides a live view of recognition quality:

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
│  ┌────────┬─────────┬──────────┬─────────────────┐ │
│  │ Word   │ Tested  │ Accepted │ Avg Confidence  │ │
│  ├────────┼─────────┼──────────┼─────────────────┤ │
│  │ start  │    3    │    3     │     91%         │ │
│  │ stop   │    2    │    2     │     85%         │ │
│  │ up     │    5    │    4     │     88%         │ │
│  │ ...    │   ...   │   ...    │     ...         │ │
│  └────────┴─────────┴──────────┴─────────────────┘ │
│                                                    │
│  Threshold: [====|======] 0.30                     │
│  Debounce:  [==|========] 300ms                    │
│                                                    │
│  [Save Settings]    [Back to Menu]                 │
└────────────────────────────────────────────────────┘
```

**Calibration resources:**

```rust
#[derive(Resource, Default)]
pub struct CalibrationStats {
    pub per_word: HashMap<String, WordCalibrationData>,
}

#[derive(Debug, Default)]
pub struct WordCalibrationData {
    pub tested: u32,
    pub accepted: u32,
    pub total_confidence: f32,
}
```

---

### 7.6 Effects Module (`effects/`)

#### 7.6.1 Sound Effects (`sound_fx.rs`)

Uses `bevy_kira_audio` for game-oriented audio playback.

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

**Design note:** SFX must be short and non-intrusive to avoid interfering with the microphone input. Background music volume should be configurable and defaults to low.

---

### 7.7 Diagnostics Module (`diagnostics/`)

#### 7.7.1 Debug Overlay (`overlay.rs`)

Toggleable via config or keyboard shortcut (F3).

```
┌─ Debug ─────────────────────────────────────┐
│ FPS: 60.0  Frame: 1.2ms                     │
│ Recognizer: idle (last: 45ms)               │
│ Ring buffer: 2400/32000 samples              │
│ Last word: "up" (0.94) → Jump [Accepted]    │
│ Commands: 24 accepted / 3 rejected           │
│ Player: Running, Lane: Center                │
│ Obstacles: 4 active                          │
│ Latency: voice→action ~120ms                 │
└──────────────────────────────────────────────┘
```

#### 7.7.2 Latency Tracking

```rust
#[derive(Resource)]
pub struct LatencyTracker {
    /// Circular buffer of recent voice-to-action latencies.
    samples: VecDeque<Duration>,
    capacity: usize,
}

impl LatencyTracker {
    pub fn record(&mut self, latency: Duration) {
        if self.samples.len() >= self.capacity {
            self.samples.pop_front();
        }
        self.samples.push_back(latency);
    }

    pub fn average(&self) -> Duration {
        if self.samples.is_empty() {
            return Duration::ZERO;
        }
        let total: Duration = self.samples.iter().sum();
        total / self.samples.len() as u32
    }

    pub fn p95(&self) -> Duration {
        if self.samples.is_empty() {
            return Duration::ZERO;
        }
        let mut sorted: Vec<Duration> = self.samples.iter().cloned().collect();
        sorted.sort();
        sorted[(sorted.len() as f32 * 0.95) as usize]
    }
}
```

---

## 8. Configuration Design

The game uses a unified TOML config that extends `SpeekConfig` with game-specific settings.

```toml
# voice_runner.toml

# --- Speeko recognition config (loaded into SpeekConfig) ---
[audio]
sample_rate = 16000
record_duration_secs = 1.5
max_utterance_secs = 1.0

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

[recognizer]
mode = "dtw"                  # "dtw" or "cnn"
confidence_threshold = 0.35
max_distance = 35.0
sakoe_chiba_width = 0.2
min_samples_per_word = 3
recommended_samples_per_word = 5

[paths]
templates_dir = "data/templates"
recordings_dir = "data/recordings"
vocabulary_file = "data/vocabulary.txt"

# --- Game-specific config ---
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

```rust
/// Game-specific config layered on top of SpeekConfig.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VoiceRunnerConfig {
    #[serde(flatten)]
    pub speeko: SpeekConfig,
    pub game: GameConfig,
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

## 9. Bevy Plugin Registration

```rust
// main.rs
fn main() {
    let cli = Cli::parse();
    let config = VoiceRunnerConfig::load(&cli.config).unwrap_or_default();

    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "Voice Runner — Command Rush".into(),
                resolution: (config.game.display.resolution[0] as f32,
                             config.game.display.resolution[1] as f32).into(),
                ..default()
            }),
            ..default()
        }))
        .init_state::<AppState>()
        .insert_resource(config.clone())
        // --- Speeko integration ---
        .add_plugins(AudioBridgePlugin)
        .add_plugins(SpeechPlugin)
        // --- Game ---
        .add_plugins(CommandPlugin)
        .add_plugins(GamePlugin)
        .add_plugins(UIPlugin)
        .add_plugins(EffectsPlugin)
        .add_plugins(DiagnosticsPlugin)
        .run();
}
```

### 9.1 System Ordering

Within a frame, systems must execute in a deterministic order:

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

Enforced via Bevy system ordering:

```rust
app.configure_sets(Update, (
    GameSet::Input,
    GameSet::Command,
    GameSet::Simulation,
    GameSet::Collision,
    GameSet::Feedback,
    GameSet::Diagnostics,
).chain());
```

---

## 10. Recognizer Integration Details

### 10.1 DTW Mode

Uses existing speeko crates directly:

```rust
// In recognizer_thread.rs
let store = TemplateStore::new(&config.paths.templates_dir)?;
let all_templates = store.load_all_templates()?;
let mean_templates = averaging::compute_mean_templates(&all_templates);
let matcher = TemplateMatcher::new(
    config.recognizer.sakoe_chiba_width,
    config.recognizer.confidence_threshold,
    config.recognizer.max_distance,
);

// Per utterance:
let mut mfcc_extractor = MfccExtractor::new(
    config.audio.sample_rate, &config.dsp, &config.mfcc,
);
let mfcc = mfcc_extractor.extract(&speech_samples);
let mfcc = apply_transforms(mfcc, &config);  // CMN + deltas
let result = matcher.recognize(&mfcc, &mean_templates);
```

### 10.2 CNN Mode

```rust
let cnn = CnnRecognizer::load(
    &config.classifier.model_dir,
    config.recognizer.confidence_threshold,
    config.classifier.max_frames,
)?;

// Per utterance:
let result = cnn.predict(&mfcc);
```

### 10.3 Recognition Pipeline Timing Budget

| Stage | Target Latency | Notes |
|---|---|---|
| VAD detection | < 5ms | Energy-based, very fast |
| DSP preprocessing | < 1ms | In-place operations |
| MFCC extraction | < 10ms | ~50-100 frames typical |
| CMN + deltas | < 2ms | Simple arithmetic |
| DTW matching (10 words) | < 20ms | Sakoe-Chiba banded, mean templates |
| CNN inference | < 15ms | NdArray backend, small model |
| **Total pipeline** | **< 40ms** | Well within 1-frame budget at 60 FPS |

### 10.4 End-to-End Latency Estimate

| Component | Estimated Latency |
|---|---|
| Utterance duration | ~300-600ms (user speaks) |
| VAD trailing hangover | ~80ms |
| Ring buffer → recognizer drain | ~50ms (poll interval) |
| Recognition pipeline | ~40ms |
| Channel → ECS read | < 1ms (next frame) |
| Command mapping + action | < 1ms |
| **Voice-to-action total** | **~150ms** (after speech ends) |

---

## 11. Interaction Mechanic Designs

### 11.1 Gate Interaction (open/close)

```
  Player approaches gate
        │
        ▼
  Gate entity enters "interaction zone" (z within range)
        │
        ▼
  UI shows prompt: "Say OPEN to pass"
        │
  ┌─────┴──────┐
  │             │
"open"        timeout
  │             │
  ▼             ▼
Gate opens   Gate stays closed
Player       Player collides
passes       (damage/game over)
```

### 11.2 Choice Interaction (yes/no)

```
  Fork in path appears
        │
        ▼
  UI shows: "Left path: +50pts  |  Right path: shield"
  UI shows: "Say YES (left) or NO (right)"
        │
  ┌─────┴──────┐
  │             │
"yes"          "no"
  │             │
  ▼             ▼
Left path    Right path
+50 points   Shield buff
```

### 11.3 Shield/Brace Interaction (close/stop)

```
  Incoming projectile wave
        │
        ▼
  UI shows: "Say CLOSE for shield!"
        │
"close"
  │
  ▼
Shield activates for 400ms
Player is invincible
```

---

## 12. Error Handling Strategy

### 12.1 Error Types

```rust
#[derive(Debug, thiserror::Error)]
pub enum VoiceRunnerError {
    #[error("Audio device error: {0}")]
    AudioDevice(#[from] speeko_common::error::SpeekError),

    #[error("Configuration error: {0}")]
    Config(String),

    #[error("Template loading failed: {0}")]
    TemplateLoad(#[from] anyhow::Error),

    #[error("Recognizer thread panicked")]
    RecognizerPanic,

    #[error("Audio capture thread panicked")]
    CapturePanic,

    #[error("Ring buffer overflow — audio samples dropped")]
    RingBufferOverflow,
}
```

### 12.2 Graceful Degradation

| Failure | Behavior |
|---|---|
| No microphone found | Show error screen with message, offer to retry |
| Microphone disconnects mid-game | Pause game, show reconnect prompt |
| Ring buffer overflow | Log warning, drop oldest samples (self-healing) |
| Recognition thread panic | Catch via `JoinHandle`, show error, offer restart |
| Template load failure | Fall back to CNN if available, or show error |
| Config file missing | Use defaults, log warning |
| Low confidence streak | Show calibration hint in HUD |

---

## 13. Testing Strategy

### 13.1 Unit Tests

| Module | What to Test |
|---|---|
| `commands/mapping.rs` | All 10 word→command mappings, unknown word returns None |
| `commands/debounce.rs` | Debounce window enforcement, different commands independent |
| `commands/cooldown.rs` | Cooldown timer tick, ready/not-ready states |
| `game/scoring.rs` | Score increments, combo multiplier, combo reset |
| `game/lanes.rs` | Lane bounds checking, lane positions |
| `game/difficulty.rs` | Difficulty ramp at time thresholds, caps |
| `game/player.rs` | State transitions: Running→Jumping→Running, invalid transitions |

### 13.2 Integration Tests

| Test | Description |
|---|---|
| Recognition pipeline | Feed known WAV → full pipeline → verify correct word output |
| Command flow | Simulate RecognizedWordEvent → verify GameCommandEvent emitted |
| State transitions | Walk through Loading→Menu→Playing→GameOver→Menu |
| Collision | Place obstacle + player at same position → verify game over |
| Choice mechanic | Simulate yes/no during active choice → verify correct path taken |

### 13.3 Performance Tests (Criterion)

```rust
// benches/recognition_latency.rs
fn bench_dtw_recognition(c: &mut Criterion) {
    // Load templates, create matcher
    // Generate synthetic MFCC sequence
    c.bench_function("dtw_10_words", |b| {
        b.iter(|| matcher.recognize(&query, &templates))
    });
}

fn bench_mfcc_extraction(c: &mut Criterion) {
    // Load 1s of audio
    c.bench_function("mfcc_extract_1s", |b| {
        b.iter(|| extractor.extract(&samples))
    });
}
```

### 13.4 Manual Test Scenarios

| Scenario | Expected |
|---|---|
| Speak all 10 words in sequence | Each recognized and mapped correctly |
| Speak rapidly (spam "up up up") | Debounce prevents duplicate jumps |
| Speak in noisy room | Calibration threshold rejects false positives |
| Unplug microphone during game | Game pauses, shows reconnect message |
| Long play session (10+ minutes) | Stable FPS, no memory growth |
| Say unrecognized word ("hello") | Rejected, no action taken |

---

## 14. Library Selection Summary

| Concern | Library | Version | Justification |
|---|---|---|---|
| Game engine | `bevy` | 0.15 | Rust-native ECS, plugin architecture, 2D rendering, UI |
| Mic input | `cpal` | 0.15 | Already used by `speeko-audio`, cross-platform |
| Game audio | `bevy_kira_audio` | 0.21 | Bevy-integrated, game-oriented SFX/music |
| Config | `serde` + `toml` | 1.0 / 0.8 | Already used by `speeko-common`, human-readable |
| Error handling | `thiserror` + `anyhow` | 2.0 / 1.0 | Typed errors + boundary convenience |
| Logging | `tracing` | 0.1 | Structured spans for latency measurement |
| CLI | `clap` | 4.5 | Already used by speeko CLI |
| Lock-free queue | `ringbuf` | 0.4 | SPSC ring buffer for audio callback |
| Channels | `crossbeam-channel` | 0.5 | MPSC channel for recognizer → ECS communication |
| RNG | `rand` | 0.8 | Obstacle spawning, choice generation |
| Benchmarking | `criterion` | 0.5 | Microbenchmarks for recognition pipeline |

### 14.1 Alternatives Considered

| Concern | Alternative | Tradeoff |
|---|---|---|
| Game engine | `macroquad` | Simpler but lacks ECS, harder to scale |
| Game audio | `rodio` | Simpler but less game-oriented control |
| Lock-free queue | `crossbeam` deque | More powerful but SPSC ring buffer is sufficient and simpler |
| Logging | `log` + `env_logger` | Already in speeko crates; `tracing` adds structured spans |
| Config | `serde_json` | Machine-friendly but worse for hand-editing |

---

## 15. Risks and Mitigations

| Risk | Impact | Likelihood | Mitigation |
|---|---|---|---|
| SFX/BGM interferes with mic | Recognition accuracy drops | High | Keep BGM low, use directional mic guidance, add SFX volume control |
| `cpal` continuous stream API differs from `record_audio` | Integration complexity | Medium | Build thin adapter; continuous stream is simpler than fixed-duration |
| False positives during gameplay | Frustrating unintended actions | High | State-aware validation, confidence threshold, debounce, cooldown |
| Bevy version churn | API breakage | Medium | Pin Bevy version, isolate engine-specific code in plugins |
| Ring buffer overflow under load | Dropped audio, missed words | Low | Size buffer for 2s, log warnings, self-healing design |
| Speech fatigue during long sessions | User discomfort | Medium | Design meaningful sparse commands, avoid rapid-fire requirements |
| Recognition latency variance | Inconsistent gameplay feel | Medium | Forgiving timing windows, visual telegraph for upcoming obstacles |

---

## 16. Phased Roadmap

### Phase 1: Technical Skeleton (Week 1-2)

- [ ] Project scaffolding (`voice-runner/` with Cargo.toml, workspace integration)
- [ ] Bevy app with `AppState` state machine
- [ ] Main menu screen (Start Game, Calibration, Settings, Exit)
- [ ] Audio bridge: ring buffer + capture thread + recognizer thread
- [ ] `crossbeam` channel integration: RecognizedWord → Bevy event
- [ ] Basic HUD showing last recognized word + confidence
- [ ] Keyboard fallback for testing (press 1-0 to simulate commands)

### Phase 2: Gameplay MVP (Week 3-4)

- [ ] 3-lane runner with auto-scrolling world
- [ ] Player sprite with Jump, Slide, Lane Change animations
- [ ] Obstacle spawning: LowBarrier, HighBarrier, LaneBlocker
- [ ] Collision detection and game over
- [ ] `start` → begin run, `stop` → pause
- [ ] Score + distance tracking
- [ ] Basic difficulty ramp

### Phase 3: Full Command Integration (Week 5-6)

- [ ] Gate mechanic (`open`)
- [ ] Shield/brace mechanic (`close`, `stop`)
- [ ] Choice fork mechanic (`yes`, `no`)
- [ ] Combo system with multiplier
- [ ] Debounce and cooldown tuning
- [ ] SFX for all commands (accept/reject sounds)
- [ ] Calibration screen with per-word stats

### Phase 4: Product Polish (Week 7-8)

- [ ] Visual polish: sprite art, parallax background, screen effects
- [ ] Settings screen with persistence
- [ ] Debug overlay (F3 toggle)
- [ ] Diagnostics logging (tracing spans)
- [ ] Performance profiling and optimization
- [ ] Stability testing (long sessions, noisy environments)
- [ ] Packaging for Windows release

---

## 17. Definition of Done (MVP)

The MVP is complete when all of the following are true:

1. Game runs as a standalone binary on Windows desktop
2. Continuous microphone input is captured without blocking the game loop
3. All 10 voice commands are recognized and mapped to gameplay actions
4. Player can: start a run, navigate 3 lanes, jump, slide, interact with gates, make choices, freeze, and die
5. Score, combo, and distance are tracked and displayed
6. Calibration mode allows testing all words with confidence feedback
7. Debug overlay shows FPS, latency, and recognition diagnostics
8. Debounce and cooldown prevent duplicate/spam commands
9. Config is loaded from `voice_runner.toml` with sensible defaults
10. Game degrades gracefully on microphone errors
11. Gameplay is understandable and fun enough for repeated runs

---

## 18. Future Architecture Considerations

### 18.1 Streaming VAD (Post-MVP)

The current design uses periodic polling of a sliding window for VAD. A future improvement could implement a **streaming VAD** that processes audio frame-by-frame as it arrives, reducing detection latency by ~50ms.

### 18.2 Recognition Mode Hot-Swap

Support switching between DTW and CNN modes at runtime (e.g., from the settings screen) without restarting the recognizer thread.

### 18.3 Replay System

Record all `GameCommandEvent`s with timestamps for deterministic replay. This enables:
- Ghost runs
- Bug reproduction
- Recognition quality analysis from recorded sessions

### 18.4 Embedded Target Path

The architecture separates audio capture and recognition from the game engine. For embedded targets:
- Replace Bevy with a lighter renderer (e.g., `embedded-graphics`)
- Keep the recognition pipeline unchanged
- Replace `cpal` with platform-specific audio HAL

### 18.5 Input Session Recording

Record raw audio alongside gameplay for offline recognition tuning. Save as WAV files tagged with the expected command context.

---

## 19. Appendix: Speeko Crate API Quick Reference

### RecognitionResult (from speeko-common)

```rust
pub struct RecognitionResult {
    pub word: Option<String>,      // None if rejected
    pub confidence: f32,           // [0.0, 1.0]
    pub best_distance: f32,        // DTW distance or (1-prob)
    pub scores: Vec<WordScore>,    // All words, sorted by distance asc
}
```

### Full Recognition Pipeline (as used in speeko CLI)

```rust
// 1. Record audio
let samples = capture::record_audio(sample_rate, duration, &cancel)?;

// 2. Preprocess
preprocess::preprocess(&mut samples, config.dsp.pre_emphasis);

// 3. VAD trim
let speech = energy_vad::trim_silence(
    &samples, sample_rate, frame_length, frame_step, &config.vad
).ok_or(SpeekError::NoSpeechDetected)?;

// 4. Extract MFCC
let mfcc = mfcc_extractor.extract(&speech);

// 5. Apply transforms
let mfcc = cmn::normalize(&mfcc);                          // if use_cmn
let mfcc = delta::append_deltas_and_double_deltas(&mfcc);  // if use_deltas

// 6. Recognize
let result = matcher.recognize(&mfcc, &templates);  // DTW mode
// or
let result = cnn.predict(&mfcc);                    // CNN mode
```

This pipeline is replicated in the recognizer thread, operating on segments extracted from the continuous audio ring buffer rather than single-shot recordings.
