# ADR-0001: Foundational Architecture for Auralis

## Context and Problem Statement
Autonomous AI agents lack native auditory perception and cannot operate visual GUI DAWs in real-time. Direct screenshot/macro manipulation is fragile, slow, and lacks closed-loop sensory feedback. We need an agent-accessible audio engine that allows AI agents to compose, arrange, render, and evaluate multi-track audio deterministically.

## Considered Options
1. Python prototype (`pedalboard` + `librosa` + `pyloudnorm`)
2. Pure Rust audio compiler (`hound` + `rustfft` + custom DSP graph)
3. Headless wrapper over existing GUI DAWs (REAPER ReaScript / Ableton OSC)

## Decision Outcome
Chosen option: **Pure Rust audio compiler (Option 2)**.
- **Declarative Schema**: Songs and stems defined as inspectable, diffable JSON AST (`model.rs`).
- **Deterministic DSP Graph**: Offline buffer rendering without audio device dependencies (`graph.rs`).
- **Auditory Telemetry**: Integrated spectral balance (FFT) and loudness metrics (`telemetry.rs`) providing closed-loop evaluation for agents.
- **Agent Control Plane**: Headless CLI and Model Context Protocol (MCP) server interface (`mcp.rs`).

### Positive Consequences
- Sub-second offline multi-track rendering (100x+ real-time).
- Zero external runtime or audio driver requirements.
- Native MCP compatibility across agent environments.

### Negative Consequences
- Custom DSP effects and synthesis algorithms must be maintained in Rust.

## Reversal / Kill Criteria
If pure Rust DSP maintenance creates excessive complexity compared to standard audio plugin hosting, reverse to embedding a headless VST3/CLAP host runtime via `nih-plug`.
