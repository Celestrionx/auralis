# ADR-0015: WAV One-Shot & Pitched Sampler Instrument

## Context and Problem Statement
Synthesizers and GM SoundFonts cannot reproduce custom recorded acoustic foley, drum one-shots, or vocal chops. Auralis needed a declarative sample playback instrument compatible with `Pattern`/`Clip`, `FxConfig` chains, and parallel rendering.

## Considered Options
1. Embed raw PCM arrays directly inside JSON manifests.
2. Add `InstrumentConfig::Sample { wav_path, root_midi }` backed by a thread-safe `SAMPLE_CACHE` and linear-interpolated resampling (`src/sampler.rs`).

## Decision Outcome
Chosen option: **External WAV sampler with thread-safe caching & pitch resampling (Option 2)**.
- **Schema (`src/model.rs`)**: `InstrumentConfig::Sample { wav_path, root_midi }` (`root_midi` defaults to `60`).
- **Sampler (`src/sampler.rs`)**: Decodes mono/stereo int or float WAVs via `hound`, caches decoded stereo buffers in `SAMPLE_CACHE`, resamples at `(src_sr / dst_sr) * 2^((midi - root_midi)/12)` (including `slide_to_midi` glides and `chord_notes`), and applies a 10ms anti-click release fade.
- **Resilience**: Missing or corrupt WAV paths emit a warning and render silence cleanly, matching `soundfont.rs` fallback behavior.

### Positive Consequences
- Custom drum one-shots, foley recordings, and pitched vocal/instrument chops integrate seamlessly into Pass 1 parallel rendering and all `FxConfig` effects.
- Zero new `Cargo.toml` dependencies.

### Negative Consequences
- Linear interpolation suffices for drum/foley/pitched samples; extreme >+24st upward shifts may introduce mild aliasing compared to windowed-sinc resampling.

## Reversal / Kill Criteria
Upgrade interpolation inside `render_sample_track` to cubic/sinc without changing any manifest schema.
