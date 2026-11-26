<p align="center">
  <img src="doc/assets/auralis-logo.png" alt="Auralis logo" width="180" />
</p>

<h1 align="center">Auralis (<code>v0.2.1</code>)</h1>

<p align="center">
  Headless Audio Graph Compiler &amp; Auditory Telemetry Engine in pure Rust for autonomous AI agents.<br />
  <a href="https://www.celestrionx.com/#demos">Listen to Live Compiled Showcases →</a>
</p>

## For AI Agents
See [AGENTS.md](AGENTS.md) for crate layout, exact commands, architectural boundaries, and git workflow, [DESIGN.md](DESIGN.md) for invariants, [skills/](skills/) for anti-generic composition & sound-design skills, and [doc/adr/](doc/adr/) for Architecture Decision Records (`ADR-0001` through `ADR-0018`).

## Capabilities (`v0.2.1`)
- **Declarative Composition**: JSON/YAML `Song` manifests with reusable `Pattern` definitions, track `Clip` scheduling (`repeat`, `transpose_semitones`, `velocity_scale`), chords, portamento (`slide_to_midi`), and seamless `LoopPoints`.
- **Pre-Render Intelligence & Skills**: Static `<1 ms` manifest linting (`auralis lint` / `auralis_lint_manifest`) catching flat velocities, bare oscillators, missing `Pattern`/`Clip` reuse, and low-end masking before synthesis, paired with archetype scaffolding (`init --archetype`) and 4 embedded composition/SFX skills.
- **Three Synthesis Backends**:
  - `Synth`: Multi-waveform oscillators, ADSR & pitch envelopes, unison detune with constant-power stereo spread, vibrato LFO, 2-op FM synthesis, per-note `FilterEnvelope`, and percussion/noise synthesis.
  - `SoundFont`: Cached SF2 rendering via `rustysynth` with block-based MIDI pitch bend and CC64 damper sustain.
  - `Sample`: Cached external WAV one-shot and pitched/glide playback (`InstrumentConfig::Sample { wav_path, root_midi }`) with graceful missing-file fallback.
- **Deterministic Parallel DSP & FX**: Multi-threaded track rendering (`std::thread::scope`), resonant `LowPass` / `HighPass` / `BandPass` biquad filters, beat-synced `FilterSweep`, `Saturate`, `SoftClip`, `Compressor`, track-keyed `Sidechain` ducking, `StereoWidth`, tempo-synced ping-pong `Delay`, Schroeder stereo `Reverb`, and `master_fx`.
- **Game & DAW Export**: 32-bit/16-bit WAV with RIFF `smpl` loop chunks, OGG Vorbis with `LOOPSTART`/`LOOPLENGTH` tags (via `ffmpeg`), and equal-length `--stems` export.
- **Closed-Loop Auditory Telemetry**: Peak/RMS dBFS, crest factor, stereo correlation, side-to-mid ratio, 32-window Hann FFT spectral energy ratios, time-sliced `sections`, and automated mix `diagnostics`.
- **MCP Server**: Built-in stdio server exposing `auralis_init_manifest`, `auralis_lint_manifest`, `auralis_compile_audio`, `auralis_inspect_soundfont`, `auralis_eval_wav`, plus `prompts` and `resources` serving all 4 `skills/`.

## Install

### One-Line Binary + Bundled SoundFont (Linux / macOS)
```bash
curl -fsSL https://raw.githubusercontent.com/Celestrionx/auralis/main/scripts/install.sh | bash
```

### From Source (Cargo)
```bash
cargo install --git https://github.com/Celestrionx/auralis
```
> When running outside the repo root, `auralis` resolves `assets/...` automatically via `AURALIS_SOUNDFONT`, `AURALIS_ASSETS_DIR`, `<exe_dir>/assets`, or `~/.local/share/auralis/assets`.

## MCP Client Setup (Claude / Cursor / Antigravity / OpenCode)
Add to your MCP config (`~/.config/.../mcp.json` or project `.mcp.json`):
```json
{
  "mcpServers": {
    "auralis": {
      "command": "auralis",
      "args": ["serve"]
    }
  }
}
```
Exposes tools (`auralis_init_manifest`, `auralis_lint_manifest`, `auralis_compile_audio`, `auralis_inspect_soundfont`, `auralis_eval_wav`), `prompts`, and `resources` (`auralis://skills/*`).

## Quick Start
```bash
auralis init demo.json --archetype synthwave
auralis lint demo.json --strict
auralis compile demo.json -o output.wav --stems
auralis compile examples/showcase_power_metal.json -o power_metal.ogg
auralis eval output.wav
auralis serve
```

## Examples
Explore [examples/](examples/) for multi-genre BGM showcases (`showcase_synthwave.json`, `showcase_power_metal.json`, `showcase_lofi_jazzhop.json`, `showcase_edm_future_bass.json`, `showcase_cinematic_ambient.json`, `trap_anthem.json`, `uk_drill_dark.json`) and procedural SFX (`sfx_door_close.json`, `sfx_lightning_thunder.json`, `sfx_laser_shoot.json`).
