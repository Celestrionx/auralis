# ADR-0009: Sidechain Ducking, Envelope Compression, Mid/Side Width & Parallel Graph

## Context and Problem Statement
Auralis compiled every track serially on one thread (`graph.rs`) and lacked dynamic gain reduction (`Sidechain`, `Compressor`) and Mid/Side stereo control (`StereoWidth`), limiting punch and mix separation in dense multi-stem arrangements.

## Considered Options
1. Add `rayon` to `Cargo.toml` for parallel track rendering.
2. Use zero-dependency `std::thread::scope` in a deterministic two-pass audio graph with pure-Rust envelope dynamics and Mid/Side stereo width.

## Decision Outcome
Chosen option: **Two-pass `std::thread::scope` parallel graph + pure-Rust dynamics (Option 2)**.
- **Schema (`src/model.rs`)**: Added `FxConfig::Sidechain { source_track, amount, threshold_db, attack_sec, release_sec }`, `FxConfig::Compressor { threshold_db, ratio, attack_sec, release_sec, makeup_db }`, and `FxConfig::StereoWidth { width }`.
- **Two-Pass Parallel Graph (`src/graph.rs`)**:
  - *Pass 1*: All `Synth` and `SoundFont` stems render concurrently inside `std::thread::scope`.
  - *Sidechain Detector Snapshot*: Mono peak-absolute detector signals are extracted once for referenced `source_track` IDs, eliminating circular feedback deadlocks.
  - *Pass 2*: Track FX chains execute concurrently, then sum deterministically in track index order prior to `master_fx`.

### Positive Consequences
- Zero new external dependencies in `Cargo.toml`.
- Bit-identical deterministic output preserved across parallel executions.

### Negative Consequences
- Sidechain triggers reflect raw Pass 1 synthesis levels rather than post-FX levels of the trigger track.

## Reversal / Kill Criteria
If `Sidechain` ever needs post-FX triggers, topologically sort track FX dependencies in Pass 2.
