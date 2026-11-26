---
name: auralis-sound-design
description: Replace dry single-oscillator beeps with rich Synth unison/FM/filter envelopes, SoundFont presets, Sample layering, and Sidechain/FX depth.
---

# Auralis Sound Design & Mix Depth

## 1. Never Leave `Synth` Bare
A raw `Waveform::Sine` or `Square` with default `Adsr` and empty `fx` sounds like a test tone. Every non-percussion `Synth` track must shape timbre with at least one modulation or FX stage:
- **Wide Analog Pads / Leads**: `waveform: "sawtooth"`, `unison: { voices: 4..7, detune_cents: 12..22, stereo_spread: 0.75..0.95 }`, plus `LowPass` (`1800–4500 Hz`) or `filter_env`.
- **Organic Keys / Bells / Plucks**: `fm: { mod_ratio: 2.0..3.5, mod_index: 1.2..4.5 }` + `filter_env: { start_cutoff_hz: 4500, end_cutoff_hz: 600, decay_sec: 0.25, q: 1.4 }`.
- **Expressive Vibrato**: `lfo: { rate_hz: 4.5..6.0, depth_semitones: 0.12..0.25 }` on sustained leads/strings.
- **808 / Kick Punch**: `pitch_env: { drop_semitones: 24..42, decay_sec: 0.04..0.09 }` + `Saturate { drive: 1.3..2.0 }`.

## 2. Acoustic & Sample Layering
- **SoundFont 2 (`TimGM6mb.sf2`)**: Always query `auralis_inspect_soundfont` first rather than guessing `bank`/`preset` integers. Combine sustain (`sustain: true` on `NoteEvent`) with a subtle `Reverb { room_size: 0.55, damping: 0.45, mix: 0.22 }`.
- **External WAV Samples**: Use `InstrumentConfig::Sample { wav_path, root_midi }` for authentic transients/foley layered under synth bodies.

## 3. Mix Carving & Master Bus
1. **Kick-to-Bass / Pad Clarity**: Add `Sidechain { source_track: "kick", amount: 0.45..0.75, threshold_db: -24.0, attack_sec: 0.005, release_sec: 0.14 }` on bass and pads.
2. **Stereo Panorama**: Distribute mid/high tracks across `-0.6..+0.6` via `Pan` or `unison.stereo_spread`; keep kick and sub centered.
3. **Master Glue**: For `>= 3` tracks, always configure `master_fx` with `Compressor` and/or `SoftClip` to tame summed transients before `CLIPPING_DETECTED` occurs.
