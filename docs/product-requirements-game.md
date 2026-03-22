# Product Requirements Document  
## Project: Voice Runner  
### Working title: Command Rush

---

## 1. Executive Summary

Voice Runner is a voice-controlled 2D action game written in Rust. The game is designed around a fixed offline command vocabulary rather than keyboard or controller input. The player uses short spoken commands such as `up`, `down`, `left`, `right`, `open`, and `yes` to survive obstacles, interact with the environment, and make choices during gameplay.

The product serves two purposes:

1. a fun, replayable game built around voice as the primary input method  
2. a technical showcase for ultra-low-latency offline speech recognition in Rust

The MVP will be a desktop-first, voice-first endless runner with clear command feedback, calibration tooling, diagnostics, and a modular architecture aligned with Bevy ECS.

---

## 2. Product Vision

Create a polished Rust game that proves voice can be a primary gameplay input when the game is designed around discrete intent rather than continuous control.

The product should demonstrate:

- fast and satisfying voice-driven gameplay
- offline operation with no cloud dependency
- clean modular Rust architecture
- low-latency command-to-action pipeline
- future portability to smaller devices

---

## 3. Problem Statement

Speech control is often treated as a novelty in games because typical speech systems are too slow, too error-prone, too dependent on cloud services, or poorly matched to continuous gameplay.

This project addresses that by using:

- a fixed small vocabulary
- offline recognition
- very low-latency command recognition
- gameplay designed specifically around spoken intent

The goal is not to imitate keyboard controls with speech, but to build a game where speech is the natural and enjoyable input model.

---

## 4. Goals

### Primary Goals
- Build a playable voice-first game MVP in Rust
- Make command response feel immediate and satisfying
- Design gameplay that naturally fits discrete spoken commands
- Keep runtime lightweight and robust
- Build a clean and extensible codebase

### Secondary Goals
- Provide calibration and diagnostics tools
- Support tuning of recognition thresholds and debounce behavior
- Create a strong technical demo for offline speech-driven interaction
- Enable future expansion to other game modes and devices

---

## 5. Non-Goals

The MVP will not include:

- free-form speech recognition
- natural language understanding
- online multiplayer
- cloud APIs
- mobile-first delivery
- 3D gameplay
- heavy physics simulation
- user-trained speech models within the shipping game UI
- large story mode or narrative campaign

---

## 6. Target Users

### Primary Users
- Rust developers
- technical demo audiences
- indie game enthusiasts
- people interested in offline AI or embedded voice systems

### Secondary Users
- casual players
- accessibility-oriented users
- makers and hobbyists exploring speech interfaces

---

## 7. Core Gameplay Concept

The MVP is a 2D voice-controlled endless runner.

The player character automatically moves forward. Obstacles, gates, route choices, and interaction prompts appear. The player must issue short voice commands to survive and maximize score.

### Example Command Mapping

| Spoken Word | Gameplay Action |
|---|---|
| `start` | begin run / resume |
| `stop` | freeze / brace / pause |
| `up` | jump |
| `down` | slide / duck |
| `left` | move to left lane |
| `right` | move to right lane |
| `open` | open gate / activate path |
| `close` | close shield / barrier |
| `yes` | accept option / route |
| `no` | reject option / take alternate |

---

## 8. MVP Scope

### Included
- desktop Rust game
- 2D endless runner gameplay
- 3-lane movement model
- voice-only gameplay actions
- main menu
- calibration screen
- HUD with recognized word feedback
- score and combo system
- simple obstacle set
- contextual interactions using all 10 commands
- diagnostics/debug overlay
- settings for threshold and device selection

### Excluded
- online features
- achievements service
- leaderboard backend
- character unlocks
- procedural art pipeline
- advanced campaign structure
- complex particle-heavy graphics

---

## 9. Functional Requirements

### 9.1 Main Menu
The game shall provide:
- Start Game
- Calibration
- Settings
- Exit

Optional convenience:
- keyboard or mouse input allowed in menu navigation

### 9.2 Calibration Mode
The game shall provide a calibration mode that:
- listens to microphone input
- displays last recognized word
- displays confidence score
- displays accepted vs rejected recognition attempts
- allows testing all supported words
- stores microphone and threshold preferences

### 9.3 Gameplay State
The game shall support:
- starting a run
- pausing/resuming
- restarting after failure
- entering game over state after failure

### 9.4 Player Actions
The game shall support:
- jump
- slide
- left lane shift
- right lane shift
- contextual open
- contextual close
- contextual accept
- contextual reject
- freeze/brace action
- start/resume action

### 9.5 Obstacles and Interactions
The MVP shall include:
- low obstacles requiring jump
- high obstacles requiring slide
- lane hazards requiring left/right
- gates requiring open/close
- choice prompts requiring yes/no

### 9.6 HUD Feedback
The game shall display:
- score
- combo
- distance
- last recognized word
- confidence
- accepted/rejected status

### 9.7 Settings
The game shall support:
- microphone device selection
- confidence threshold tuning
- debounce tuning
- audio volume
- debug overlay enable/disable
- fullscreen/windowed mode

---

## 10. Non-Functional Requirements

### Performance
- target 60 FPS on target desktop machine
- low command-to-action latency
- minimal per-frame allocations in hot paths
- bounded obstacle count
- efficient collision checks

### Reliability
- recognition rejection shall not crash gameplay
- microphone disconnect shall fail gracefully
- invalid commands shall be ignored safely
- recognition diagnostics shall remain observable in debug mode

### Offline Requirement
- gameplay must function fully offline
- no cloud APIs or external speech services

### Maintainability
- modular architecture
- decoupled speech and gameplay layers
- testable command mapping logic
- configurable thresholds and cooldowns

### Portability
- desktop-first implementation
- future path to Linux and low-resource devices

---

## 11. Gameplay Mechanics

### 11.1 Movement Model
Use a 3-lane runner model for the MVP.

Reason:
- discrete lane changes fit voice input naturally
- collision logic stays simple
- gameplay remains readable and fair

### 11.2 Timing Model
The game should use forgiving timing windows:
- jump windows readable by casual users
- lane changes visually smooth
- open/close and yes/no interactions clearly telegraphed

### 11.3 Scoring Model
Scoring should include:
- distance survived
- successful command execution
- combo streak
- perfect timing bonuses
- penalties for invalid or unnecessary repeated commands

### 11.4 Difficulty Progression
Difficulty should increase gradually by:
- increasing run speed
- increasing obstacle density
- introducing mixed command patterns
- adding contextual interactions later in a run

---

## 12. Command Mapping and Interaction Model

### 12.1 Design Principle
Speech should be treated as intent input, not continuous analog input.

### 12.2 Command Rules
- `up` valid when jump is allowed
- `down` valid when slide is allowed
- `left/right` valid only if a lane change is possible
- `open/close` valid only near interactable entities
- `yes/no` valid only when a choice is active
- `stop` may be given priority over most actions
- `start` begins a run or resumes from pause/freeze state

### 12.3 Debounce and Cooldown
The game shall support per-command debounce/cooldown logic to avoid duplicate command spam and false positives.

---

## 13. UX Requirements

The product must make recognition results visible and understandable.

### Required Feedback
- what word was heard
- confidence level
- whether the command was accepted
- what action occurred

### Example HUD Feedback
- `Heard: UP (0.94) -> Jump`
- `Heard: LEFT (0.61) -> Rejected`

### Audio Feedback
- accepted commands should trigger subtle confirmation sound
- rejected commands should have distinct non-annoying feedback

### Accessibility
- recognized-word subtitles on screen
- adjustable volume
- future support for color-safe visual themes

---

## 14. Calibration Mode Requirements

Calibration mode is mandatory for MVP.

### Calibration Screen Shall Show
- live mic activity
- last recognized word
- confidence score
- accepted/rejected reason
- per-word test counts
- threshold value
- selected audio device

### Calibration Shall Allow
- threshold tuning
- debounce tuning
- testing all 10 words
- saving tuned settings

---

## 15. Debug and Diagnostics Requirements

The game shall provide a debug overlay for development and tuning.

### Debug Overlay Should Include
- FPS
- frame time
- last recognized words
- command accept/reject counters
- last rejection reason
- current player state
- current lane
- obstacle count
- end-to-end command latency estimate

### Diagnostics Logging Should Include
- recognizer timing
- command mapping timing
- command rejection reasons
- microphone initialization issues
- state transitions

---

## 16. Performance Requirements

### Targets
- 60 FPS on primary desktop target
- stable command processing under gameplay load
- no blocking work in microphone callback
- avoid dynamic allocation in recognizer hot path where possible

### Key Performance Risks
- excessive audio copying
- heavy UI updates every frame
- too many dynamic spawns/despawns
- collision checks scaling poorly
- recognizer work on the gameplay thread without proper boundaries

---

## 17. Reliability Requirements

The game should degrade gracefully.

### Required Behaviors
- low-confidence words are rejected safely
- invalid commands do not corrupt state
- missing microphone presents actionable error
- gameplay continues even when a command is rejected
- config load failure falls back to defaults where possible

---

## 18. Portability Requirements

### MVP Platform
- Windows desktop

### Near-Term Expansion
- Linux desktop

### Long-Term Direction
- smaller devices / embedded-style deployment
- low-resource systems with offline speech support

The architecture should avoid unnecessary platform lock-in.

---

## 19. Architecture Expectations

The product should align with a modular Bevy ECS architecture.

### Required Conceptual Separation
- microphone capture
- speech recognition
- recognized-word stream
- command mapping / debounce / cooldown
- gameplay mutation
- UI feedback
- diagnostics

### Design Goals
- deterministic command-before-simulation flow
- plugin-based modularity
- state-driven gameplay structure
- clear separation between real-time audio capture and ECS world mutation

---

## 20. Suggested Module Breakdown

```text
src/
  main.rs
  app.rs
  config.rs
  common/
  core/
  audio_input/
  speech/
  commands/
  game/
  ui/
  effects/
  diagnostics/
  debug/
```

### Module Responsibilities

#### `audio_input`
- microphone enumeration
- microphone stream setup
- PCM capture
- ring buffer handoff

#### `speech`
- feature extraction
- word classification
- confidence scoring
- recognition metrics

#### `commands`
- recognized word to gameplay command mapping
- debounce
- cooldown
- arbitration
- invalid command rejection

#### `game`
- player state machine
- movement
- lane logic
- obstacle spawning
- interactions
- collision
- score
- difficulty

#### `ui`
- menu
- HUD
- calibration screen
- subtitles
- debug overlays

#### `effects`
- sound effects
- music
- visual feedback
- screen shake or polish effects later

#### `diagnostics`
- tracing
- performance counters
- latency recording

---

## 21. Suggested Rust Libraries

### 21.1 Game Engine
**Recommended:** `bevy`

Why:
- Rust-native
- strong ECS model
- plugin-based architecture
- good fit for modular gameplay and UI

Use for:
- game loop
- rendering
- ECS
- states
- UI
- resource management

### Alternative
`macroquad`

Tradeoff:
- easier and lighter for prototypes
- less structured for long-term ECS-heavy architecture

---

### 21.2 Microphone Input
**Recommended:** `cpal`

Why:
- low-level cross-platform audio I/O
- good control over input devices and streams
- appropriate base for a custom recognizer

Use for:
- microphone capture
- device selection
- PCM stream input

### Alternative
Higher-level wrappers or custom platform APIs

Tradeoff:
- more convenience possible
- less control or worse portability in some cases

---

### 21.3 Sound Effects / Music
**Recommended:** `kira`

Why:
- game-oriented audio playback
- useful control for music, SFX, and transitions

### Optional Integration
`bevy_kira_audio`

Why:
- more convenient Bevy integration for Kira-based audio handling

### Alternative
`rodio`

Tradeoff:
- simpler for experiments
- less game-focused control for a more polished product

---

### 21.4 Configuration
**Recommended:** `serde` + `toml`

Why:
- standard Rust serialization approach
- easy-to-read config files
- great for tuning thresholds and gameplay parameters

Use for:
- microphone settings
- threshold tuning
- gameplay constants
- debug options

### Alternative
`serde` + `json`

Tradeoff:
- JSON is common and machine-friendly
- TOML is nicer for hand-editing configs

---

### 21.5 Logging / Diagnostics
**Recommended:** `tracing`

Why:
- structured diagnostics
- useful for timing spans and debugging
- better fit than plain println-style logging

Use for:
- recognizer timing
- command pipeline timing
- config loading diagnostics
- state transitions

### Alternative
`log` + logger backend

Tradeoff:
- simpler ecosystem
- less powerful for structured timing and spans

---

### 21.6 Error Handling
**Recommended:** `thiserror`

Why:
- clean error enums
- low boilerplate
- good internal error structure

### Optional
`anyhow` for app-level startup or top-level command handling

Tradeoff:
- `anyhow` is convenient at boundaries
- `thiserror` is better for domain-specific typed errors

---

### 21.7 CLI / Tooling
**Recommended:** `clap`

Why:
- robust command-line parsing
- useful for calibration-only, replay, or benchmark modes

Use for:
- `--calibration`
- `--benchmark`
- `--config`
- `--debug`

---

### 21.8 Benchmarking
**Recommended:** `criterion`

Why:
- useful for microbenchmarks of recognizer components
- good for measuring command mapping and feature extraction cost

Use for:
- recognizer latency benchmarks
- feature extraction benchmarks
- queue handoff benchmarks

---

### 21.9 Concurrency / Queueing
**Recommended:** standard library channels for simple cases, or a dedicated queue crate if needed

Suggested direction:
- keep microphone callback minimal
- write to queue/ring buffer
- drain from gameplay-side system

Potential options:
- lock-free queue crate
- crossbeam channels where appropriate

Tradeoff:
- keep it simple first
- only introduce more complex lock-free primitives if actual profiling shows a need

---

## 22. Library Selection Summary

| Concern | Recommended | Alternative | Notes |
|---|---|---|---|
| Engine | Bevy | macroquad | Bevy preferred for long-term modular architecture |
| Mic Input | cpal | platform-specific APIs | CPAL is the right portable base |
| Game Audio | kira / bevy_kira_audio | rodio | Kira is more game-oriented |
| Config | serde + toml | serde + json | TOML better for hand tuning |
| Logging | tracing | log | tracing better for performance instrumentation |
| Errors | thiserror | anyhow | use anyhow only at boundaries if needed |
| CLI | clap | manual args | clap is the practical choice |
| Benchmarking | criterion | custom timers | criterion preferred for repeatable microbenchmarks |

---

## 23. Testing Strategy

### Unit Tests
- command mapping
- debounce logic
- cooldown logic
- player state transitions
- score calculations
- config load/save

### Integration Tests
- recognizer output to accepted command flow
- state transitions between menu/calibration/playing/game over
- obstacle interaction success/failure
- gameplay reactions to valid and invalid commands

### Performance Tests
- recognizer frame throughput
- command mapping latency
- queue/ring buffer throughput
- frame-time stability under load

### Manual Test Scenarios
- noisy environment
- repeated fast commands
- low-confidence spoken commands
- microphone device switching
- long-running session stability

---

## 24. Risks and Mitigations

### Risk: False Positives
Mitigation:
- confidence threshold
- debounce
- cooldown
- state-aware validation
- calibration tooling

### Risk: Speech Fatigue
Mitigation:
- keep sessions focused and fun
- avoid requiring constant speech spam
- design around meaningful commands only

### Risk: Noisy Environments
Mitigation:
- calibration mode
- threshold tuning
- device selection
- optional frontend noise gating later

### Risk: Overengineering the First Version
Mitigation:
- keep MVP 2D
- keep collision simple
- avoid heavy physics
- focus on responsiveness and clarity first

### Risk: Engine/Dependency Churn
Mitigation:
- pin versions for MVP
- isolate engine-specific code in plugins/modules
- keep speech core independent of gameplay details

---

## 25. Milestones / Roadmap

### Phase 1: Technical Skeleton
- project setup
- Bevy app states
- menu and calibration screens
- microphone capture integration
- recognizer integration
- command overlay

### Phase 2: Gameplay MVP
- 3-lane runner
- jump / slide / left / right
- start / stop behavior
- basic obstacle spawning
- collision and game over
- score and HUD

### Phase 3: Full Command Integration
- open / close mechanics
- yes / no choice events
- combo system
- feedback sounds
- diagnostics overlay

### Phase 4: Product Polish
- settings persistence
- improved art/audio
- balancing
- stability testing
- packaging for release/demo

---

## 26. Definition of MVP Done

The MVP is considered done when:

- the game runs on desktop in Rust
- microphone input is functional
- the offline recognizer controls gameplay
- all 10 commands are used meaningfully
- the player can start, play, fail, and restart a run
- calibration mode is available
- diagnostics are available for tuning
- gameplay is understandable and fun enough for repeat runs
- command feedback is visible and trustworthy

---

## 27. Future Enhancements

### Gameplay
- boss encounters driven by command sequences
- rhythm-command hybrid mode
- challenge levels
- endless daily challenge
- replay ghost system

### Technical
- replay recording
- deterministic simulation mode
- benchmark harness
- input session recording for offline tuning
- packaging for embedded or small-device targets

### UX
- better visual polish
- richer animations
- accessibility modes
- alternate themes
- tutorial mode

---

## 28. Final Recommendation

Build the MVP with:

- `bevy`
- `cpal`
- `kira` or `bevy_kira_audio`
- `serde`
- `toml`
- `tracing`
- `thiserror`
- `clap`
- `criterion`

Keep the first version focused on:
- responsiveness
- clarity
- calibration
- meaningful gameplay mapping for all 10 commands

Do not try to make speech behave like a keyboard.  
Design the game around voice as intent.

That is the product advantage.
