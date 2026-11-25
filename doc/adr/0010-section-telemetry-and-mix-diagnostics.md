# ADR-0010: Section-Level Telemetry, Crest Factor & Automated Mix Diagnostics

## Context and Problem Statement
Whole-song RMS, peak, and spectral ratios in `AudioTelemetry` hid localized balance problems such as quiet-intro-to-loud-hook jumps, over-compressed squashed transients, phase-cancelled stereo channels, or sub-bass masking.

## Considered Options
1. Break `analyze_audio` signature to return a separate report struct.
2. Enrich `AudioTelemetry` with additive `#[serde(default)]` fields (`sections`, `crest_factor_db`, `side_to_mid_ratio`, `diagnostics`) while preserving the 4-argument `analyze_audio` contract.

## Decision Outcome
Chosen option: **Additive `AudioTelemetry` enrichment (Option 2)**.
- **Per-Section Metrics (`SectionTelemetry`)**: Tracks `>= 8s` divide into 8 equal windows (`>= 1s` into 4 windows), each reporting `peak_dbfs`, `rms_dbfs`, `crest_factor_db`, `is_clipping`, and Hann-windowed `spectral_bands`.
- **Stereo & Dynamics**: Added global `crest_factor_db` and Mid/Side `side_to_mid_ratio`.
- **Automated Diagnostics (`diagnostics: Vec<String>`)**: Emits deterministic flags (`CLIPPING_DETECTED`, `SUB_BASS_DOMINANT`, `DULL_HIGH_END`, `LOW_DYNAMIC_RANGE`, `PHASE_CANCELLATION_RISK`, `HIGH_SECTION_VARIANCE`) for closed-loop agent self-correction.

### Positive Consequences
- Zero breaking changes across `main.rs`, `mcp.rs`, or existing manifest consumers.
- Agents automatically receive section-level loudness variance and spectral mud flags on every `compile` and `eval`.

### Negative Consequences
- Slightly larger telemetry JSON output when sections are populated.

## Reversal / Kill Criteria
All new fields use `#[serde(default)]` and `skip_serializing_if`; reversible without impacting existing callers.
