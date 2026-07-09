# `elegy-desktop` — distribution

## What this binary does

Safe desktop automation for agentic workflows. Mouse clicks, keyboard input,
window management with dry-run and evidence capture.

This binary is packaged as an `elegy-plugin/v1` plugin. Release configuration is in
`distribution/surfaces.json`.

## Binary surface

- **Crate:** `plugins/desktop/`
- **Binary name:** `elegy-desktop`
- **Source:** `plugins/desktop/src/main.rs`
- **Plugin manifest:** `.elegy-plugin/plugin.json`
- **Plugin skills:** `plugins/desktop/skills/elegy-desktop/`

## Distribution shape

- **Release plugin archive:** `elegy-desktop-plugin-<target>.zip` (primary GitHub release and marketplace contract)
- **Local pack default:** `elegy-desktop-v<version>.plugin.zip` (ad hoc output when `pack --output` is omitted)
- **Codex export** (derived host projection): `.codex-plugin/plugin.json` + `skills/` directory
- **Versioning:** follows workspace `version`.

## Install

```bash
# Install as a plugin package (primary lane)
elegy-plugin-packaging install --archive elegy-desktop-plugin-<target>.zip

# Export for Codex host (derived lane)
elegy-plugin-packaging export --plugin plugins/desktop --host codex --output ./export
```

## Build from source

```bash
cargo build -p elegy-desktop
cargo run -p elegy-desktop -- --help
```

## Validation

- `cargo test -p elegy-desktop`
- Plugin verify: `cargo run -p elegy-tooling --bin elegy-plugin-packaging -- verify --plugin plugins/desktop`

## Where to read more

- Plugin manifest authority: `shared/plugin-sdk/src/lib.rs`
- Crate boundaries: [`AGENTS.md`](./AGENTS.md)
