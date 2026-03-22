# Voice Commando: Command Rush

A voice-controlled endless runner game built with [Bevy](https://bevyengine.org/) and the [Speeko](../README.md) offline speech recognition engine. Dodge obstacles by speaking commands into your microphone — no keyboard required during gameplay.

## How It Works

Your voice is captured in real-time, processed through a VAD → MFCC → CNN pipeline, and converted into game commands. The entire recognition pipeline runs **offline** — no internet connection or cloud APIs needed.

```
Microphone → Ring Buffer → VAD → Pre-emphasis → MFCC → CMN + Deltas → CNN → Game Command
```

## Requirements

- **Rust 1.75+**
- **A working microphone**
- **Trained Speeko CNN model** in `data/models/` (see [Training](#training-the-model))
- **OS**: Windows (WASAPI), Linux (`libasound2-dev`), macOS (CoreAudio)

## Quick Start

```bash
# From the voice-commando directory
cargo run
```

The game window opens to the **Main Menu**. Say **"start"** or press **Enter** to begin.

## Voice Commands

During gameplay, speak these words clearly into your microphone:

| Word | Action | Use |
|---------|----------------|---------------------------------------------|
| `start` | Start / Resume | Begin the game from menu or game-over screen |
| `up` | Jump | Leap over **red** low barriers |
| `down` | Slide | Duck under **blue** high barriers |
| `left` | Move Left | Change to the lane on your left |
| `right` | Move Right | Change to the lane on your right |
| `stop` | Freeze | Briefly freeze in place (grants invincibility) |

> **Tip:** Speak each word clearly with a brief pause before and after. The recognizer needs ~1.5 seconds of audio context to detect speech reliably.

## Gameplay

### Objective

Survive as long as possible by dodging obstacles that scroll toward you. Score points by successfully executing voice commands and building combos.

### The Playing Field

```
        Left     Center    Right
        lane      lane      lane
          |        |        |
          |  [OBS] |        |   ← Obstacles spawn at the top
          |        |        |
          |        |        |
          |        | [YOU]  |   ← You start in the center lane
          |        |        |
```

- **3 lanes**: Left, Center, Right. You start in the **center lane**.
- **Obstacles** spawn at the top and scroll downward toward you.
- You have a **3-second invincibility** grace period at game start.

### Obstacle Types

| Obstacle | Color | How to Dodge |
|----------------|------------|-------------------------------|
| Low Barrier | **Red** | Say **"up"** to jump over it |
| High Barrier | **Blue** | Say **"down"** to slide under |
| Lane Blocker | **Yellow** | Say **"left"** or **"right"** to switch lanes |

### Scoring

- **+10 points** per successful command
- **Combo multiplier**: every 5 consecutive successful commands increases the multiplier
- **Distance**: accumulates based on scroll speed
- Final score shows: **Points**, **Distance**, and **Max Combo**

### Difficulty Progression

The game starts slow and gradually increases over ~3 minutes:

- **Scroll speed**: starts at 0.4x, ramps to 2.0x max
- **Obstacle rate**: starts at 1 every ~7 seconds, ramps to ~2.5 per second
- Plenty of reaction time at the start for voice recognition latency

### Game Over

When you collide with an obstacle you didn't dodge:
- Your score summary is displayed
- Say **"start"** or press **Enter** to play again
- Press **Escape** to return to the main menu

## Keyboard Controls

While the game is designed for voice control, keyboard shortcuts are available for navigation:

| Key | Action |
|---------|-------------------------------|
| Enter | Start game / Restart |
| Escape | Return to main menu / Quit |
| C | Open calibration (from menu) |

## Configuration

All settings are in [`config/voice_commando.toml`](config/voice_commando.toml):

### Game Speed

```toml
[game]
initial_scroll_speed = 0.4   # Starting scroll speed (pixels/s × 100)
max_scroll_speed = 2.0       # Maximum scroll speed
```

### Voice Recognition Tuning

```toml
[vad]
threshold_factor = 5.0       # VAD sensitivity (higher = less sensitive)
min_utterance_ms = 200       # Minimum speech duration to accept
max_utterance_ms = 1200      # Maximum speech duration to capture

[recognition]
confidence_threshold = 0.35  # Minimum CNN confidence to accept a word

[game.ring_buffer]
recognizer_poll_ms = 30      # How often the recognizer checks for speech
post_recognition_cooldown_ms = 500  # Cooldown between recognitions
```

### Display

```toml
[game.display]
fullscreen = false
resolution = [1280, 720]
```

## Training the Model

The game requires a trained CNN model. Use the main Speeko CLI to train:

```bash
# From the speeko root directory

# 1. Record training samples for all 10 words
speeko record-samples up down left right start stop open close yes no \
    --samples 10 -o data/train_wavs

# 2. Train the CNN model
speeko cnn-train

# Model is saved to data/models/ (shared with voice-commando)
```

The 10 recognized vocabulary words are: `up`, `down`, `left`, `right`, `start`, `stop`, `open`, `close`, `yes`, `no`.

## Testing

Run the integration tests to verify the full recognition pipeline:

```bash
# From the voice-commando directory

# Full pipeline test on all 100 test WAV files (10 words × 10 samples)
cargo test --test ring_buffer_integration_test -- --nocapture
```

This runs two tests:
1. **Simulated pipeline** — feeds WAV files directly through the processing pipeline (target: 100% accuracy)
2. **Ring buffer integration** — pushes WAV data through the actual ring buffer at real-time rate, replicating live gameplay conditions

## Project Structure

```
voice-commando/
├── config/
│   └── voice_commando.toml    # All game and recognition settings
├── crates/
│   ├── audio/                 # Microphone capture, WAV I/O
│   ├── classifier/            # CNN inference (burn backend)
│   ├── common/                # Shared types and config
│   ├── dsp/                   # Pre-emphasis, preprocessing
│   ├── features/              # MFCC extraction, CMN, deltas
│   └── vad/                   # Voice Activity Detection
├── src/
│   ├── audio_bridge/          # Ring buffer + recognizer thread
│   ├── commands/              # Word → GameCommand mapping, debounce
│   ├── game/                  # Player, obstacles, lanes, scoring
│   ├── speech/                # Speech events (recognized/rejected)
│   └── ui/                    # Menu, game-over, HUD
├── tests/
│   └── ring_buffer_integration_test.rs
└── Cargo.toml
```

## Tips for Best Recognition

1. **Quiet environment** — background noise degrades accuracy
2. **Consistent distance** — stay ~30cm from the mic
3. **Clear pronunciation** — short, crisp words work best
4. **Pause between commands** — allow ~1 second between words
5. **Train with your voice** — the CNN model is speaker-dependent; retrain if accuracy is low
