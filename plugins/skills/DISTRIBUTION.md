# `elegy-skills` — distribution

## What this binary does

Dedicated CLI for the governed skill registry. Provides search, resolve,
inspect, profile filtering, projection, and validation over governed skill
artifacts. Reusable executable behavior over governed artifacts — the
registry's authority stays in the governed catalog.

This binary is packaged as an `elegy-plugin/v1` plugin. Release configuration is in
`distribution/surfaces.json`.

## Binary surface

- **Crate:** `plugins/skills/`
- **Binary name:** `elegy-skills`
- **Source:** `plugins/skills/src/main.rs`
- **Plugin manifest:** `.elegy-plugin/plugin.json`
- **Plugin skills:** `plugins/skills/skills/elegy-skills/`

## Distribution shape

- **Plugin archive:** `elegy-skills-v<version>.plugin.zip` (primary release contract)
- **Codex export** (derived host projection): `.codex-plugin/plugin.json` + `skills/` directory
- **Versioning:** follows workspace `version`.

## Install

```bash
# Install as a plugin package (primary lane)
elegy-plugin-packaging install --archive elegy-skills-v<version>.plugin.zip

# Export for Codex host (derived lane)
elegy-plugin-packaging export --plugin plugins/skills --host codex --output ./export
```

## Build from source

```bash
cargo build -p elegy-skills
cargo run -p elegy-skills -- --help
```

## Validation

- `cargo test -p elegy-skills`
- Plugin verify: `cargo run -p elegy-tooling -- verify --plugin plugins/skills`

## Where to read more

- Plugin manifest authority: `shared/plugin-sdk/src/lib.rs`
- Crate boundaries: [`AGENTS.md`](./AGENTS.md)
