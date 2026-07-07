# `elegy-documentation` — distribution

## What this binary does

Dedicated CLI for the authority-aware documentation inspector. Provides
`inspect | map | check | new (adr|spec|note)` and `index` flows over the
Elegy documentation doctrine. Validates frontmatter, freshness, ADR/spec
classification, and the `docs/docs-index.md` content bundle shape.

This binary is packaged as an `elegy-plugin/v1` plugin. Release configuration is in
`distribution/surfaces.json`.

## Binary surface

- **Crate:** `plugins/documentation/`
- **Binary name:** `elegy-documentation`
- **Source:** `plugins/documentation/src/main.rs`
- **Plugin manifest:** `.elegy-plugin/plugin.json`
- **Plugin skills:** `plugins/documentation/skills/elegy-documentation/`

## Distribution shape

- **Plugin archive:** `elegy-documentation-v<version>.plugin.zip` (primary release contract)
- **Codex export** (derived host projection): `.codex-plugin/plugin.json` + `skills/` directory
- **Versioning:** follows workspace `version`.

## Install

```bash
# Install as a plugin package (primary lane)
elegy-plugin-packaging install --archive elegy-documentation-v<version>.plugin.zip

# Export for Codex host (derived lane)
elegy-plugin-packaging export --plugin plugins/documentation --host codex --output ./export
```

## Build from source

```bash
cargo build -p elegy-documentation
cargo run -p elegy-documentation -- check --project . --json
```

## Validation

- `cargo test -p elegy-documentation`
- Plugin verify: `cargo run -p elegy-tooling --bin elegy-plugin-packaging -- verify --plugin plugins/documentation`

## Where to read more

- Plugin manifest authority: `shared/plugin-sdk/src/lib.rs`
- Crate boundaries: [`AGENTS.md`](./AGENTS.md)
