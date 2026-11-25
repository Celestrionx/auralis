# ADR-0013: Stereo Unison Voice Spread

## Context and Problem Statement
Multi-genre showcase telemetry (`showcase_edm_future_bass.wav`, `showcase_synthwave.wav`) showed that 5–7 voice `UnisonConfig` supersaws still summed into a mono buffer before `StereoWidth`, yielding high stereo correlation (`0.969` on EDM) and minimal side energy.

## Considered Options
1. Rely solely on downstream `Reverb` / `Delay` for stereo width on `Synth` tracks.
2. Pan unison voices directly in `render_synth_track` via `UnisonConfig::stereo_spread` (`#[serde(default)]` = `0.85`).

## Decision Outcome
Chosen option: **Per-voice constant-power stereo unison spread (Option 2)**.
- `UnisonConfig` gains `stereo_spread: f32` (default `0.85`, clamped `[0.0, 1.0]`).
- `render_synth_track` outputs stereo `(left_buf, right_buf)` directly, panning multi-voice oscillators symmetrically across `[-stereo_spread, +stereo_spread]`.
- Single-voice (`voices == 1`) tracks retain `pan = 0.0` (`FRAC_1_SQRT_2` L/R), keeping legacy renders 100% bit-identical.

### Positive Consequences
- Multi-voice supersaws render with true stereo width out of the box and respond directly to `FxConfig::StereoWidth`.

### Negative Consequences
- Two `Vec<f32>` buffers allocated per `Synth` track instead of one mono buffer.

## Reversal / Kill Criteria
Setting `stereo_spread: 0.0` reverts any unison track to mono summing.
