# ADR-0014: Dynamic Filter Sweeps, BandPass & Per-Note Filter Envelopes

## Context and Problem Statement
Static `LowPass` and `HighPass` cutoffs prevented percussive synth plucks, acid squelches, build-up sweeps, and resonant bandpass foley shaping (e.g. swept wind/whooshes).

## Considered Options
1. Add separate modulated synth variants.
2. Unify 2nd-order RBJ biquad math (`LowPass`, `HighPass`, `BandPass`), add beat-synced exponential `FxConfig::FilterSweep`, and add per-note `FilterEnvelope` on `InstrumentConfig::Synth`.

## Decision Outcome
Chosen option: **Shared RBJ biquad + exponential block-rate sweeps + per-note `FilterEnvelope` (Option 2)**.
- `FxConfig::BandPass { cutoff_hz, q }` and `FxConfig::FilterSweep { start_hz, end_hz, start_beat, end_beat, q, mode }` operate in both track and `master_fx` chains.
- `FilterEnvelope { start_cutoff_hz, end_cutoff_hz, decay_sec, q }` filters each polyphonic note independently before summing.
- Zero overhead when `filter_env` is `None`.

### Positive Consequences
- Authentic synth plucks, riser builds, and swept foley effects without extra crates.

### Negative Consequences
- `apply_time_varying_biquad` refreshes coefficients every 16–32 samples; very fast sub-millisecond sweeps quantize to block boundaries.

## Reversal / Kill Criteria
All new fields/variants use `#[serde(default)]`; fully additive.
