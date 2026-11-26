---
name: auralis-sfx-foley
description: Synthesize non-generic procedural game SFX and seamless game-engine audio loops using 3-layer transients, pitch/filter drops, and RIFF/OGG loop tags.
---

# Auralis Procedural SFX & Game Loop Protocol

## 1. Three-Layer Procedural SFX Rule
Single-oscillator UI blips sound cheap. Layer 3 distinct tracks over short beats (`0.25–2.0` beats at `120 BPM`):
1. **Transient Snap (`0–30 ms`)**: `Waveform::Noise` or short `FmConfig` burst with `attack_sec: 0.001`, `decay_sec: 0.025`, `sustain_level: 0.0`, `HighPass` (`1200–3500 Hz`).
2. **Body / Weight**: `Waveform::Sine` or `Triangle` with `pitch_env: { drop_semitones: 18..48, decay_sec: 0.04..0.15 }` + `Saturate { drive: 1.4..2.2 }`.
3. **Tail / Space**: Filtered `Noise` or pitched tail with `FilterSweep` closing downward (`end_hz < start_hz`) + short `Reverb { room_size: 0.35..0.65, damping: 0.5, mix: 0.25 }`.

## 2. Game Engine Loop Precision
- **Seamless BGM**: Set `loop_points: { start_beat: 0.0, end_beat: <bar_aligned> }`. Let reverb/delay tails settle or use bar-aligned `LoopPoints` so `export_wav` injects RIFF `smpl` chunks and `export_ogg` embeds `LOOPSTART`/`LOOPLENGTH` Vorbis comments (Godot 4 compatible).
- **Interactive Stems**: Use `cargo run -- compile song.json -o theme.ogg --stems` to emit equal-length stems for adaptive vertical remixing.
- **Reference Examples**: Study `examples/sfx_door_close.json`, `examples/sfx_laser_shoot.json`, and `examples/sfx_lightning_thunder.json`.
