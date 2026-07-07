# `elegy-observe` — distribution

## What this binary does

Desktop and OS observation for agentic workflows. Process snapshots, window
tracking, clipboard, screen capture, and filesystem diffing.

This binary is packaged as an `elegy-plugin/v1` plugin. Release configuration is in
`distribution/surfaces.json`.

## Binary surface

- **Crate:** `plugins/observe/`
- **Binary name:** `elegy-observe`
- **Source:** `plugins/observe/src/main.rs`
- **Plugin manifest:** `.elegy-plugin/plugin.json`
- **Plugin skills:** `plugins/observe/skills/elegy-observe/`

## Distribution shape

- **Plugin archive:** `elegy-observe-v<version>.plugin.zip` (primary release contract)
- **Codex export** (derived host projection): `.codex-plugin/plugin.json` + `skills/` directory
- **Versioning:** follows workspace `version`.

## Install

```bash
# Install as a plugin package (primary lane)
elegy-plugin-packaging install --archive elegy-observe-v<version>.plugin.zip

# Export for Codex host (derived lane)
elegy-plugin-packaging export --plugin plugins/observe --host codex --output ./export
```

## Build from source

```bash
cargo build -p elegy-observe
cargo run -p elegy-observe -- --help
```

## Validation

- `cargo test -p elegy-observe`
- Plugin verify: `cargo run -p elegy-tooling --bin elegy-plugin-packaging -- verify --plugin plugins/observe`

## Where to read more

- Plugin manifest authority: `shared/plugin-sdk/src/lib.rs`
- Crate boundaries: [`AGENTS.md`](./AGENTS.md)
