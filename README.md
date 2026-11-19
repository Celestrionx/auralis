# Auralis

Headless Audio Graph Compiler & Auditory Telemetry Engine in pure Rust for autonomous AI agents.

## For AI Agents
See [AGENTS.md](AGENTS.md) for project layout, exact commands, architectural boundaries, and git workflow.

## Overview
Auralis enables LLMs to compose, synthesize, and evaluate music via:
- **Declarative Composition**: JSON/YAML song manifests.
- **Offline DSP Graph**: Multi-threaded, deterministic software rendering to WAV.
- **Auditory Telemetry**: Quantitative FFT, RMS, and peak metrics providing closed-loop sensory feedback.
- **Agent Server**: Built-in Model Context Protocol (MCP) tool provider.
