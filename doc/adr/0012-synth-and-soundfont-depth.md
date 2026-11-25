# ADR-0012: Synth Expressiveness (Unison, LFO, FM) & SoundFont MIDI Depth

## Context and Problem Statement
Auralis's native `Synth` renderer produced single-oscillator voices without stereo chorus/unison detuning, pitch vibrato LFO, or FM harmonics, while `SoundFont` rendering ignored `slide_to_midi` pitch glides and damper pedal `CC64` sustain.

## Considered Options
1. Introduce separate instrument variants (`FmSynth`, `UnisonSynth`) with breaking schema duplication.
2. Extend `InstrumentConfig::Synth` with optional `unison`, `lfo`, and `fm` configs, and extend `NoteEvent` + `soundfont.rs` with `sustain` (`CC64`) and RPN-calibrated ±12-semitone Pitch Bend (`0xE0`).

## Decision Outcome
Chosen option: **Additive `Synth` modulation + SoundFont MIDI control (Option 2)**.
- **Unison Detune (`UnisonConfig`)**: Renders `1..=7` voices spread across `±detune_cents/2` with deterministic initial phase dispersion and `1/sqrt(voices)` normalization.
- **Vibrato LFO (`LfoConfig`) & 2-Op FM (`FmConfig`)**: Modulates pitch in semitones via sinusoidal LFO and modulates carrier phase (`carrier + mod_index * sin(mod_phase)`) wrapped via `rem_euclid(2*PI)`.
- **SoundFont Expressiveness (`src/soundfont.rs`)**: Configures GM Pitch Bend Sensitivity RPN (`±12` semitones), steps `0xE0` pitch-bend events across `glide_beats`, and emits `CC64` damper sustain events.

### Positive Consequences
- Rich supersaw leads, FM electric pianos/bells, vibrato lo-fi textures, and gliding/sustained SoundFont instruments in pure Rust.
- Zero new `Cargo.toml` dependencies.

### Negative Consequences
- Multi-voice unison scales synthesis CPU linearly with `voices` (capped at `7`).

## Reversal / Kill Criteria
All new fields use `#[serde(default)]`; existing JSON manifests remain bit-compatible.
