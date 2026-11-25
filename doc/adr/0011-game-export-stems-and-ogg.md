# ADR-0011: Game Export Pipeline — Adaptive Stems & Loop-Tagged OGG Vorbis

## Context and Problem Statement
Game engines (`stellarpub-vn-player`, `inkquarium`) require two export capabilities beyond single uncompressed WAV mixes:
1. Equal-length, loop-aligned per-track stems for vertical adaptive layering.
2. Compressed OGG Vorbis audio carrying native `LOOPSTART` and `LOOPLENGTH` metadata tags to avoid bloated DLC packages.

## Considered Options
1. Add native Rust Vorbis encoder crates (`vorbis_rs` / `ogg`) to `Cargo.toml` (requires external C headers/networked crate fetch).
2. Implement pure-Rust `compile_stems` / `compile_song_with_stems` alongside a zero-crate `ffmpeg` (`libvorbis`) bridge (`export_ogg` / `export_audio`) triggered when `.ogg` output is requested.

## Decision Outcome
Chosen option: **Pure-Rust adaptive stems + zero-crate `ffmpeg` OGG bridge (Option 2)**.
- **Adaptive Stems (`src/graph.rs`, `src/main.rs`)**: `compile_song_with_stems` shares the exact Pass 1/Pass 2 parallel graph, returning pre-`master_fx` equal-length stereo stems with identical `loop_samples`. CLI `compile --stems` writes sanitized `<dir>/<base>_<track_id>.<ext>` files.
- **OGG Vorbis Export (`export_ogg` / `export_audio`)**: Keeps `Cargo.toml` unchanged and invokes `ffmpeg -c:a libvorbis` with `-metadata LOOPSTART=<n> -metadata LOOPLENGTH=<n>` for `.ogg` targets.

### Positive Consequences
- Zero `Cargo.toml` dependencies added; `.wav` compilation stays 100% pure Rust.
- Godot 4 reads `LOOPSTART` / `LOOPLENGTH` Vorbis comments out of the box.

### Negative Consequences
- `.ogg` export requires `ffmpeg` with `libvorbis` installed on PATH (fails cleanly with an error otherwise).

## Reversal / Kill Criteria
Swap `export_ogg` internals to a pure-Rust encoder crate if offline self-contained OGG binaries become mandatory.
