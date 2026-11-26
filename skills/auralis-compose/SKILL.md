---
name: auralis-compose
description: Prevent generic AI music by enforcing structural Pattern/Clip arrangement, dynamic velocity phrasing, syncopation, and register separation in Auralis manifests.
---

# Auralis Composition & Anti-Slop Protocol

## 1. Anti-Slop Bans (Never Emit These)
- **Flat-Velocity Blocks**: Never emit 4+ consecutive notes with identical `velocity` (e.g., all `0.8`). Vary accent, ghost, and release notes (`0.42`–`0.95`, `std_dev >= 0.04`).
- **Four-Bar Event Copy-Paste**: For songs `>= 32` resolved notes, define reusable `patterns` and schedule `clips` with `repeat`, `transpose_semitones`, and `velocity_scale` rather than dumping flat `events` arrays.
- **Default C-Major Sine Triads**: Choose an explicit modal/harmonic palette (e.g., D Phrygian, F minor, Dorian) with voice-led inversions (`chord_notes`) and tension/release across sections.
- **Robot Grid Syndrome**: Introduce off-beat anticipation, ghost percussion (`0.35`–`0.50` velocity), passing tones, and `slide_to_midi` + `glide_beats` portamento on bass/lead lines.

## 2. Structural Arc Blueprint
1. **Intro (Bars 1–4)**: Sparse motif or filtered pad (`FilterSweep` opening `LowPass` from `400 Hz -> 4200 Hz`), establish space before full percussion drops.
2. **Drive / Main Loop (Bars 5–12)**: Full bass + rhythm pocket + counter-melody (`Clip` variation with `transpose_semitones` or octave shifts).
3. **Break / Lift (Bars 13–16)**: Drop kick/sub for 2–4 bars, automate `velocity_scale: 0.78`, bring back impactfully.
4. **Loop Integrity**: When exporting BGM loops, set `loop_points: { start_beat, end_beat }` aligned strictly to bar boundaries (`end_beat > start_beat`).

## 3. Register Separation
| Role | Target MIDI Range | Stereo & Filter Rule |
| :--- | :--- | :--- |
| **Sub / 808 Bass** | `28–48` (`E1–C3`) | Mono (`pan: 0.0`), `LowPass` <= `280 Hz` or `Sidechain` from kick |
| **Chords / Pads** | `48–72` (`C3–C5`) | `HighPass` `140–220 Hz` to clear sub, wide `unison.stereo_spread` or `StereoWidth` |
| **Lead / Arp** | `64–88` (`E4–E6`) | `HighPass` `250+ Hz`, off-center `Pan` (`-0.35`..`+0.35`), rhythmic `Delay` |
| **Hats / Air** | `72+` / `Noise` | `HighPass` `3000+ Hz`, alternating velocity accents (`0.4` / `0.78`) |

## 4. Reference Manifests
Inspect `examples/showcase_synthwave.json`, `examples/uk_drill_dark.json`, and `examples/orchestral_serenade_bgm.json` before authoring complex arrangements from scratch.
