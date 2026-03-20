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
| FR-5 | CLI commands: `train <word>`, `test`, `list-words`, `evaluate`, `calibrate` |
| FR-6 | Configurable vocabulary list (add/remove words via `vocabulary.txt` or CLI) |
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
│   train <word> | test | list-words | evaluate | calibrate │
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
    │   Mel filterbank → Log → DCT → 13 MFCCs      │
    │   + optional delta coefficients               │
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
| **features** | FFT (rustfft), mel filterbank, log energy, DCT → MFCC vectors |
| **recognizer** | DTW distance computation, multi-template matching, confidence scoring, rejection |
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
│   ├── product-requirements.md # (existing)
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
│   │       └── delta.rs        # Delta/delta-delta coefficients
│   ├── recognizer/
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── dtw.rs          # DTW algorithm
│   │       ├── matcher.rs      # Multi-template matching + scoring
│   │       └── confidence.rs   # Confidence computation + rejection
│   ├── store/
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── templates.rs    # Template persistence (bincode/JSON)
│   │       └── vocabulary.rs   # Vocabulary config loading
│   └── common/
│       ├── Cargo.toml
│       └── src/
│           ├── lib.rs
│           ├── types.rs        # AudioFrame, MfccFrame, Template, RecognitionResult
│           ├── error.rs        # SpeekError enum
│           └── config.rs       # Config structs + defaults
├── data/                       # Runtime data directory (gitignored)
│   ├── templates/              # Saved per-word templates
│   └── recordings/             # Raw WAV recordings
├── tests/
│   ├── integration/
│   │   └── pipeline_test.rs
│   └── fixtures/               # Pre-recorded WAV test files
└── benches/
    └── inference_bench.rs
```

### 5.1 External Crate Dependencies (Minimal)

| Crate | Version | Purpose | Justification |
|-------|---------|---------|---------------|
| `cpal` | `0.17` | Cross-platform audio I/O | Standard Rust audio capture, not ASR. Pre-1.0; pin to `0.17.x` to avoid surprise API changes. |
| `rustfft` | `6.4` | FFT computation | General math, not speech-specific. Stable 6.x line; has SIMD support (AVX/SSE/NEON). |
| `clap` | `4.6` | CLI argument parsing | Standard CLI crate. Use `derive` feature. Requires Rust 1.85+. |
| `serde` | `1.0` | Serialization framework | Template persistence. Stable, no breaking changes. |
| `serde_json` | `1.0` | JSON serialization | Human-readable template format for debugging/inspection. |
| `hound` | `3.5` | WAV file I/O | Simple audio file format, not ASR. |
| `toml` | `1.0` | Config file parsing | Standard config format. Supports TOML spec 1.1.0. |
| `anyhow` | `1.0` | Error handling | Ergonomic top-level errors in `app-cli` crate only. |
| `thiserror` | `2.0` | Typed error derives | Used in library crates (`dsp`, `features`, `recognizer`, `store`) for structured `SpeekError` variants. Pairs with `anyhow` at the CLI boundary. |
| `tracing` | `0.1` | Structured logging | Lightweight, level-filtered, can be compiled out in release. Essential for debugging DSP pipelines on embedded devices. |
| `tracing-subscriber` | `0.3` | Log output formatting | Console output for `tracing`. Only needed by `app-cli`. |

> **Note on `bincode`**: bincode 3.0 is available but introduces a completely new API (`Encode`/`Decode` derive traits replacing `serde`-based `serialize`/`deserialize`). For v1, we use **`serde_json`** for template persistence — templates are small and the human-readable format simplifies debugging. If binary size on disk becomes a concern (unlikely at 10 words × 5 templates), bincode 3.0 or a raw binary format can be added in v2.

No speech recognition crates. All DSP (MFCC, mel filterbank, DCT, DTW) implemented from scratch.

### 5.2 Rust Edition & Minimum Supported Rust Version (MSRV)

```toml
[workspace.package]
edition = "2021"
rust-version = "1.85"
```

- **Edition 2021**: Stable, widely supported, no compatibility concerns.
- **MSRV 1.85**: Required by `clap 4.6`. Verified compatible with all other dependencies.
- **Future**: Edition 2024 is available but not yet widely adopted; upgrade when ecosystem stabilizes.

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
- **Zero-crossing rate (ZCR)**: secondary feature for unvoiced detection
- **Algorithm**:
  1. Compute energy of first 10 frames (assumed silence) → silence threshold = mean + 2σ
  2. Mark frames above threshold as active
  3. Apply minimum utterance duration (200ms) and hangover (100ms)
  4. Trim to active region with small padding (50ms each side)

### 6.5 Feature Extraction (MFCC)
Per frame:
1. Apply Hamming window
2. Compute N-point FFT (N=512, zero-padded from 400)
3. Compute power spectrum: `|FFT|²`
4. Apply **26 triangular mel-scaled filters** spanning 0–8000 Hz
   - Mel scale: `mel(f) = 2595 * log10(1 + f/700)`
5. Take log of filterbank energies
6. Apply **DCT** to get 13 MFCC coefficients (keep coefficients 1–13, discard 0th)
7. *Optional (v2)*: compute delta and delta-delta coefficients

**Output**: Each utterance → matrix of shape `[num_frames × 13]`

### 6.6 DTW Template Matching
- **Distance metric**: Euclidean distance between MFCC vectors
- **DTW algorithm**: Standard dynamic programming
  - Cost matrix `D[i][j] = dist(query[i], template[j]) + min(D[i-1][j], D[i][j-1], D[i-1][j-1])`
  - Sakoe-Chiba band constraint (width = 20% of max length) to limit warping and speed up
- **Multi-template**: Store K templates per word (K=3–5), compute DTW to each, take minimum distance
- **Score**: `score(word) = min(dtw_distance to each template of that word)`
- **Prediction**: word with lowest score

### 6.7 Confidence & Rejection
- **Confidence formula**: `confidence = 1.0 - (best_score / second_best_score)`
  - High confidence → best match is much better than second best
  - **Edge case: single trained word** — no `second_best_score` available. Fall back to absolute distance check only: if `best_score ≤ max_acceptable_distance` → accept with confidence = 1.0, otherwise reject.
  - **Guard**: `confidence = max(0.0, 1.0 - (best_score / second_best_score))` — prevents negative values if scores are inverted due to floating-point edge cases.
- **Rejection threshold**: configurable, default 0.3
  - If `confidence < threshold` → return "unknown"
- **Absolute distance check**: if `best_score > max_acceptable_distance` → reject regardless
  - Guards against all words being poor matches

---

## 7. Data Flow

### 7.1 Training Flow
```
User says: speeko train "start"
  → CLI prompts "Say 'start' now..."
  → Audio capture: 3s recording @ 16kHz mono
  → Preprocessing: DC removal → pre-emphasis → normalize
  → VAD: detect utterance boundaries → trim
  → Feature extraction: framing → FFT → mel → MFCC → [F×13] matrix
  → Save: raw WAV → data/recordings/start/sample_003.wav
  → Save: MFCC template → data/templates/start/template_003.bin
  → Repeat for N samples
  → Print summary: "Trained 'start' with 5 samples"
```

### 7.2 Inference Flow
```
User says: speeko test
  → CLI prompts "Listening..."
  → Audio capture: continuous 3s sliding window
  → Preprocessing: DC removal → pre-emphasis → normalize
  → VAD: detect utterance → trim
  → Feature extraction: → [F×13] MFCC matrix
  → Recognizer: DTW against all templates for all words
  → Scoring: pick best word, compute confidence
  → Confidence check: above threshold? → "Recognized: start (92%)"
                       below threshold? → "Unknown word (confidence too low)"
```

### 7.3 Unknown-Word Rejection Flow
```
  → DTW scores: {start: 45.2, stop: 47.8, open: 89.1, ...}
  → Best: start (45.2), Second-best: stop (47.8)
  → Confidence = 1 - 45.2/47.8 = 0.054 → LOW
  → Also check: 45.2 > max_acceptable_distance (40.0)? → YES
  → Result: "Unknown" — neither margin nor absolute distance is acceptable
```

### 7.4 Evaluate Command Flow
```
User says: speeko evaluate
  → Load all stored recordings from data/recordings/
  → For each word directory, use stored WAV files as labeled test data
  → Hold-one-out: for each sample, match against all OTHER templates (leave-one-out cross-validation)
  → OR: match against all templates (if separate test recordings exist)
  → For each test sample:
    → Preprocess → VAD → MFCC → DTW against all templates
    → Record: expected word, predicted word, confidence
  → Output:
    → Per-word accuracy: "start: 4/5 (80%), stop: 5/5 (100%), ..."
    → Confusion matrix: which words are confused with which
    → Overall accuracy: "Total: 43/50 (86%)"
    → List of misclassified samples with expected vs predicted
  → Exit code: 0 if overall accuracy ≥ threshold (default 85%), 1 otherwise
```

### 7.5 Calibrate Command Flow
```
User says: speeko calibrate
  → Run evaluate internally to collect DTW distance distributions
  → For each word, compute:
    → Mean and stddev of "correct match" distances
    → Mean and stddev of "incorrect match" distances
  → Auto-tune:
    → max_acceptable_distance = mean(correct) + 3 * stddev(correct)
    → rejection_threshold = optimal separation point between correct/incorrect distributions
  → Save tuned thresholds to config/speeko.toml
  → Print: "Calibrated thresholds saved. max_distance=42.5, rejection_threshold=0.35"
```

### 7.6 Vocabulary Management Flow
```
Vocabulary loading:
  → Read vocabulary.txt (one word per line, # for comments)
  → Validate: no duplicates, only alphanumeric + hyphens, ≥1 word
  → On train <word>: warn if word is not in vocabulary.txt (but allow it)
  → On list-words: show vocabulary.txt words with training status:
    → "start" — trained (5 samples) ✓
    → "stop"  — trained (3 samples) ✓
    → "open"  — not yet trained ✗
  → Adding a word: add line to vocabulary.txt, then train samples
  → Removing a word: remove from vocabulary.txt + optionally delete templates/recordings
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
- CLI: `speeko evaluate` — batch test against stored recordings, output per-word accuracy, confusion matrix, and overall accuracy percentage

**Success criteria:** ≥80% accuracy on 5 words with 3 training samples each in quiet environment.

### Phase 6: Polish + Full Vocabulary (Est. 2-3 days)
**Deliverables:**
- Train and test with full 10-word vocabulary
- Tune thresholds (VAD energy, DTW rejection, confidence)
- CLI: `speeko calibrate` — auto-tune rejection thresholds from training data
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

### 10.6 Cross-Compilation (ARM Targets)

Acceptance criteria #6 requires ARM Linux (Raspberry Pi) support. Cross-compilation strategy:

**Target triples:**
- `aarch64-unknown-linux-gnu` — Raspberry Pi 3/4/5 (64-bit)
- `armv7-unknown-linux-gnueabihf` — Raspberry Pi 2/3 (32-bit)

**System dependencies:**
- `cpal` requires ALSA on Linux → cross-compile needs `libasound2-dev` for the target architecture
- Install via cross-compilation sysroot or use `cross` tool (Docker-based)

**Recommended approach:**
```bash
# Option 1: Using `cross` (simplest)
cargo install cross
cross build --release --target aarch64-unknown-linux-gnu

# Option 2: Manual cross-compilation
rustup target add aarch64-unknown-linux-gnu
# Install cross-linker and ALSA dev headers for target
sudo apt install gcc-aarch64-linux-gnu libasound2-dev:arm64
# Set linker in .cargo/config.toml:
# [target.aarch64-unknown-linux-gnu]
# linker = "aarch64-linux-gnu-gcc"
cargo build --release --target aarch64-unknown-linux-gnu
```

**CI verification:** Add a GitHub Actions job that cross-builds for ARM (build-only, no test) to catch compilation issues early.

---

## 11. Risks & Design Tradeoffs

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

## 12. Deliverable Files

```
speeko/
├── README.md
├── Cargo.toml
├── config/speeko.toml
├── vocabulary.txt
├── docs/
│   ├── product-requirements.md    # (existing)
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

## 13. Starter Implementation Scope

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

1. Create workspace scaffolding and all Cargo.toml files
2. Implement `common` crate (types, errors, config)
3. Implement `audio` crate (mic capture, WAV I/O)
4. Implement `dsp` crate (preprocessing, framing, FFT wrapper)
5. Implement `vad` crate (energy-based endpoint detection)
6. Implement `features` crate (mel filterbank, MFCC)
7. Implement `store` crate (template persistence, vocabulary)
8. Implement `recognizer` crate (DTW, matcher, confidence)
9. Implement `app-cli` (CLI orchestration)
10. Add config files, vocabulary, README
11. Add integration tests and benchmarks
