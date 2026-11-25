# AGENTS.md — auralis

> Auto-loaded by agentic tools (opencode, Gemini CLI/agy, Codex, Copilot). Closest AGENTS.md wins; this is the project-root contract.

## Overview
- Headless Audio Graph Compiler and Auditory Telemetry Engine in pure Rust for autonomous agents.
- Compiles declarative music manifests (JSON/YAML) into rendered audio (WAV) and extracts quantitative acoustic metrics (Peak, RMS, FFT spectral energy) for closed-loop agent composition.
- Single source of truth for agents: this file + DESIGN.md (pointer below) + doc/adr/ (decision history).
- Status: v0.2.0 (pre-1.0).

## Map
- `src/` — crate source:
  - `model.rs` — declarative AST (`Song`, `Track`, `Pattern`, `Clip`, `NoteEvent`, `InstrumentConfig`, `FxConfig`).
  - `graph.rs` — parallel audio graph compilation, DSP FX chains, `smpl` WAV & OGG Vorbis (`ffmpeg`) export.
  - `synth.rs` — headless tone/percussion generators, unison stereo spread, LFO, 2-op FM, filter/pitch envelopes, ADSR.
  - `soundfont.rs` — cached SF2 acoustic synthesizer (`rustysynth`) with pitch bend and CC64 damper sustain.
  - `sampler.rs` — cached WAV one-shot and pitched/glide sample playback instrument.
  - `telemetry.rs` — auditory telemetry engine (RMS, peak dBFS, crest factor, stereo correlation, 32-window Hann FFT, sections, diagnostics).
  - `mcp.rs` — Model Context Protocol stdio tool server (`init`, `compile`, `inspect_soundfont`, `eval_wav`).
  - `lib.rs` — public library API exposing compiler, sampler, soundfont inspection, and telemetry routines.
  - `main.rs` — CLI entrypoint (`init`, `compile`, `eval`, `serve`).
- `examples/` — declarative song & procedural SFX manifests (`showcase_*.json`, `sfx_*.json`, `trap_anthem.json`, `uk_drill_dark.json`).
- `tests/` — integration and smoke tests:
  - `smoke_test.rs` — audio rendering, DSP, exporter, MCP, and telemetry validation suite.
- `DESIGN.md` — architectural invariants and non-goals. NOT auto-loaded; read before structural changes.
- `doc/adr/` — immutable architecture decision records (`ADR-0001`–`ADR-0016`). Append-only, never edit in-place.
- `.opencode/` — agent config: permissions, reviewer subagent, start/status commands.

## Commands (exact; verified)
- Build: `cargo build` (exit 0)
- Test: `cargo test` (exit 0)
- Lint: `cargo clippy --all-targets` (exit 0; zero warnings permitted)
- Format: `cargo fmt`, verify with `cargo fmt --check` (exit 0)
- Run CLI: `cargo run -- <command> [args]`
  - Scaffold manifest: `cargo run -- init demo.json`
  - Compile audio & optional stems: `cargo run -- compile demo.json -o output.wav [--stems]`
  - Evaluate telemetry: `cargo run -- eval output.wav`
  - Run MCP server: `cargo run -- serve`

> Dependencies are managed purely through `Cargo.toml`. Never invent commands; check Cargo.toml and README first.

## Git workflow
- Trunk-based: commit to `main` directly.
- Conventional Commits (`feat:`, `fix:`, `docs:`, `refactor:`, `test:`, `chore:`), imperative subject, atomic commits.
- Before every commit: `cargo fmt --check`, `cargo clippy --all-targets`, and `cargo test` must all exit with code 0.
- No force-push, ever. Pushes require explicit human approval (permission-gated in `.opencode/opencode.json`).

## Boundaries
Always
- Maintain deterministic offline DSP rendering: identical manifests produce identical PCM audio buffers.
- Keep `test`, `clippy`, and `fmt` clean (exit code 0) before declaring work done.
- Record architectural shifts as new MADR records in `doc/adr/`.
- Keep `model.rs` strictly decoupled from direct audio hardware or I/O calls.

Ask first
- Adding or upgrading dependencies in `Cargo.toml`.
- Mutating public CLI or MCP tool schemas.
- Editing files under `.opencode/`, `AGENTS.md`, or `DESIGN.md`.

Never
- `git push --force`, `rm -rf`, or destructive git history rewrites.
- Commit or log secrets, tokens, or credentials.
- Introduce GUI or hardware soundcard dependencies into the core compilation pipeline.
