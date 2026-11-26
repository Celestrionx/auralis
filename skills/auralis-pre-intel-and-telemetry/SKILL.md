---
name: auralis-pre-intel-and-telemetry
description: Run Auralis dual-gate verification: static pre-render manifest lint before synthesis and closed-loop acoustic telemetry evaluation after compile.
---

# Auralis Dual-Gate Intelligence Loop

Never render blindly or accept the first compilation. Enforce both gates on every composition:

```text
  init(archetype) ──▶ [GATE 1: Pre-Render Lint] ──▶ compile ──▶ [GATE 2: Post-Render Telemetry]
                         (auralis_lint_manifest)                   (auralis_eval_wav)
                         Target: score >= 75, 0 errors             Target: 0 diagnostic flags
```

## Gate 1 — Pre-Render Intelligence (`auralis_lint_manifest` / `auralis lint`)
Run static analysis in `<1 ms` before DSP synthesis. Resolve all findings:
- `EMPTY_SONG` / `UNRESOLVED_CLIP_PATTERN` *(Error)*: Fix missing tracks or misspelled `clip.pattern` names.
- `FLAT_VELOCITY` *(Warning)*: Add musical accents and ghost notes so velocity standard deviation `>= 0.02`.
- `BARE_SYNTH_TIMBRE` *(Warning)*: Add `unison`, `fm`, `lfo`, `filter_env`, `pitch_env`, or track `fx`.
- `MISSING_PATTERN_REUSE` *(Warning)*: Refactor repetitive `>= 32`-event songs into `patterns` + `clips`.
- `CENTER_STACKED_MIX` *(Warning)*: Add `Pan`, `StereoWidth`, `Delay`/`Reverb`, or `unison.stereo_spread`.
- `UNCARVED_LOW_END` *(Warning)*: Apply `HighPass` (`>= 120 Hz`) or `Sidechain` when multiple melodic tracks share notes `< MIDI 55`.
- `MISSING_MASTER_DYNAMICS` *(Warning)*: Add `SoftClip` or `Compressor` to `master_fx`.

## Gate 2 — Post-Render Telemetry (`auralis_compile_audio` / `auralis_eval_wav`)
Inspect the returned `AudioTelemetry` JSON and iterate until clear:
- `CLIPPING_DETECTED`: Reduce track `Gain`, enable `master_fx: [{ "type": "softclip" }]`, or lower hot velocities.
- `DULL_HIGH_END`: Add hi-hats, brighten `LowPass` cutoffs, or add harmonic FM/saw layers (`high_ratio >= 0.01`).
- `LOW_DYNAMIC_RANGE`: Ease heavy `Compressor`/`Saturate` drive or widen note velocity spread.
- `PHASE_CANCELLATION_RISK`: Reduce excessive `StereoWidth > 1.5` or conflicting stereo delays on low frequencies.
- `SUB_BASS_DOMINANT`: `HighPass` non-bass tracks (`120–200 Hz`) and trim sub `Gain` by `-2` to `-4 dB`.
- `HIGH_SECTION_VARIANCE`: Smooth abrupt `sections` energy jumps with `FilterSweep` transitions or transitional fills.
