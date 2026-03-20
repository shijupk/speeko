You are a senior embedded/edge AI architect and Rust systems engineer.

I want you to help me design and implement a **small-footprint offline spoken-word recognition application in Rust**.

This is **not** general speech-to-text. The application only needs to recognize a **small fixed vocabulary of at least 10 words** spoken by a user. It must run on a **small device / edge device** with **no external APIs**, **no cloud dependency**, and **no readily available speech recognition libraries**. The goal is a **working low-footprint solution** with **near real-time response**.

You must create a **product requirements + architecture + implementation plan + starter code structure** for this system.

## Core Goal
Build an offline Rust application that:
1. Captures audio from a microphone
2. Supports **training/enrollment** of at least 10 spoken words
3. Supports **testing/inference** from a microphone
4. Recognizes one of the trained words in near real time
5. Works with a **small memory and CPU footprint**
6. Uses **custom DSP / feature extraction / classification logic**
7. Does **not** use external speech recognition APIs or ready-made speech recognition libraries

## Hard Constraints
- Preferred language: **Rust**
- Runtime target: **small device / embedded-ish Linux device / low-resource edge device**
- Must be **offline only**
- Must support **microphone-based training**
- Must support **microphone-based testing**
- Must avoid:
  - cloud APIs
  - online inference
  - Whisper-like engines
  - Vosk / PocketSphinx / DeepSpeech / Kaldi / Coqui / wav2vec / ready-made ASR engines
  - “just use an existing speech recognizer” solutions
- Keep dependencies minimal
- Design for low latency and low memory use
- Should be practical and buildable, not academic-only

## Functional Scope
The first version should recognize at least **10 isolated spoken command words**, for example:
- start
- stop
- open
- close
- up
- down
- left
- right
- yes
- no

The actual word list should be configurable.

The system should support:
- recording multiple training samples per word from microphone
- saving training samples locally
- extracting features locally
- training a lightweight model locally or building reference templates locally
- real-time microphone input for inference
- returning:
  - predicted word
  - confidence score
  - rejected/unknown result when confidence is too low

## Non-Functional Requirements
- Low CPU usage
- Low RAM usage
- Low binary size where practical
- Fast startup
- Near real-time inference
- Robust enough for a small demo / prototype
- Modular architecture so the recognition engine can be improved later
- Clear separation between audio capture, preprocessing, feature extraction, model/template matching, and UI/CLI

## Important Technical Direction
Since I do not want ready-made speech-recognition libraries, design a **custom small-vocabulary recognizer**.

You should evaluate and recommend one practical approach for v1, such as:
- MFCC or log-mel-like handcrafted features
- endpoint detection / voice activity detection
- DTW-based template matching
- lightweight classifier such as k-NN, centroid matching, or small classical ML approach
- optionally HMM-like logic only if realistic to implement simply

Favor the approach that best balances:
- implementation simplicity
- low footprint
- accuracy for 10-word vocabulary
- near real-time performance
- maintainability in Rust

Do not jump to large neural-network solutions unless you can justify them for very low footprint and self-implemented inference. Simpler is preferred for v1.

## What I want from you
Produce the output in the following sections.

### 1. Product Requirements Document
Write a clear PRD containing:
- problem statement
- goals
- non-goals
- target users / usage scenario
- functional requirements
- non-functional requirements
- constraints
- acceptance criteria
- risks and mitigations

### 2. Recommended Technical Approach
Recommend the best v1 approach for this use case and explain:
- why it fits a 10-word offline recognizer
- tradeoffs vs alternatives
- why it is suitable for small devices
- expected accuracy limitations
- expected latency characteristics

### 3. High-Level Architecture
Provide a modular architecture with components such as:
- microphone capture
- framing/windowing
- preprocessing
- voice activity detection
- feature extraction
- training/enrollment store
- recognizer/classifier
- confidence/rejection logic
- CLI or tiny UI
- persistence layer
- configuration

### 4. Rust Project Structure
Propose a clean Rust workspace/folder structure, for example:
- app-cli
- audio-capture
- dsp-core
- feature-extraction
- vad
- recognizer
- training-store
- config
- common
- benches
- tests

Keep the structure practical and not overengineered.

### 5. Data Flow
Explain step-by-step:
- training flow from microphone input to saved word model/template
- inference flow from microphone input to predicted word
- unknown-word rejection flow

### 6. Algorithm Details
Provide implementation-level detail for:
- audio sampling assumptions
- frame size / overlap
- normalization
- noise handling
- endpoint detection
- feature extraction
- template/classifier creation
- matching logic
- score normalization
- confidence thresholding

### 7. MVP Plan
Define a realistic staged implementation plan:
- Phase 1: microphone recording + WAV saving + simple CLI
- Phase 2: VAD + segmentation
- Phase 3: feature extraction
- Phase 4: template matching recognizer
- Phase 5: training mode
- Phase 6: real-time inference mode
- Phase 7: optimization and footprint reduction

For each phase, define deliverables and success criteria.

### 8. Testing Strategy
Include:
- unit tests
- audio fixture tests
- deterministic feature extraction tests
- recognizer accuracy tests on known samples
- latency tests
- memory/CPU checks
- noisy environment tests
- unknown-word rejection tests

### 9. Performance and Footprint Strategy
Provide specific advice for Rust and small-device optimization:
- allocation minimization
- buffer reuse
- fixed-size or preallocated vectors where possible
- avoiding unnecessary copies
- simple numeric representations
- SIMD only if justified later
- feature caching where appropriate
- compile profile considerations
- binary size considerations

### 10. Risks and Design Tradeoffs
Cover likely issues:
- speaker dependence vs speaker independence
- background noise
- microphone quality
- threshold tuning
- too little training data
- confusing similar words
- latency vs accuracy tradeoff
- portability issues

### 11. Deliverable Files
List the files you would generate in the repo, including:
- README.md
- docs/architecture.md
- docs/prd.md
- Cargo.toml
- crates/...
- sample config files
- sample vocabulary file
- test fixtures plan

### 12. Starter Implementation
After the design, generate a **practical starter Rust codebase** with:
- CLI commands like:
  - `train <word>`
  - `test`
  - `list-words`
  - `evaluate`
- placeholder or initial implementation for:
  - microphone capture
  - WAV persistence
  - simple VAD
  - basic feature extraction
  - simple DTW/template matcher
- code must be clean, modular, and extensible

## Additional Requirements for the Solution
- Prefer isolated-word recognition, not continuous speech recognition
- Prefer speaker-dependent or lightly speaker-adapted design for v1 if it improves feasibility
- Use simple math and DSP that can be implemented from scratch
- If external crates are used, they must be general-purpose utilities/audio I/O crates, not speech recognition engines
- Clearly label which parts are:
  - mandatory for MVP
  - optional improvements
  - future enhancements

## Engineering Expectations
Be very practical.
Do not give a generic textbook answer.
Do not suggest impossible scope for v1.
Do not overengineer.
Do not hide complexity.
Call out assumptions explicitly.
When there are multiple choices, recommend one and explain why.

## Output Format
Return your answer in this order:
1. Executive summary
2. PRD
3. Recommended architecture
4. Algorithm choice and rationale
5. Rust workspace structure
6. Phase-wise implementation plan
7. Testing and benchmarking plan
8. Footprint optimization plan
9. Risks and mitigations
10. Starter repository scaffold
11. Initial Rust code

## Final Design Preference
Unless there is a strong reason otherwise, bias the design toward:
- isolated command-word recognition
- MFCC-like handcrafted features
- DTW-based template matching or another simple classical approach
- low dependency count
- small-device practicality
- near real-time microphone-driven interaction

Also include a brief section titled:
**“Why this approach is better than using a full speech-to-text engine for this use case.”**