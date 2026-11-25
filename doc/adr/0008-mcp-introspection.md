# ADR-0008: MCP Introspection Tools (`auralis_inspect_soundfont`, `auralis_eval_wav`)

## Context and Problem Statement
Auralis's stdio MCP server (`src/mcp.rs`) exposed only `auralis_init_manifest` and `auralis_compile_audio`. Autonomous agents could not discover SoundFont 2 bank/preset IDs (such as `bank=128, preset=25` for TR-808 drums) or evaluate existing WAV files without shelling out to external scripts or the CLI.

## Considered Options
1. Keep MCP limited to `init` and `compile` and rely on CLI `eval` + manual SF2 parsing.
2. Add pure-Rust `auralis_inspect_soundfont` and `auralis_eval_wav` MCP tools while refactoring request dispatch into a testable `handle_mcp_request` entrypoint.

## Decision Outcome
Chosen option: **Additive MCP introspection tools + testable dispatcher (Option 2)**.
- **`auralis_inspect_soundfont`**: Loads `.sf2` via `get_or_load_soundfont`, optionally filters by `bank`, and returns sorted `{bank, preset, name}` entries.
- **`auralis_eval_wav`**: Decodes 16/24/32-bit int or float WAVs via `hound`, reads RIFF `smpl` loop markers, and returns full `AudioTelemetry` JSON.
- **Compatibility**: `auralis_init_manifest` and `auralis_compile_audio` schemas remain untouched.

### Positive Consequences
- Agents discover acoustic/percussion presets and re-evaluate rendered audio purely through MCP.
- JSON-RPC protocol behavior is covered directly in `tests/smoke_test.rs`.

### Negative Consequences
- None; strictly additive MCP tool registrations.

## Reversal / Kill Criteria
Remove the two tool match arms in `src/mcp.rs` if MCP surface minimization is ever required.
