# Speeko

**Offline spoken-word recognition for edge devices.**

Speeko is a small-footprint, offline, isolated-word recognizer written in Rust. It uses MFCC feature extraction and DTW template matching to recognize a fixed vocabulary of spoken command words — no cloud APIs, no neural networks, no ready-made ASR engines.

## Features

- **Offline only** — no network, no cloud, no external APIs
- **Custom DSP** — MFCC extraction and DTW matching implemented from scratch
- **Small footprint** — <10MB binary, <50MB RAM
- **Near real-time** — <500ms inference latency
- **Speaker-dependent** — trained per user for best accuracy
- **Configurable** — vocabulary, thresholds, and DSP parameters via TOML config

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

### Train

Record 3 samples for each word you want to recognize:

```bash
speeko train start --samples 3
speeko train stop --samples 3
speeko train open --samples 3
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

### Other Commands

```bash
speeko list-words          # Show trained words and sample counts
speeko list-words --detailed  # Show frame counts per word
speeko evaluate            # Batch evaluate against stored recordings
speeko record -o test.wav  # Record audio to a WAV file
speeko record --trim       # Record and trim silence
speeko extract test.wav    # Show MFCC feature dimensions
speeko extract test.wav --dump  # Dump full MFCC matrix
speeko diagnose            # Print audio devices, config, template info
speeko reset start         # Delete templates for a word
speeko reset --all         # Delete all templates
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
- `recognizer.confidence_threshold` — rejection threshold (default: 0.3)
- `recognizer.max_distance` — absolute DTW distance cutoff (default: 100.0)
- `vad.threshold_factor` — VAD sensitivity (default: 2.0)

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
│   ├── store/        Template persistence (bincode), vocabulary loading
│   └── common/       Shared types, errors, configuration
```

Pipeline: **Mic → Preprocess → VAD → MFCC → DTW Match → Result**

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
