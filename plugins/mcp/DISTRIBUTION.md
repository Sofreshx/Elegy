# `elegy-mcp` — distribution

## What this binary does

Dedicated CLI for MCP descriptor authoring and analysis. Wraps the reusable
author/analyze behavior over governed MCP descriptors.

Lower-level MCP-to-skill generation is contributor tooling; this binary
focuses on the authoring and analysis lane.

This binary is packaged as an `elegy-plugin/v1` plugin. Release configuration is in
`distribution/surfaces.json`.

## Binary surface

- **Crate:** `plugins/mcp/`
- **Binary name:** `elegy-mcp`
- **Source:** `plugins/mcp/src/main.rs`
- **Plugin manifest:** `.elegy-plugin/plugin.json`
- **Plugin skills:** `plugins/mcp/skills/elegy-mcp/`

## Distribution shape

- **Plugin archive:** `elegy-mcp-v<version>.plugin.zip` (primary release contract)
- **Codex export** (derived host projection): `.codex-plugin/plugin.json` + `skills/` directory
- **Versioning:** follows workspace `version`.

## Install

```bash
# Install as a plugin package (primary lane)
elegy-plugin-packaging install --archive elegy-mcp-v<version>.plugin.zip

# Export for Codex host (derived lane)
elegy-plugin-packaging export --plugin plugins/mcp --host codex --output ./export
```

## Build from source

```bash
cargo build -p elegy-mcp
cargo run -p elegy-mcp -- --help
```

## Validation

- `cargo test -p elegy-mcp`
- Plugin verify: `cargo run -p elegy-tooling --bin elegy-plugin-packaging -- verify --plugin plugins/mcp`

## Where to read more

- Plugin manifest authority: `shared/plugin-sdk/src/lib.rs`
- Crate boundaries: [`AGENTS.md`](./AGENTS.md)
