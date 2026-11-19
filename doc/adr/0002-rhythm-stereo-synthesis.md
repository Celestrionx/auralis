# ADR-0002: Rhythm Synthesis, Polyphony, and Stereo Panning Architecture

## Context and Problem Statement
To function as an effective BGM and game soundtrack composer, Auralis required rhythm generation (percussion), polyphonic chord voicings, and spatial stereo field control without sacrificing deterministic execution.

## Considered Options
1. External sample library loading (`.sf2`, wavetable PCM)
2. Algorithmic procedural percussion (deterministic noise + pitch-drop envelopes) with constant-power stereo panning
3. Hardcoded MIDI channel maps

## Decision Outcome
Chosen option: **Algorithmic procedural percussion & stereo panning (Option 2)**.
- **Waveform::Noise**: Deterministic 32-bit linear congruential generator ensuring bit-for-bit reproducible white noise bursts for snares and hi-hats.
- **PitchEnvelope**: Dynamic exponential pitch modulation (`drop_semitones`, `decay_sec`) transforming sine oscillators into punchy acoustic/808 kicks.
- **Polyphony via Chord Notes**: `NoteEvent::chord_notes` allows multi-note chords with velocity normalization ($v / \sqrt{N}$) to prevent sum-clipping.
- **Constant-Power Stereo Panning**: Implemented $\cos(\theta)/\sin(\theta)$ pan law (`FxConfig::Pan`) and interleaved 2-channel WAV export.
- **Stereo Auditory Telemetry**: Added cross-channel correlation metric ($\rho_{LR}$) to detect phase cancellation and quantify stereo width.

### Positive Consequences
- Zero external sample asset dependencies.
- True closed-loop self-correction demonstrated on `examples/celestial_ambient_bgm.json` (calibrating mix from clipping +2.99 dBFS down to pristine -0.12 dBFS).

### Negative Consequences
- Highly realistic acoustic drums (cymbals, brushes) will eventually benefit from wavetable / SoundFont synthesis.

## Reversal / Kill Criteria
If complex realistic acoustic instrumentation becomes the primary design target, reverse to SoundFont 2 (`.sf2`) integration via `rustysynth`.
