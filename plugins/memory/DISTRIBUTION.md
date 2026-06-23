# `elegy-memory` — distribution

## What this binary does

Dedicated CLI for the bounded local memory operator. Provides governed
local memory persistence, search, inspect, export, contradictions, and reembed
flows over the SQLite-backed store owned by the `elegy-memory` library.

Salience, correction, scope binding, and provenance are enforced at the
library layer; this binary is a thin CLI over the same primitives.

## Binary surface

- **Crate:** `rust/features/elegy-memory/`
- **Binary name:** `elegy-memory`
- **Source:** `rust/features/elegy-memory/src/cli.rs`
- **Library consumers:** `rust/features/elegy-memory-mcp` (transport adapter),
  `rust/bin/elegy-cli` (umbrella CLI subcommands).

## Distribution shape

- **CLI archive asset family:** `elegy-memory-<cliVersion>-<target>.zip`
- **Wrapper archive:** `elegy-memory-wrapper-<bundleVersion>.zip`
- **Release catalog entry:** `distribution/surfaces.json` (name: `elegy-memory`)
- **Skill fixture:** `contracts/fixtures/skill.elegy-memory.json`
- **Versioning:** follows workspace `version`.

## Install

```bash
# Canonical installer (recommended)
bash ./scripts/install-distribution.sh -Tag vX.Y.Z -Destination ./tools/elegy -CliSurfaces elegy-memory -Force
```

```powershell
# Native-pwsh entry point: thin shim that forwards all args to bash (requires bash in PATH)
pwsh ./scripts/install-distribution.ps1 -Tag vX.Y.Z -Destination ./tools/elegy -CliSurfaces elegy-memory -Force
```

## Build from source

```bash
cd rust
cargo build -p elegy-memory
cargo run -p elegy-memory -- --help
```

## Validation

- `cargo test -p elegy-memory`
- Library contract tests under
  `rust/features/elegy-memory/tests/{cli,conformance,governed_memory,integration,local_store}.rs`

## Where to read more

- Memory architecture and MVP scope:
  [`docs/architecture/mvp-scope.md`](./docs/architecture/mvp-scope.md)
- Memory model (scopes, scoring, decay, confidence, provenance):
  [`docs/architecture/memory-model.md`](./docs/architecture/memory-model.md)
- Memory storage schema:
  [`docs/architecture/storage-schema.md`](./docs/architecture/storage-schema.md)
- Crate boundaries: [`rust/features/elegy-memory/AGENTS.md`](./AGENTS.md)
- Memory MCP transport adapter (separate binary):
  [`../elegy-memory-mcp/DISTRIBUTION.md`](../elegy-memory-mcp/DISTRIBUTION.md)
