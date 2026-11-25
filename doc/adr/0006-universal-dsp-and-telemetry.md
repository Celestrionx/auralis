# ADR-0006: Universal Multi-Genre DSP Graph & Multi-Window Telemetry

## Context and Problem Statement
Earlier compositions (`ADR-0002` through `ADR-0005`) exposed four structural gaps in Auralis's DSP and telemetry layers across genres (electronic, hip-hop/trap, orchestral, rock, jazz, ambient):
1. `render_synth_track` computed `phase = 2 * PI * freq * t`, introducing phase-jump artifacts under pitch modulation and blocking smooth portamento slides.
2. `FxConfig` lacked dynamics/clipping protection (`Saturate`, `SoftClip`), spatial effects (`Delay`, `Reverb`), resonant biquad filtering (`q`), and an opt-in `master_fx` bus.
3. `calculate_spectral_bands` sampled only the first `2048` samples (~46ms), making full-track spectral telemetry blind to everything after a quiet intro.

## Considered Options
1. Genre-specific preset synthesizer modules (e.g., dedicated 808/EDM/rock engines).
2. General-purpose, composable DSP primitives and opt-in `master_fx` applicable across all genres.

## Decision Outcome
Chosen option: **General-purpose composable DSP primitives (Option 2)**.
- **Continuous Phase Accumulation & Portamento**: `NoteEvent` adds optional `slide_to_midi` and `glide_beats`; `render_synth_track` advances phase incrementally (`phase += 2 * PI * freq / sample_rate`).
- **Unified Stereo FX Pipeline**: `FxConfig` extends with optional resonant `q` (RBJ 2nd-order biquad) on `LowPass`/`HighPass`, `Saturate { drive }`, `SoftClip` (`tanh`), tempo-synced stereo ping-pong `Delay`, and deterministic Schroeder stereo `Reverb` (4 parallel damped combs + 2 series allpasses per channel).
- **Opt-In Master Bus (`Song::master_fx`)**: Defaults to empty so legacy mixes remain bit-identical while enabling master limiting/glue for any genre.
- **Multi-Window FFT Telemetry**: `calculate_spectral_bands` Hann-windows and accumulates up to 32 evenly spaced 2048-sample windows across the entire audio buffer.

### Positive Consequences
- Zero external crate additions; deterministic offline rendering preserved.
- Expressive slides, resonant sweeps, analog warmth, spatial depth, and safe peak control available to every genre.

### Negative Consequences
- Slightly higher CPU cost on tracks enabling Schroeder reverb or multi-window FFT (still sub-second on typical stems).

## Reversal / Kill Criteria
All new schema fields use `#[serde(default)]`; reversible without breaking any prior manifest format.
