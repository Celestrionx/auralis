# ADR-0004: Seamless Loop Metadata & RIFF 'smpl' Injection Architecture

## Context and Problem Statement
Game engines (Godot 4, Unity, Unreal) and visual novel audio engines require sample-accurate loop points (`loop_begin`, `loop_end`) to play seamless background music without audible pops, gaps, or manual inspector configuration. We need automated loop metadata export directly embedded in rendered WAV files.

## Considered Options
1. Sidecar JSON metadata files (`{track}.wav.json`)
2. Vorbis/OGG comment injection
3. Standard RIFF `smpl` (Sampler Chunk) embedding directly inside the WAV binary

## Decision Outcome
Chosen option: **Standard RIFF `smpl` chunk embedding (Option 3)**.
- **Declarative Schema**: Added `loop_points: Option<LoopPoints { start_beat, end_beat }>` to `Song` in `src/model.rs`.
- **Sample-Accurate Calculation**: `compile_song` calculates exact sample offsets from tempo and beat markers: $\text{sample} = \text{round}(\text{beat} \times \frac{60}{\text{tempo}} \times \text{sample\_rate})$.
- **RIFF Chunk Injection**: `export_wav` writes standard PCM data via `hound`, then appends a 68-byte RIFF `smpl` chunk (Manufacturer 0, Product 0, Sample Period in ns, Unity Note C4, and 1 infinite forward loop) and patches the RIFF header length.
- **Native Engine Compatibility**: When imported into Godot 4, the engine automatically detects the `smpl` chunk, enabling `edit/loop_mode = 1` without engine script intervention.
- **Auditory Telemetry**: `AudioTelemetry::loop_telemetry` reports `start_sample`, `end_sample`, and second-accurate loop duration; `auralis eval` automatically inspects any WAV binary for loop markers.

### Positive Consequences
- Zero external companion files needed; audio file is self-contained.
- 100% compatible with Godot 4, FL Studio, RPG Maker, and standard digital audio hardware.

### Negative Consequences
- Non-standard WAV parsers that don't traverse RIFF chunk sizes correctly might skip metadata (standard engines handle this transparently).

## Reversal / Kill Criteria
If compressed formats (e.g. OGG Vorbis / Opus) become the mandatory delivery format for mobile APK distribution, add VorbisComment `LOOPSTART` / `LOOPLENGTH` tagging alongside `smpl`.
