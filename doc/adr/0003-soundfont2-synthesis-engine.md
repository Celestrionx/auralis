# ADR-0003: SoundFont 2 (SF2) Acoustic Synthesis Engine

## Context and Problem Statement
Algorithmic waveform synthesis (sine, saw, noise) excels at synthetic pads, chiptunes, and electronic drums, but cannot authentically emulate natural acoustic instruments (grand piano, acoustic strings, woodwinds). We need support for SoundFont 2 (`.sf2`) banks while preserving headless, deterministic offline rendering and zero system-audio driver dependencies.

## Considered Options
1. C-library bindings to `libfluidsynth`
2. Pure Rust SoundFont 2 synthesizer (`rustysynth`)
3. Pre-rendered PCM sample slicer

## Decision Outcome
Chosen option: **Pure Rust SoundFont 2 synthesizer via `rustysynth` (Option 2)**.
- **Untagged Manifest Schema**: `InstrumentConfig` polymorphically supports both algorithmic `Synth` and `SoundFont { soundfont_path, bank, preset }`.
- **SoundFont In-Memory Cache**: Global thread-safe cache (`Arc<SoundFont>`) prevents redundant multi-megabyte disk re-reads across parallel tracks.
- **Block-Based Timeline Dispatch**: Midi events (NoteOn, NoteOff, Bank/Preset change) are sorted and dispatched to `rustysynth::Synthesizer` in discrete sub-chunks, outputting stereo audio buffers.
- **Graceful Fault Tolerance**: Missing or corrupted SoundFont files emit diagnostic warnings and fail open (silent buffer) rather than crashing the compiler.

### Positive Consequences
- Zero C/C++ build toolchains or dynamic library linker issues.
- Seamless coexistence of SoundFont tracks and algorithmic synth tracks in the same song manifest.
- All downstream audio FX (Gain, Pan, Filters) apply identically to SoundFont and Synth stems.

### Negative Consequences
- SoundFont rendering throughput is slightly lower than raw analytical oscillators due to multi-layer sample interpolation and envelope modulation.

## Reversal / Kill Criteria
If SF3 (compressed Vorbis SoundFont) or complex gigabyte DFD (Direct-From-Disk) streaming becomes mandatory, evaluate `rustysynth-ext` or headless SFZ streaming engines.
