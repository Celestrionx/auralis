# ADR-0007: Song-Level Pattern & Clip AST

## Context and Problem Statement
Full-length multi-track manifests (`examples/trap_anthem.json`, `examples/uk_drill_dark.json`) exceeded 15,000 lines of flat `NoteEvent` JSON because `Song` had no reusable pattern or repetition abstraction. Autonomous agents had to rely on external scripts to emit repetitive bar structures.

## Considered Options
1. Track-local pattern definitions (each track owns private patterns).
2. Song-level `BTreeMap<String, Pattern>` referenced by `Track::clips` with optional `repeat`, `transpose_semitones`, and `velocity_scale`.

## Decision Outcome
Chosen option: **Song-level `BTreeMap<String, Pattern>` + `Track::clips` (Option 2)**.
- **Declarative Schema (`src/model.rs`)**: Added `Pattern { length_beats, events }` and `Clip { pattern, start_beats, repeat, transpose_semitones, velocity_scale }`. Both `Song::patterns` and `Track::clips`/`Track::events` use `#[serde(default)]` for 100% backward compatibility.
- **Deterministic Resolution (`Track::resolved_events`)**: Expands `clips` across `0..repeat.unwrap_or(1)` iterations, shifts `start_beats`, clamps transposed `midi_note` / `chord_notes` / `slide_to_midi` to `[0, 127]`, scales velocity to `[0.0, 1.0]`, merges direct `events`, and sorts deterministically by `start_beats`.
- **Unified Rendering**: Both `Synth` and `SoundFont` renderers consume resolved events inside `compile_song`.

### Positive Consequences
- Multi-layer instruments (e.g. `808_sub` + `808_harmonics`) share one pattern with `transpose_semitones: 12`.
- Full 64-bar arrangements shrink by an order of magnitude (`examples/uk_drill_dark.json`: 15,197 lines → compact declarative patterns/clips) with bit-identical audio output.

### Negative Consequences
- Missing pattern names log a warning and skip cleanly rather than failing deserialization; nested patterns-within-patterns remain deferred to avoid recursion complexity.

## Reversal / Kill Criteria
Fully additive via `#[serde(default)]`; legacy flat `events` manifests remain valid without modification.
