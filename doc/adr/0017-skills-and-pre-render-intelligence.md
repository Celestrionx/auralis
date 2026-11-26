# ADR-0017: Compositional Skills & Pre-Render Intelligence (`src/prelint.rs`)

## Context and Problem Statement
Auralis previously provided only post-render acoustic feedback (`src/telemetry.rs`) and developer-focused repository rules (`AGENTS.md`). Autonomous agents scaffolding songs via `auralis_init_manifest` received an empty `Song::default()` (or a minimal 2-track `kick` + C-major triad in CLI `init`) with zero pre-render guidance, frequently generating flat-velocity, bare-oscillator, center-stacked audio manifests.

## Considered Options
1. Add static `skills/*/SKILL.md` documentation only without modifying the Rust CLI or MCP server.
2. Pair four composable `SKILL.md` guides (`skills/`) with a deterministic pre-render manifest intelligence engine (`src/prelint.rs`), archetype-backed `init`, CLI `lint`, and MCP `prompts`/`resources` + `auralis_lint_manifest`.

## Decision Outcome
Chosen option: **Dual-layer Skills + Pre-Render Intelligence (Option 2)**.
- **Pre-Render Intelligence (`src/prelint.rs`)**: Added deterministic `analyze_manifest(&Song) -> ManifestAudit` (`quality_score` 0–100, `passed` gate, and 9 checks: `EMPTY_SONG`, `UNRESOLVED_CLIP_PATTERN`, `FLAT_VELOCITY`, `BARE_SYNTH_TIMBRE`, `MISSING_PATTERN_REUSE`, `CENTER_STACKED_MIX`, `UNCARVED_LOW_END`, `MISSING_MASTER_DYNAMICS`, `INVALID_LOOP_RANGE`/`UNBOUNDED_LOOP_END`) plus 5 non-generic archetypes (`synthwave`, `ambient`, `drill`, `orchestral`, `sfx_impact`).
- **CLI & MCP Integration (`src/main.rs`, `src/mcp.rs`)**: Added `auralis lint <manifest> [--strict]`, `auralis init --archetype <name>`, `auralis_lint_manifest`, embedded `Pre-Lint` output in `auralis_compile_audio`, and MCP `prompts/list|get` + `resources/list|read` serving `skills/*/SKILL.md` via compile-time `include_str!`.

### Positive Consequences
- Agents fail fast in `<1 ms` before DSP rendering on generic or malformed manifests.
- Any external MCP client auto-discovers compositional and sound-design rules without cloning the repository.
- Maintains strict `DESIGN.md` separation (`model.rs` stays free of DSP/I/O; zero external crate additions).

### Negative Consequences
- `skills/*/SKILL.md` edits require rebuilding the binary for MCP `prompts/resources` updates because they are embedded via `include_str!`.

## Reversal / Kill Criteria
Disable `Pre-Lint` warnings or adjust rule thresholds in `src/prelint.rs` if false positives appear on valid minimalist compositions.
