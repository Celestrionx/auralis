# ADR-0016: v0.2.0 Release & `DULL_HIGH_END` Telemetry Threshold Tuning

## Context and Problem Statement
In `src/telemetry.rs`, `calculate_spectral_bands` computes `high_ratio` as the linear sum of FFT bin magnitudes above `4000 Hz` divided by total spectrum magnitude across up to 32 Hann windows. Warm or dark acoustic compositions (e.g., piano, mellow synth pads, cinematic ambient mixes) naturally measure `high_ratio` around `0.015`–`0.03`, which tripped the original `high_ratio < 0.05` `DULL_HIGH_END` diagnostic flag as a false positive. Simultaneously, Milestones P1–P6 plus `ADR-0013`–`ADR-0015` completed the `v0.2.0` feature set requiring updated crate versioning and documentation.

## Considered Options
1. Convert `calculate_spectral_bands` from magnitude ratios to log-power decibel bands and rewrite all existing spectral thresholds.
2. Lower the `DULL_HIGH_END` diagnostic trigger in `build_diagnostics` from `high_ratio < 0.05` to `high_ratio < 0.01` and release `v0.2.0`.

## Decision Outcome
Chosen option: **Lower `DULL_HIGH_END` threshold to `high_ratio < 0.01` and release `v0.2.0` (Option 2)**.
- **Telemetry (`src/telemetry.rs`)**: `DULL_HIGH_END` now fires only when `rms_dbfs > -40.0 && spectral_bands.high_ratio < 0.01`, eliminating false warnings on warm acoustic/ambient mixes while continuing to catch genuinely treble-starved or pure sub-bass renders.
- **Release & Docs (`Cargo.toml`, `README.md`, `AGENTS.md`)**: Bumped crate version to `0.2.0` and synchronized documentation with all 9 `src/` modules, `InstrumentConfig::Sample`, dynamic filters, game export flags, and MCP tools.

### Positive Consequences
- Preserves calibrated `sub_bass_ratio` / `mid_ratio` / `high_ratio` schema semantics for existing automated composition loops while improving signal-to-noise ratio of `diagnostics`.
- Documentation accurately reflects the complete `v0.2.0` architecture (`ADR-0001`–`ADR-0016`).

### Negative Consequences
- Mixes that intentionally sit between `0.01` and `0.05` `high_ratio` must inspect numeric `spectral_bands.high_ratio` directly if a brighter pop/EDM top-end target is desired.

## Reversal / Kill Criteria
Restore `< 0.05` or introduce genre-specific diagnostic profiles inside `build_diagnostics` in `src/telemetry.rs`.
