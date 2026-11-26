# ADR-0018: Portable Asset Resolution & Zero-Friction User Onboarding

## Context and Problem Statement
Auralis examples and archetypes reference SoundFont (`assets/soundfonts/TimGM6mb.sf2`) and WAV sample paths relative to the repository root. When an external user installs `auralis` globally (`cargo install` or release binary) and runs `auralis serve` or `auralis compile` from another directory (such as a game project), `File::open("assets/soundfonts/TimGM6mb.sf2")` fails and silently triggers sine-wave fallback. Additionally, the repository lacked CI verification, release packaging, and a copy-paste MCP client configuration in `README.md`.

## Considered Options
1. Embed the entire 5.8 MB `TimGM6mb.sf2` inside the compiled binary via `include_bytes!`.
2. Add deterministic multi-root asset resolution (`resolve_asset_path`), surface `MISSING_ASSET_FILE` in `prelint.rs`, bundle `TimGM6mb.sf2` in release tarballs/installer (`~/.local/share/auralis/assets/`), and document copy-paste MCP setup + CI workflows.

## Decision Outcome
Chosen option: **Deterministic Multi-Root Asset Resolution & Release Packaging (Option 2)**.
- **Asset Resolver (`src/soundfont.rs`, `src/sampler.rs`)**: `resolve_asset_path` checks CWD/exact path, `AURALIS_SOUNDFONT` (for `.sf2`), `AURALIS_ASSETS_DIR`, executable-adjacent paths (`<exe_dir>`, `<exe_dir>/../share/auralis`, parent directories), and XDG user data (`$XDG_DATA_HOME/auralis` or `~/.local/share/auralis`).
- **Pre-Lint Guard (`src/prelint.rs`)**: Added `MISSING_ASSET_FILE` warning when `SoundFont` or `Sample` tracks reference unresolved paths before synthesis.
- **CI, Release & Onboarding (`.github/workflows/`, `scripts/install.sh`, `README.md`)**: Added CI (`fmt`, `clippy -D warnings`, `test`), cross-platform release packaging bundling `TimGM6mb.sf2`, one-line installer, and copy-paste MCP client config.

### Positive Consequences
- Keeps the Rust binary lean (no 5.8 MB binary bloat) while enabling `auralis serve` to locate `TimGM6mb.sf2` from any working directory.
- Catches broken sample/SoundFont paths in `<1 ms` during `auralis lint` instead of silently falling back during synthesis.

### Negative Consequences
- Users installing purely via `cargo install --git` (without `scripts/install.sh` or repo assets) must set `AURALIS_SOUNDFONT` or copy `TimGM6mb.sf2` into `~/.local/share/auralis/assets/soundfonts/`.

## Reversal / Kill Criteria
Embed a compressed minimal SF2 via `include_bytes!` if external users repeatedly fail SoundFont setup after `install.sh` / release adoption.
