# ADR-0005: Orchestral SoundFont Composition & Multi-Timbral Acoustic Stems

## Context and Problem Statement
To validate Auralis as a production-grade composer for narrative visual novels (`stellarpub-vn-player`) and cinematic media, we needed an end-to-end multi-timbral acoustic arrangement utilizing General MIDI SoundFont presets (Acoustic Grand Piano, String Ensemble, Solo Violin, Acoustic Bass, Orchestral Flute) rendered with spatial panning and seamless looping.

## Considered Options
1. Single synthetic multi-oscillator track
2. Multi-track hybrid arrangement combining synthesized drums and pure SoundFont acoustic stems
3. Pure SoundFont 5-stem orchestral serenade

## Decision Outcome
Chosen option: **Pure SoundFont 5-stem orchestral serenade (Option 3)**.
- **Composition**: Synthesized 40.6-second cinematic piece (`examples/orchestral_serenade_bgm.json`) in G Major / E Minor at 84 BPM.
- **Multi-Timbral Preset Map**:
  - Track 1 (`acoustic_piano`): Preset 0 (Acoustic Grand Piano), arpeggiated extended chords, Pan -0.25.
  - Track 2 (`string_ensemble`): Preset 48 (String Ensemble 1), warm sustaining orchestral beds, Pan +0.25.
  - Track 3 (`solo_violin`): Preset 40 (Violin), lyrical soaring melody line, Pan -0.15.
  - Track 4 (`acoustic_bass`): Preset 32 (Acoustic Bass), root foundation downbeats, Pan 0.0.
  - Track 5 (`orchestral_flute`): Preset 73 (Flute), ethereal upper register call-and-response counter-melody, Pan +0.35.
- **Embedded Loop**: 32-beat seamless forward loop (beats 16.0 to 48.0 = 22.86 seconds, samples 504,000 to 1,512,000) embedded via RIFF `smpl` chunk.
- **Acoustic Metrics**: Stereo correlation $\rho = 0.565$ (wide, immersive orchestral soundstage), peak -13.18 dBFS, RMS -28.71 dBFS, mid-frequency energy 54.3% matching live orchestral recordings.

### Positive Consequences
- Proven capability to generate emotional, human-grade acoustic scores purely from declarative JSON.
- Direct drop-in compatibility for Godot visual novel engines.

### Negative Consequences
- Requires distribution or local bundling of standard GM SoundFonts (e.g. `TimGM6mb.sf2`).

## Reversal / Kill Criteria
None. Confirms production capability of the Auralis audio graph compiler.
