# `elegy-memory` — distribution

## What this binary does

Dedicated CLI for the bounded local memory operator. Provides governed
local memory persistence, search, inspect, export, contradictions, and reembed
flows over the SQLite-backed store owned by the `elegy-memory` library.

Salience, correction, scope binding, and provenance are enforced at the
library layer; this binary is a thin CLI over the same primitives.

This binary is packaged as an `elegy-plugin/v1` plugin. Release configuration is in
`distribution/surfaces.json`.

## Binary surface

- **Crate:** `plugins/memory/`
- **Binary name:** `elegy-memory`
- **Source:** `plugins/memory/src/main.rs`
- **Plugin manifest:** `.elegy-plugin/plugin.json`
- **Plugin skills:** `plugins/memory/skills/elegy-memory/`

## Distribution shape

- **Plugin archive:** `elegy-memory-v<version>.plugin.zip` (primary release contract)
- **Codex export** (derived host projection): `.codex-plugin/plugin.json` + `skills/` directory
- **Versioning:** follows workspace `version`.

## Install

```bash
# Install as a plugin package (primary lane)
elegy-plugin-packaging install --archive elegy-memory-v<version>.plugin.zip

# Export for Codex host (derived lane)
elegy-plugin-packaging export --plugin plugins/memory --host codex --output ./export
```

## Build from source

```bash
cargo build -p elegy-memory
cargo run -p elegy-memory -- --help
```

## Validation

- `cargo test -p elegy-memory`
- Plugin verify: `cargo run -p elegy-tooling --bin elegy-plugin-packaging -- verify --plugin plugins/memory`

## Where to read more

- Plugin manifest authority: `shared/plugin-sdk/src/lib.rs`
- Crate boundaries: [`AGENTS.md`](./AGENTS.md)
- Memory MCP transport adapter (separate binary):
  [`../../hosts/memory-mcp/DISTRIBUTION.md`](../../hosts/memory-mcp/DISTRIBUTION.md)
