# DESIGN.md — Auralis System Architecture

## 1. System Philosophy
Auralis is a headless audio graph compiler and auditory telemetry engine designed for autonomous agents. It treats audio composition as code: a declarative manifest is compiled into an audio buffer, which is analyzed and returned with structured acoustic telemetry.

## 2. Component Topology
```text
[Declarative Manifest (JSON)]
           │
           ▼
     [model.rs] (Song, Tracks, Clips, Notes, FX params)
           │
           ▼
     [graph.rs] (Audio Graph & Offline DSP Render Pipeline)
           │
     ┌─────┴─────────────────────┐
     ▼                           ▼
[synth.rs]                 [telemetry.rs]
(Oscillators, Envelopes)   (FFT, Peak, RMS, Clash Detect)
     │                           │
     ▼                           ▼
[WAV / PCM Output]         [Acoustic Feedback JSON]
```

## 3. Core Invariants
1. **Deterministic Execution**: Same manifest + same parameters = identical PCM samples and identical telemetry metrics.
2. **Zero Audio Driver Requirement**: Compiles offline directly to RAM buffers and files; does not require soundcards, ALSA/PulseAudio/JACK, or real-time audio clocks.
3. **Closed-Loop Auditory Telemetry**: Every compilation produces quantitative telemetry (Peak, RMS, Spectral bands) enabling LLM self-correction.
4. **Separation of Model and DSP**: The manifest model (`model.rs`) has no audio DSP logic or file I/O dependencies.

## 4. Non-Goals
- **Visual GUI / Timeline**: Auralis will never implement a graphical timeline, mixer faders, or skeuomorphic knobs.
- **Real-Time Live DAW**: Not designed for low-latency live stage performance; optimized for ultra-fast offline batch compilation.
- **Proprietary Formats**: No opaque binary project formats; pure JSON/YAML.
