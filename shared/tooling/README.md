# Elegy tooling — readiness: implemented, not agent-routable

`shared/tooling/` contains the host-neutral `elegy` authoring CLI. It creates,
checks, tests, packs, locks, installs, verifies, and projects capability
packages. It is package infrastructure, not a Codex plugin or a business
logic runtime.

## Build and invoke

```bash
cargo build -p elegy-tooling
cargo run -p elegy-tooling --bin elegy -- init --name my-tool --output ./my-tool
cargo run -p elegy-tooling --bin elegy -- check --package ./my-tool
cargo run -p elegy-tooling --bin elegy -- pack --package ./my-tool --output ./dist/my-tool.zip
```

`pack` materializes per-file SHA-256 values and emits a deterministic
`<archive>.sbom.json` sidecar. `lock create`, `install`, `verify`, `project`,
`install --update`, and `uninstall` keep an agent setup pinned to a reviewed
archive, explicit entrypoint executable digests, and installer receipt.

The scaffold and source tests prove contract behavior only. There is not yet a
reviewed clean independent-repository install or non-fixture agent task
receipt; see [readiness.json](readiness.json).
