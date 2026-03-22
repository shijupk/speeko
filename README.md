# Speeko

**Offline spoken-word recognition for edge devices.**

Speeko is a small-footprint, offline, isolated-word recognizer written in Rust. It supports two recognition backends:

- **DTW** (default) — MFCC + Dynamic Time Warping template matching
- **CNN** — 1D Convolutional Neural Network classifier (via [burn](https://burn.dev) crate)

No cloud APIs, no external ASR engines.

## Features

- **Offline only** — no network, no cloud, no external APIs
- **Custom DSP** — MFCC extraction and DTW matching implemented from scratch
- **CNN classifier** — optional 1D CNN backend with data augmentation (burn 0.20)
- **Small footprint** — <10MB binary, <50MB RAM
- **Near real-time** — <500ms inference latency
- **Speaker-dependent** — trained per user for best accuracy
- **Configurable** — vocabulary, thresholds, and DSP parameters via TOML config
- **Switchable backends** — toggle between DTW and CNN via config

## Quick Start

### Prerequisites

- Rust 1.75+ (`rustup install stable`)
- A working microphone
- Linux: `libasound2-dev` (`sudo apt install libasound2-dev`)
- Windows: WASAPI (built-in)
- macOS: CoreAudio (built-in)

### Build

```bash
cargo build --release
```

The binary is at `target/release/speeko`.

### Train (Recommended: batch WAV workflow)

Record all words first, review/delete bad takes, then train from the folder:

```bash
# 1) Record WAVs for all words (review/delete any bad recordings)
speeko record-samples start stop yes no --samples 5 -o data/train_wavs

# 2) Train from curated WAVs
speeko train-from data/train_wavs --reset
```

You can still use the direct mic training flow:

```bash
speeko train start --samples 50
speeko train stop --samples 50
speeko train open --samples 50
# ... repeat for each word in vocabulary.txt
```

Each invocation prompts you to speak the word multiple times. Speak clearly in a quiet environment.

### Test

Recognize a single spoken word:

```bash
speeko test
```

Or run continuously:

```bash
speeko test --continuous
```

Run a repeatable test suite from pre-recorded WAVs:

```bash
# Record a test set first
speeko record-samples start stop yes no --samples 3 -o data/test_wavs

# Evaluate from WAVs (per-word accuracy table)
speeko test-from data/test_wavs --verbose
```

### CNN Classifier (optional)

Train a CNN model from the same WAV folder structure:

```bash
# Train CNN (uses data augmentation: time stretch, noise, shift, freq mask)
speeko cnn-train data/train_wavs --epochs 50

# Override batch size or learning rate
speeko cnn-train data/train_wavs --epochs 100 --batch-size 32 --lr 0.0005
```

Switch recognition to CNN mode by editing `config/speeko.toml`:

```toml
[recognizer]
mode = "cnn"    # "dtw" (default) or "cnn"
```

Then `test`, `test-from`, and `evaluate` will use the CNN automatically.

### Other Commands

```bash
speeko list-words               # Show trained words and sample counts
speeko list-words --detailed    # Show frame counts per word
speeko evaluate                 # Batch evaluate against stored recordings
speeko record -o test.wav       # Record audio to a WAV file
speeko record --trim            # Record and trim silence
speeko record-samples start stop --samples 5 -o data/train_wavs  # Batch record
speeko train-from data/train_wavs --reset  # Train from pre-recorded WAVs
speeko test-from data/test_wavs --verbose  # Test from pre-recorded WAVs
speeko cnn-train data/train_wavs --epochs 50  # Train CNN classifier
speeko extract test.wav         # Show feature dimensions
speeko extract test.wav --dump  # Dump full feature matrix
speeko diagnose                 # Print audio devices, config, template info
speeko reset start              # Delete templates for a word
speeko reset --all              # Delete all templates
```

### Verbosity

```bash
speeko -v test     # INFO level
speeko -vv test    # DEBUG level (includes timing)
speeko -vvv test   # TRACE level (per-frame values)
```

## Configuration

Edit `config/speeko.toml` to tune parameters. See the file for all options.

Key settings:
- `audio.sample_rate` — microphone sample rate (default: 16000)
- `recognizer.mode` — `"dtw"` or `"cnn"` (default: `"dtw"`)
- `recognizer.confidence_threshold` — rejection threshold (default: 0.2)
- `recognizer.max_distance` — absolute DTW distance cutoff (default: 80.0)
- `vad.threshold_factor` — VAD sensitivity (default: 3.5)
- `mfcc.use_cmn` — enable cepstral mean normalization (default: true)
- `mfcc.use_deltas` — enable delta + delta-delta coefficients (default: true)
- `classifier.max_frames` — CNN input length in frames (default: 100)
- `classifier.model_dir` — CNN model save directory (default: `data/models`)
- `classifier.epochs` — CNN training epochs (default: 50)
- `classifier.batch_size` — CNN training batch size (default: 16)
- `classifier.learning_rate` — CNN learning rate (default: 0.001)

## Vocabulary

Edit `vocabulary.txt` (one word per line) to define the set of recognized words. Default:

```
start, stop, open, close, up, down, left, right, yes, no
```

## Architecture

```
speeko/
├── crates/
│   ├── app-cli/      CLI binary (clap)
│   ├── audio/        Mic capture (cpal) + WAV I/O (hound)
│   ├── dsp/          Preprocessing, framing, FFT (rustfft)
│   ├── vad/          Energy-based voice activity detection
│   ├── features/     Mel filterbank, MFCC extraction, delta coefficients
│   ├── recognizer/   DTW algorithm, template matching, confidence scoring
│   ├── classifier/   CNN classifier (burn 0.20): model, dataset, augmentation, training, inference
│   ├── store/        Template persistence (bincode), vocabulary loading
│   └── common/       Shared types, errors, configuration
```

Pipeline (DTW): **Mic → Preprocess → VAD → MFCC + CMN + Δ/ΔΔ → Mean Template DTW Match → Result**

Pipeline (CNN): **Mic → Preprocess → VAD → MFCC + CMN + Δ/ΔΔ → Pad/Truncate → CNN Forward → Softmax → Result**

All DSP math (mel filterbank, DCT, MFCC, DTW) is implemented from scratch in Rust.

## Cross-Compilation

```bash
# ARM64 (Raspberry Pi 4/5)
rustup target add aarch64-unknown-linux-gnu
cargo install cross
cross build --release --target aarch64-unknown-linux-gnu

# ARMv7 (Raspberry Pi 3/Zero 2)
cross build --release --target armv7-unknown-linux-gnueabihf
```

## Design Documents

- [Detailed Design & Implementation Plan](docs/detailed-design-approved.md)

## License

MIT
