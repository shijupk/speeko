# Speeko — Offline Spoken-Word Recognition System: Design Document & Implementation Plan

A comprehensive design and phased implementation plan for a small-footprint, offline, isolated-word recognizer in Rust using MFCC features and DTW-based template matching.

---

## 1. Executive Summary

**Speeko** is an offline spoken-word recognition application targeting edge/embedded Linux devices. It recognizes a small fixed vocabulary (≥10 words) using custom DSP — no cloud APIs, no ready-made ASR engines. The v1 approach uses **MFCC feature extraction + DTW template matching**, which is the best tradeoff of simplicity, accuracy, and low footprint for isolated command-word recognition.

---

## 2. Product Requirements Document

### 2.1 Problem Statement
Edge and embedded devices often need voice command recognition but cannot rely on cloud APIs due to latency, connectivity, privacy, or resource constraints. Existing ASR engines (Whisper, Vosk, DeepSpeech, etc.) are too heavy or violate the "no ready-made ASR" constraint. A purpose-built lightweight recognizer for a small fixed vocabulary is needed.

### 2.2 Goals
- Recognize ≥10 isolated spoken command words offline
- Run on low-resource edge devices (ARM Linux, Raspberry Pi class)
- Near real-time inference (<500ms end-to-end)
- Custom DSP/ML pipeline — no ASR library dependencies
- Support microphone-based training and inference
- Speaker-dependent v1 (trained per user)

### 2.3 Non-Goals
- Continuous speech recognition
- Speaker-independent recognition (v1)
- Multi-language support (v1)
- Wake-word detection / always-on listening (v1)
- GUI — CLI only for v1

### 2.4 Target Users / Usage Scenario
- Developer prototyping voice-controlled embedded devices
- IoT device accepting 10-20 voice commands (start, stop, up, down, etc.)
- Environments where cloud is unavailable or undesirable

### 2.5 Functional Requirements
| ID | Requirement |
|----|------------|
| FR-1 | Record audio from system microphone |
| FR-2 | Train/enroll words: record N samples per word, extract features, store templates |
| FR-3 | Inference: capture mic input, extract features, match against templates, return predicted word + confidence |
| FR-4 | Reject unknown/low-confidence input |
| FR-5 | CLI commands: `train <word>`, `record-samples`, `train-from`, `test`, `test-from`, `list-words`, `evaluate` |
| FR-6 | Configurable vocabulary list |
| FR-7 | Persist training data and templates to disk |
| FR-8 | Support ≥10 words, ≥3 samples per word |

### 2.6 Non-Functional Requirements
| ID | Requirement | Target |
|----|------------|--------|
| NFR-1 | Inference latency | <500ms from end-of-utterance to result |
| NFR-2 | RAM usage | <50MB working set |
| NFR-3 | Binary size | <10MB stripped |
| NFR-4 | CPU | Single-core capable |
| NFR-5 | Startup time | <1s |
| NFR-6 | Accuracy | ≥85% on trained vocabulary in quiet environment |

### 2.7 Constraints
- Rust only (no C/C++ DSP libs linked, general-purpose crates OK)
- **MSRV**: Rust 1.75.0 (stable, Dec 2023) — ensures `async fn` in traits if needed, wide toolchain availability
- No cloud, no network calls
- No ASR libraries (Whisper, Vosk, DeepSpeech, Kaldi, PocketSphinx, Coqui, wav2vec)
- Minimal crate dependencies — audio I/O and general-purpose only

### 2.8 Acceptance Criteria
1. User can train 10 words with ≥3 samples each via CLI
2. User can run inference and get correct word + confidence for each of the 10 words
3. Unknown words are rejected with "unknown" result
4. Entire pipeline runs offline with no network
5. Inference completes in <500ms
6. Works on x86_64 Linux and ARM Linux (Raspberry Pi)

### 2.9 Risks and Mitigations
| Risk | Mitigation |
|------|-----------|
| Poor accuracy with MFCC+DTW | Use multiple templates per word, tune distance metric |
| Background noise degrades results | Implement energy-based VAD, pre-emphasis, optional spectral subtraction |
| Similar-sounding words confused | Choose distinct vocabulary, add per-pair confusion testing |
| Speaker variation | v1 is speaker-dependent; future: multi-speaker enrollment |
| Microphone quality variance | Normalize amplitude, document mic requirements |
| Threshold tuning is brittle | Provide calibration command, adaptive thresholds |

---

## 3. Recommended Technical Approach

### 3.1 Chosen Approach: MFCC + DTW Template Matching

**Why this fits:**
- **MFCC** is the gold standard for compact speech features — captures vocal tract shape, discards pitch/volume, ~13 coefficients per frame → very compact
- **DTW** handles variable-length utterances naturally — "stop" spoken fast vs slow still matches
- No training phase in the ML sense — just store reference templates
- Extremely low memory: templates are small 2D arrays
- Simple math: FFT → mel filterbank → log → DCT → done
- DTW is O(N×M) per comparison but with 10 words × few templates, this is trivial

### 3.2 Alternatives Considered

| Approach | Pros | Cons | Verdict |
|----------|------|------|---------|
| MFCC + DTW | Simple, proven, low footprint | Speaker-dependent, O(N×M) per template | **Selected for v1** |
| MFCC + k-NN | Simple classifier | Needs fixed-length features (padding/truncation) | Good v2 option |
| MFCC + centroid matching | Fastest inference | Loses temporal structure | Too lossy for v1 |
| Log-mel + tiny CNN | Better generalization | Needs training, larger footprint, complex | v3 option |
| HMM | Classical ASR approach | Complex to implement from scratch | Overkill for 10 words |

### 3.3 Expected Characteristics
- **Accuracy**: 85-95% for 10 distinct words, quiet environment, same speaker
- **Latency**: ~50-200ms for feature extraction + DTW matching
- **Memory**: <5MB for templates of 10 words × 5 samples
- **Limitation**: Speaker-dependent; degrades with noise; similar-sounding words may confuse

### 3.4 Why This Is Better Than Full STT for This Use Case
- **100x smaller footprint** — no neural network weights (Whisper small = 244MB, this = <5MB)
- **No GPU needed** — pure CPU, single-core capable
- **Instant startup** — no model loading
- **Privacy** — no cloud, no large model to exfiltrate
- **Simpler to debug** — every step is inspectable math
- **Sufficient accuracy** — for 10 isolated words, DTW matches full STT accuracy
- **Customizable** — add words by recording, not retraining a neural net

---

## 4. High-Level Architecture

```
┌─────────────────────────────────────────────────────┐
│                      CLI (app-cli)                   │
│   train <word> | record-samples | train-from        │
│   test | test-from | list-words | evaluate          │
└──────────┬──────────────────────────────┬────────────┘
           │                              │
    ┌──────▼──────┐              ┌────────▼────────┐
    │   Training   │              │    Inference     │
    │   Pipeline   │              │    Pipeline      │
    └──────┬──────┘              └────────┬────────┘
           │                              │
    ┌──────▼──────────────────────────────▼────────┐
    │              Audio Capture (cpal)              │
    │         16kHz mono i16 PCM stream              │
    └──────────────────┬───────────────────────────┘
                       │
    ┌──────────────────▼───────────────────────────┐
    │           Preprocessing (dsp-core)            │
    │  DC removal → Pre-emphasis → Normalization    │
    └──────────────────┬───────────────────────────┘
                       │
    ┌──────────────────▼───────────────────────────┐
    │         VAD / Endpoint Detection (vad)        │
    │   Energy-based voice activity detection        │
    │   Detect utterance start/end                   │
    └──────────────────┬───────────────────────────┘
                       │
    ┌──────────────────▼───────────────────────────┐
    │       Feature Extraction (features)           │
    │   Framing → Hamming window → FFT →            │
    │   Mel filterbank → Log → DCT → MFCCs           │
    │   + CMN + delta + delta-delta coefficients     │
    └──────────────────┬───────────────────────────┘
                       │
           ┌───────────┴───────────┐
           │                       │
    ┌──────▼──────┐        ┌───────▼───────┐
    │  Template    │        │   Recognizer   │
    │  Store       │        │   (DTW match)  │
    │  (persist)   │        │   + confidence │
    └─────────────┘        └───────────────┘
```

### 4.1 Component Responsibilities

| Component | Responsibility |
|-----------|---------------|
| **app-cli** | CLI parsing (clap), orchestrates training/inference flows |
| **audio** | Mic capture via `cpal`, produces PCM i16 buffers |
| **dsp** | Pre-emphasis, DC removal, amplitude normalization, framing, windowing |
| **vad** | Energy-based VAD, utterance endpoint detection |
| **features** | FFT (rustfft), mel filterbank, log energy, DCT → MFCC vectors, CMN, delta/delta-delta |
| **recognizer** | DTW distance computation, mean-template matching, confidence scoring, rejection |
| **classifier** | CNN model definition, dataset loading, data augmentation, training loop, inference (burn 0.20) |
| **store** | Serialize/deserialize templates and raw audio (WAV), vocabulary config |
| **common** | Shared types, error types, config structures |

---

## 5. Rust Workspace Structure

```
speeko/
├── Cargo.toml                  # Workspace root
├── README.md
├── config/
│   └── speeko.toml             # Default config (sample rate, MFCC params, thresholds)
├── vocabulary.txt              # Default word list (one word per line)
├── docs/
│   ├── prodict-requirements.md # (existing)
│   ├── architecture.md         # Architecture doc
│   └── design.md               # This design document
├── crates/
│   ├── app-cli/
│   │   ├── Cargo.toml
│   │   └── src/
│   │       └── main.rs         # CLI entry point (clap)
│   ├── audio/
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── capture.rs      # Mic capture via cpal
│   │       └── wav.rs          # WAV read/write (simple custom impl or hound)
│   ├── dsp/
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── preprocess.rs   # DC removal, pre-emphasis, normalization
│   │       ├── framing.rs      # Frame slicing + Hamming window
│   │       └── fft.rs          # FFT wrapper (rustfft)
│   ├── vad/
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       └── energy_vad.rs   # Energy + ZCR-based VAD
│   ├── features/
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── mel.rs          # Mel filterbank
│   │       ├── mfcc.rs         # MFCC extraction pipeline
│   │       ├── cmn.rs          # Cepstral mean normalization
│   │       └── delta.rs        # Delta/delta-delta coefficients
│   ├── recognizer/
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── dtw.rs          # DTW algorithm
│   │       ├── averaging.rs    # Mean template computation
│   │       ├── matcher.rs      # Template matching + scoring
│   │       └── confidence.rs   # Confidence computation + rejection
│   ├── store/
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── templates.rs    # Template persistence (bincode/JSON)
│   │       └── vocabulary.rs   # Vocabulary config loading
│   ├── classifier/
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── model.rs         # CNN model definition (KeywordCnn)
│   │       ├── dataset.rs       # Dataset loading, vocab mapping, train/val split
│   │       ├── augment.rs       # Data augmentation (time stretch, noise, shift, freq mask)
│   │       ├── training.rs      # Training loop, optimizer, early stopping
│   │       └── inference.rs     # CnnRecognizer for prediction
│   └── common/
│       ├── Cargo.toml
│       └── src/
│           ├── lib.rs
│           ├── types.rs        # AudioFrame, MfccFrame, Template, RecognitionResult
│           ├── error.rs        # SpeekError enum
│           └── config.rs       # Config structs + defaults
├── data/                       # Runtime data directory (gitignored)
│   ├── templates/              # Saved per-word templates
│   └── recordings/             # Raw WAV recordings (train/test sets)
├── tests/
│   ├── integration/
│   │   └── pipeline_test.rs
│   └── fixtures/               # Pre-recorded WAV test files
└── benches/
    └── inference_bench.rs
```

### 5.1 External Crate Dependencies (Minimal)

| Crate | Purpose | Justification |
|-------|---------|---------------|
| `cpal` | Cross-platform audio I/O | Standard Rust audio capture, not ASR |
| `rustfft` | FFT computation | General math, not speech-specific |
| `clap` | CLI argument parsing | Standard CLI crate |
| `serde` + `serde_json` / `bincode` | Serialization | Template persistence |
| `hound` | WAV file I/O | Simple audio file format, not ASR |
| `toml` | Config file parsing | Standard config format |
| `anyhow` | Error handling | Ergonomic errors |
| `log` | Logging facade | Standard Rust logging, zero-cost when disabled |
| `env_logger` | Logger implementation | Configurable via `RUST_LOG` |
| `ctrlc` | Signal handling | Graceful Ctrl+C shutdown during recording/inference |

No speech recognition crates. All DSP (MFCC, mel filterbank, DCT, DTW) implemented from scratch.

### 5.2 Pinned Dependency Versions

| Crate | Version | Notes |
|-------|---------|-------|
| `cpal` | `0.15` | Latest stable; ALSA/PulseAudio/WASAPI/CoreAudio support |
| `rustfft` | `6.2` | Stable API, `f32` planner, no-std-compatible internals |
| `clap` | `4.5` | Derive macro API, minimal footprint with `derive` feature only |
| `serde` | `1.0` | With `derive` feature |
| `bincode` | `1.3` | Compact binary serialization for templates |
| `hound` | `3.5` | Simple WAV read/write, no dependencies |
| `toml` | `0.8` | Config parsing |
| `anyhow` | `1.0` | Ergonomic error handling |
| `log` | `0.4` | Logging facade |
| `env_logger` | `0.11` | Concrete logger for CLI; configurable via `RUST_LOG` |
| `ctrlc` | `3.4` | Ctrl+C signal handling for graceful shutdown |
| `burn` | `0.20` | CNN backend: tensor ops, autodiff, ndarray backend, training utilities |
| `rand` | `0.8` | Data augmentation random number generation |

All versions pinned with `~` (compatible) in Cargo.toml to avoid unexpected breakage. `Cargo.lock` committed to repo since this is an application.

---

## 6. Algorithm Details

### 6.1 Audio Parameters
| Parameter | Value | Rationale |
|-----------|-------|-----------|
| Sample rate | 16,000 Hz | Standard for speech, sufficient bandwidth |
| Bit depth | 16-bit signed int | Standard mic output |
| Channels | Mono | Sufficient for command recognition |
| Max utterance length | 2 seconds | Isolated words are short |
| Recording buffer | 3 seconds | With silence padding |

### 6.2 Preprocessing
1. **DC offset removal**: Subtract mean of entire utterance
2. **Pre-emphasis filter**: `y[n] = x[n] - 0.97 * x[n-1]` — boosts high frequencies for better MFCC
3. **Amplitude normalization**: Scale to [-1.0, 1.0] float range

### 6.3 Framing & Windowing
| Parameter | Value |
|-----------|-------|
| Frame length | 25ms = 400 samples @ 16kHz |
| Frame step (hop) | 10ms = 160 samples |
| Window function | Hamming: `0.54 - 0.46 * cos(2π * n / (N-1))` |
| Resulting frames | ~100 frames per 1s utterance |

### 6.4 Voice Activity Detection (VAD)
- **Energy-based**: compute short-time energy per frame
- **Algorithm**:
  1. Estimate silence energy from the lowest 20% of frame energies (skip first 3 frames)
  2. Silence threshold = max(min_floor, silence_energy * threshold_factor)
  3. Mark frames above threshold as active
  4. Apply minimum utterance duration (200ms) and hangover (100ms)
  5. Cap max utterance length and center on peak energy region

### 6.5 Feature Extraction (MFCC)
Per frame:
1. Apply Hamming window
2. Compute N-point FFT (N=512, zero-padded from 400)
3. Compute power spectrum: `|FFT|²`
4. Apply **26 triangular mel-scaled filters** spanning 0–8000 Hz
   - Mel scale: `mel(f) = 2595 * log10(1 + f/700)`
5. Take log of filterbank energies
6. Apply **DCT** to get 13 MFCC coefficients (keep coefficients 1–13, discard 0th)
7. **CMN**: subtract per-utterance mean from each coefficient
8. **Δ + ΔΔ**: append delta and delta-delta coefficients

**Output**: Each utterance → matrix of shape `[num_frames × 39]`

### 6.6 DTW Template Matching
- **Distance metric**: Weighted Euclidean distance (lower MFCC coefficients weighted more)
- **DTW algorithm**: Standard dynamic programming
  - Cost matrix `D[i][j] = dist(query[i], template[j]) + min(D[i-1][j], D[i][j-1], D[i-1][j-1])`
  - Sakoe-Chiba band constraint (width = 20% of max length) to limit warping and speed up
- **Mean-template matching**: Compute a mean template per word by interpolating templates to the
  median length and averaging. One DTW per word.
- **Score**: `score(word) = dtw_distance(query, mean_template[word])`
- **Prediction**: word with lowest score

### 6.7 Confidence & Rejection
- **Relative confidence**: log-ratio of best vs second-best distances
  - `relative = ln(second/best) / ln(2)` (clamped to [0,1])
- **Absolute confidence**: `absolute = 1.0 - (best_score / max_distance)`
- **Blend**: `confidence = 0.3 * relative + 0.7 * absolute`
- **Rejection threshold**: configurable, default 0.2
  - If `confidence < threshold` → return "unknown"
- **Absolute distance check**: if `best_score > max_distance` → reject regardless
  - Guards against all words being poor matches

### 6.8 CNN Classifier (Alternative Backend)

An optional 1D CNN classifier is available as an alternative to DTW, implemented in the `classifier` crate using the [burn](https://burn.dev) crate (v0.20).

**Architecture** (`KeywordCnn`):
- Input: `[batch, 39, max_frames]` — padded/truncated MFCC sequences
- 3× Conv1d blocks: Conv1d → BatchNorm → ReLU → MaxPool1d
  - Channels: 39→64→128→256, kernel=3, padding=1, pool=2
- AdaptiveAvgPool1d → flatten
- FC1: 256→128 + ReLU + Dropout(0.3)
- FC2: 128→num_classes (softmax output)

**Data Augmentation** (applied during training):
- **Time stretch**: resample MFCC frames by random factor 0.8–1.2
- **Gaussian noise**: add N(0, 0.005) noise to coefficients
- **Time shift**: circular shift by ±5 frames
- **Frequency masking**: zero out 1–3 random coefficient bands

**Training**:
- Optimizer: Adam (lr=0.001 default)
- Loss: cross-entropy
- Early stopping: patience=10 epochs on validation accuracy
- Model saved as burn record + vocabulary JSON in `data/models/`

**Inference**:
- Load model + vocabulary from disk
- Pad/truncate query MFCC to `max_frames`
- Forward pass → softmax → argmax → word prediction
- Confidence = max softmax probability
- Returns `RecognitionResult` (same type as DTW)

**Mode switching**: Set `recognizer.mode = "cnn"` in `speeko.toml`. The `test`, `test-from`, and `evaluate` commands automatically dispatch to the CNN recognizer.

---

## 7. Data Flow

### 7.1 Training Flow (Direct Mic)
```
User says: speeko train "start"
  → CLI prompts "Say 'start' now..."
  → Audio capture: 3s recording @ 16kHz mono
  → Preprocessing: DC removal → pre-emphasis → normalize
  → VAD: detect utterance boundaries → trim
  → Feature extraction: framing → FFT → mel → MFCC → CMN → Δ/ΔΔ → [F×39]
  → Save: raw WAV → data/recordings/start/sample_003.wav
  → Save: MFCC template → data/templates/start/template_003.bin
  → Repeat for N samples
  → Print summary: "Trained 'start' with 5 samples"
```

### 7.2 Training Flow (Batch WAV Workflow)
```
User says: speeko record-samples start stop yes no --samples 5 -o data/train_wavs
  → Record WAVs for all words into data/train_wavs/<word>/sample_###.wav
  → User reviews WAVs and deletes mis-pronounced recordings
User says: speeko train-from data/train_wavs --reset
  → Loads WAVs, preprocess + VAD + MFCC + CMN + Δ/ΔΔ
  → Saves templates per word
```

### 7.3 Inference Flow (Live Mic)
```
User says: speeko test
  → CLI prompts "Listening..."
  → Audio capture: continuous 3s sliding window
  → Preprocessing: DC removal → pre-emphasis → normalize
  → VAD: detect utterance → trim
  → Feature extraction: → [F×39] matrix (MFCC + CMN + Δ/ΔΔ)
  → Recognizer: DTW against mean template per word
  → Scoring: pick best word, compute confidence
  → Confidence check: above threshold? → "Recognized: start (92%)"
                       below threshold? → "Unknown word (confidence too low)"
```

### 7.4 Unknown-Word Rejection Flow
```
  → DTW scores: {start: 45.2, stop: 47.8, open: 89.1, ...}
  → Best: start (45.2), Second-best: stop (47.8)
  → Confidence = 0.3*ln(47.8/45.2)/ln(2) + 0.7*(1 - 45.2/80.0) → LOW
  → Also check: 45.2 > max_distance (80.0)? → NO
  → Result: "Unknown" — neither margin nor absolute distance is acceptable
```

### 7.5 CNN Training Flow
```
User says: speeko cnn-train data/train_wavs --epochs 50
  → Load WAVs from data/train_wavs/<word>/sample_*.wav
  → Preprocess + VAD + MFCC + CMN + Δ/ΔΔ per file
  → Build vocabulary map (word → class index)
  → Split into train/validation sets (80/20)
  → Augment training data (time stretch, noise, shift, freq mask)
  → Pad/truncate all samples to max_frames
  → Train KeywordCnn with Adam optimizer + cross-entropy loss
  → Early stopping on validation accuracy (patience=10)
  → Save model record + vocabulary JSON to data/models/
```

### 7.6 Offline Test Suite (Pre-recorded WAVs)
```
User says: speeko test-from data/test_wavs --verbose
  → Load WAVs from data/test_wavs/<word>/sample_###.wav
  → Preprocess + VAD + MFCC + CMN + Δ/ΔΔ
  → Recognize using mean templates
  → Report per-word accuracy table + average confidence
```

---

## 8. MVP Phase Plan

### Phase 1: Audio Foundation (Est. 2-3 days)
**Deliverables:**
- Workspace scaffolding (all Cargo.toml files, crate stubs)
- `audio` crate: mic capture via cpal, WAV save via hound
- CLI: `speeko record` — records 3s and saves WAV
- `common` crate: shared types, config struct, error types

**Success criteria:** Run `speeko record`, speak, verify WAV plays back correctly.

### Phase 2: Preprocessing + VAD (Est. 2-3 days)
**Deliverables:**
- `dsp` crate: DC removal, pre-emphasis, normalization, framing, Hamming window
- `vad` crate: energy-based VAD with endpoint detection
- CLI: `speeko record --trim` — records, trims silence, saves trimmed WAV

**Success criteria:** VAD correctly trims silence from recordings (verify visually in Audacity or similar).

### Phase 3: Feature Extraction (Est. 3-4 days)
**Deliverables:**
- `dsp` crate: FFT wrapper using rustfft
- `features` crate: mel filterbank, MFCC pipeline, DCT
- CLI: `speeko extract <wav>` — prints MFCC matrix dimensions and values

**Success criteria:** MFCC output for same word spoken twice is more similar than for different words (visual/numeric inspection).

### Phase 4: Template Store + Training Mode (Est. 2-3 days)
**Deliverables:**
- `store` crate: template serialization, vocabulary loading, data directory management
- CLI: `speeko train <word>` — full pipeline: record → preprocess → VAD → MFCC → save template
- CLI: `speeko list-words` — shows trained words and sample counts

**Success criteria:** Train 3 samples of 3 different words, verify templates saved and loadable.

### Phase 5: DTW Recognizer + Inference (Est. 3-4 days)
**Deliverables:**
- `recognizer` crate: DTW algorithm, multi-template matcher, confidence scoring, rejection
- CLI: `speeko test` — full inference pipeline with live mic
- CLI: `speeko evaluate` — batch test against stored recordings

**Success criteria:** ≥80% accuracy on 5 words with 3 training samples each in quiet environment.

### Phase 6: Polish + Full Vocabulary (Est. 2-3 days)
**Deliverables:**
- Train and test with full 10-word vocabulary
- Tune thresholds (VAD energy, DTW rejection, confidence)
- Config file support (speeko.toml)
- Proper error messages and CLI help
- README.md with usage instructions

**Success criteria:** ≥85% accuracy on 10 words, clean CLI UX, documented setup instructions.

### Phase 7: Optimization + Hardening (Est. 2-3 days)
**Deliverables:**
- Buffer reuse, allocation reduction
- Sakoe-Chiba band constraint for DTW
- Release build profiling (binary size, memory, latency)
- Benchmark suite
- Edge device testing (if available)

**Success criteria:** Inference <500ms, RAM <50MB, binary <10MB stripped.

**Total estimated timeline: ~16-23 days for solo developer**

---

## 9. Testing Strategy

### 9.1 Unit Tests
| Component | Tests |
|-----------|-------|
| `dsp` | Pre-emphasis correctness, DC removal, normalization range, framing count/size, Hamming window values |
| `features/mel` | Mel-to-Hz and Hz-to-mel roundtrip, filterbank shape, filter overlap |
| `features/mfcc` | Known-input MFCC output matches expected values, coefficient count |
| `recognizer/dtw` | DTW on identical sequences = 0, known distance for simple sequences, Sakoe-Chiba constraint |
| `recognizer/confidence` | Confidence formula edge cases, rejection threshold logic |
| `vad` | Silent input → no active frames, loud input → all active, mixed → correct trim |
| `store` | Template round-trip serialize/deserialize, vocabulary load |

### 9.2 Integration Tests
- Full pipeline: WAV file → preprocess → VAD → MFCC → DTW → result
- Training + inference end-to-end with fixture WAVs
- Unknown-word rejection with out-of-vocabulary WAVs

### 9.3 Fixture Tests
- Pre-record 10 words × 5 samples as WAV fixtures (committed to repo or downloadable)
- Deterministic: same WAV always produces same MFCC output
- Accuracy benchmark: ≥85% on fixture set

### 9.4 Performance Tests
| Test | Target |
|------|--------|
| MFCC extraction latency | <100ms for 2s utterance |
| DTW single comparison | <10ms |
| Full inference (10 words × 5 templates) | <500ms |
| Memory during inference | <50MB |
| Binary size (stripped release) | <10MB |

### 9.5 Robustness Tests (v2+)
- Noisy environment: add white noise to fixtures, measure accuracy degradation
- Volume variation: quiet vs loud utterances
- Speed variation: fast vs slow speakers

---

## 10. Performance & Footprint Strategy

### 10.1 Allocation Minimization
- Pre-allocate frame buffer, FFT scratch, MFCC output vector at startup
- Reuse buffers across frames — never allocate per-frame
- Use `Vec::with_capacity` for known sizes

### 10.2 Numeric Efficiency
- Use `f32` throughout (not f64) — sufficient precision, faster on ARM
- FFT size 512 (power of 2) for radix-2 efficiency
- Mel filterbank: precompute filter weights once, reuse

### 10.3 DTW Optimization
- Sakoe-Chiba band: limits DTW matrix to diagonal band → O(N×W) instead of O(N×M)
- Two-row DTW: only store current and previous row → O(M) memory instead of O(N×M)
- Early termination: if running cost exceeds best-so-far, prune

### 10.4 Compile Profile
```toml
[profile.release]
opt-level = 3        # Max optimization
lto = true           # Link-time optimization
codegen-units = 1    # Better optimization, slower compile
strip = true         # Strip symbols
panic = "abort"      # Smaller binary, no unwinding
```

### 10.5 Binary Size
- Avoid large frameworks
- Use `#[cfg(feature = ...)]` for optional components
- Consider `cargo-bloat` to identify size contributors

---

## 11. Cross-Compilation & Target Platforms

### 11.1 Supported Targets
| Target | Triple | Notes |
|--------|--------|-------|
| x86_64 Linux (dev) | `x86_64-unknown-linux-gnu` | Primary dev platform |
| ARM64 Linux (Pi 4/5) | `aarch64-unknown-linux-gnu` | Primary edge target |
| ARMv7 Linux (Pi 3/Zero 2) | `armv7-unknown-linux-gnueabihf` | Secondary edge target |
| x86_64 Windows | `x86_64-pc-windows-msvc` | Dev convenience |
| macOS (Apple Silicon) | `aarch64-apple-darwin` | Dev convenience |

### 11.2 Cross-Compilation Setup
```bash
# Install cross-compilation toolchains
rustup target add aarch64-unknown-linux-gnu
rustup target add armv7-unknown-linux-gnueabihf

# Option A: Use `cross` (Docker-based, easiest)
cargo install cross
cross build --release --target aarch64-unknown-linux-gnu

# Option B: Native cross-compiler (faster, needs system packages)
# On Ubuntu: sudo apt install gcc-aarch64-linux-gnu
# Then in .cargo/config.toml:
# [target.aarch64-unknown-linux-gnu]
# linker = "aarch64-linux-gnu-gcc"
cargo build --release --target aarch64-unknown-linux-gnu
```

### 11.3 Platform-Specific Considerations
- **Audio backend**: `cpal` uses ALSA on Linux (requires `libasound2-dev`), WASAPI on Windows, CoreAudio on macOS
- **Cross-compile ALSA**: When using `cross`, the Docker image includes ALSA dev headers. For native cross-compile, install `libasound2-dev:arm64` or link statically
- **CI matrix**: GitHub Actions workflow should build for `x86_64-linux`, `aarch64-linux`, `x86_64-windows` at minimum
- **Feature flag**: `--no-default-features --features alsa` to control audio backend on embedded

### 11.4 Deployment on Edge Device
1. Cross-compile release binary
2. `scp` binary + `config/speeko.toml` + `vocabulary.txt` to device
3. Ensure ALSA is available: `aplay -l` should list capture devices
4. Run `speeko train <word>` directly on device (mic must be connected)
5. Template data is portable across same-architecture builds

---

## 12. Logging & Diagnostics

### 12.1 Logging Framework
- Use `log` crate (facade) + `env_logger` (concrete implementation)
- Controlled via `RUST_LOG` environment variable — zero overhead when disabled
- No custom logging framework — standard Rust ecosystem approach

### 12.2 Log Levels by Component
| Component | `ERROR` | `WARN` | `INFO` | `DEBUG` | `TRACE` |
|-----------|---------|--------|--------|---------|--------|
| `audio` | Mic open failure, buffer overrun | Sample rate mismatch (resampled) | Device selected, recording start/stop | Buffer sizes, callback timing | Raw sample values |
| `dsp` | — | — | — | Pre-emphasis params, frame count | Per-frame values |
| `vad` | — | No speech detected in recording | Utterance boundaries (start/end ms) | Energy values, threshold | Per-frame energy |
| `features` | FFT error | — | MFCC matrix shape | Mel filter params | Per-frame MFCC values |
| `recognizer` | — | Low-confidence result | Prediction + confidence | DTW distances per word | Cost matrix values |
| `store` | File I/O failure | Template version mismatch | Templates loaded/saved count | File paths | Serialized sizes |
| `app-cli` | Fatal errors | Config warnings | Command execution summary | Timing info | — |

### 12.3 Diagnostic CLI Flags
- `--verbose` / `-v`: Sets `RUST_LOG=info` (default is `warn`)
- `-vv`: Sets `RUST_LOG=debug`
- `-vvv`: Sets `RUST_LOG=trace`
- `speeko diagnose`: Prints audio device info, config, template counts, estimated latency
- `speeko extract --dump <wav>`: Dumps full MFCC matrix to stdout/file for debugging

### 12.4 Performance Timing
- Instrument key pipeline stages with `std::time::Instant` at `DEBUG` level:
  - Audio capture duration
  - VAD processing time
  - MFCC extraction time
  - DTW matching time (total + per-word)
  - End-to-end inference time
- Output as structured log: `DEBUG recognizer: inference completed in 142ms (vad=8ms, mfcc=45ms, dtw=89ms)`

---

## 13. CLI Edge Cases & Error Handling

### 13.1 CLI Command Specifications

```
speeko train <word> [--samples N] [--duration SECS] [--append]
speeko test [--continuous] [--timeout SECS]
speeko list-words [--detailed]
speeko evaluate [--wav-dir PATH]
speeko record [--output PATH] [--duration SECS] [--trim]
speeko extract <wav> [--dump]
speeko diagnose
speeko calibrate
speeko reset <word|--all> [--confirm]
```

### 13.2 Edge Cases & Handling

| Scenario | Behavior |
|----------|----------|
| `train <word>` with no microphone | Error: "No audio input device found. Run `speeko diagnose` to check." Exit code 1 |
| `train <word>` but VAD detects no speech | Warn: "No speech detected. Please speak louder or check mic. Try again? [y/N]" |
| `train <word>` with word not in vocabulary | Warn: "'xyz' is not in vocabulary.txt. Add it? [y/N]" or `--force` flag to skip |
| `test` with no trained templates | Error: "No templates found. Run `speeko train <word>` first." Exit code 1 |
| `test` with only partial vocabulary trained | Warn at start: "Only 4/10 words trained. Recognition limited to: start, stop, up, down" |
| `train` recording is too short (<200ms speech) | Warn: "Recording too short (150ms). Please speak the full word. Retry? [y/N]" |
| `train` recording is clipped (amplitude hits max) | Warn: "Audio appears clipped. Move further from mic or reduce volume." |
| Config file missing | Use hardcoded defaults, log `INFO`: "No speeko.toml found, using defaults" |
| Config file has invalid values | Error with specific field: "Invalid config: sample_rate must be 8000-48000, got 0" |
| Vocabulary file missing | Error: "vocabulary.txt not found at <path>. Create it with one word per line." |
| Vocabulary file has duplicates | Warn: "Duplicate word 'stop' in vocabulary.txt (line 7). Ignoring duplicate." |
| Template directory corrupted / unreadable | Error per-file: "Failed to load template start/003.bin: invalid format. Skipping." Continue with valid templates |
| `Ctrl+C` during recording | Graceful shutdown: discard partial recording, print "Recording cancelled." |
| `Ctrl+C` during `test --continuous` | Graceful shutdown: print summary of session (N words recognized, accuracy if known) |
| Disk full during template save | Error: "Failed to save template: No space left on device" Exit code 1 |
| `reset <word>` without `--confirm` | Prompt: "Delete all templates for 'start'? This cannot be undone. [y/N]" |
| `evaluate` with no WAV fixtures | Error: "No WAV files found in <path>. Provide test recordings." |

### 13.3 Exit Codes
| Code | Meaning |
|------|---------|
| 0 | Success |
| 1 | Runtime error (no mic, disk full, corrupt data) |
| 2 | CLI usage error (invalid args, missing required args) |
| 3 | No speech detected / recognition failure |

### 13.4 Signal Handling
- Register `Ctrl+C` handler via `ctrlc` crate (add to dependencies)
- Set a global `AtomicBool` flag; audio capture loop checks it each iteration
- Ensures clean shutdown: close audio stream, flush any pending writes

---

## 14. Risks & Design Tradeoffs

| Issue | Impact | Mitigation |
|-------|--------|-----------|
| **Speaker dependence** | Won't work for other speakers | v1 accepted; v2 add multi-speaker enrollment |
| **Background noise** | Accuracy drops significantly | Energy VAD helps; v2 add spectral subtraction |
| **Similar words** (e.g., "right"/"light") | Confusion | Choose distinct vocabulary; test pairwise confusion matrix |
| **Microphone quality** | Feature variance | Normalize amplitude; document requirements |
| **Threshold tuning** | Too strict = high rejection; too loose = false positives | Provide `calibrate` command; adaptive thresholds |
| **Few training samples** | Poor template coverage | Require ≥3 samples; recommend 5 |
| **Variable speaking speed** | Increased DTW distances | DTW handles this inherently; widen Sakoe-Chiba band if needed |
| **Utterance segmentation errors** | Features include silence | Tune VAD thresholds; add padding |
| **Portability** | cpal may not work on all embedded devices | cpal supports ALSA; document supported platforms |

---

## 15. Deliverable Files

```
speeko/
├── README.md
├── Cargo.toml
├── config/speeko.toml
├── vocabulary.txt
├── docs/
│   ├── prodict-requirements.md    # (existing)
│   ├── architecture.md
│   └── design.md
├── crates/
│   ├── app-cli/    (Cargo.toml + src/main.rs)
│   ├── audio/      (Cargo.toml + src/{lib,capture,wav}.rs)
│   ├── dsp/        (Cargo.toml + src/{lib,preprocess,framing,fft}.rs)
│   ├── vad/        (Cargo.toml + src/{lib,energy_vad}.rs)
│   ├── features/   (Cargo.toml + src/{lib,mel,mfcc,delta}.rs)
│   ├── recognizer/ (Cargo.toml + src/{lib,dtw,matcher,confidence}.rs)
│   ├── store/      (Cargo.toml + src/{lib,templates,vocabulary}.rs)
│   └── common/     (Cargo.toml + src/{lib,types,error,config}.rs)
├── data/           (.gitignored runtime data)
├── tests/
│   └── integration/
├── benches/
│   └── inference_bench.rs
└── .gitignore
```

---

## 16. Starter Implementation Scope

The initial code generation will include **working stubs and initial implementations** for:

| Component | Implementation Level |
|-----------|---------------------|
| Workspace Cargo.toml | Complete |
| All crate Cargo.toml files | Complete |
| `common` types/errors/config | Complete |
| `audio` capture + WAV I/O | Working implementation |
| `dsp` preprocess + framing | Working implementation |
| `vad` energy-based | Working implementation |
| `features` MFCC pipeline | Working implementation (FFT → mel → log → DCT) |
| `recognizer` DTW | Working implementation |
| `recognizer` matcher + confidence | Working implementation |
| `store` templates + vocabulary | Working implementation |
| `app-cli` full CLI | Working implementation (train, test, list-words, evaluate) |
| Config file + vocabulary | Complete |
| README | Complete |

All DSP math (mel filterbank, DCT, MFCC, DTW) will be implemented from scratch — no speech-processing crates.

---

## Summary of Implementation Order

1. Create workspace scaffolding and all Cargo.toml files (with pinned versions)
2. Implement `common` crate (types, errors, config)
3. Implement `audio` crate (mic capture, WAV I/O)
4. Implement `dsp` crate (preprocessing, framing, FFT wrapper)
5. Implement `vad` crate (energy-based endpoint detection)
6. Implement `features` crate (mel filterbank, MFCC)
7. Implement `store` crate (template persistence, vocabulary)
8. Implement `recognizer` crate (DTW, matcher, confidence)
9. Implement `app-cli` (CLI orchestration with edge case handling, logging, signal handling)
10. Add config files, vocabulary, README
11. Add integration tests and benchmarks
12. Verify cross-compilation for ARM targets
