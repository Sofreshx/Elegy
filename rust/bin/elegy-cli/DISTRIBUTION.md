# `elegy` CLI — distribution

## What this binary does

The umbrella `elegy` binary is the general-purpose bootstrap CLI. It dispatches
agent onboarding, skill compatibility, docs tooling, Mermaid and diagram
tooling, repo/web/data/notify utilities, read-only observation and desktop
automation, optional MCP hosting, contracts export, deterministic
configuration materialization, and lower-level `author|analyze|generate|validate|inspect`
commands.

Lower-level `elegy plugin new` / `elegy plugin verify` / `elegy plugin install-check`
flow into the plugin package tooling shared with `rust/core/elegy-tooling`.

## Binary surface

- **Crate:** `rust/bin/elegy-cli/`
- **Binary name:** `elegy`
- **Source:** `rust/bin/elegy-cli/src/main.rs`
- **Library consumers:** none — this is a host entrypoint binary

## Distribution shape

- **CLI archive asset family:** `elegy-cli-<cliVersion>-<target>.zip`
- **Versioning:** the umbrella CLI follows workspace `version` (currently `0.1.0`).
  Bundle and CLI versions are independent; see [`docs/distribution.md`](../../../../docs/distribution.md)
  for the release channel contract.
- **Wrapper archive:** `elegy-cli-wrapper-<bundleVersion>.zip` (when applicable).
- **Plugin package:** `elegy-cli` does not currently have a dedicated
  plugin manifest; the umbrella surface is documented by
  this `DISTRIBUTION.md` and the `distribution/surfaces.json`
  catalog.

## Install

```bash
# Canonical installer (recommended)
bash ./scripts/install-distribution.sh -Tag vX.Y.Z -Destination ./tools/elegy -CliSurfaces elegy-cli -Force
```

```powershell
# Native-pwsh entry point: thin shim that forwards all args to bash (requires bash in PATH)
pwsh ./scripts/install-distribution.ps1 -Tag vX.Y.Z -Destination ./tools/elegy -CliSurfaces elegy-cli -Force
```

## Build from source

```bash
cd rust
cargo build -p elegy-cli
cargo run -p elegy-cli -- --version --json
```

## Validation

- `cargo test -p elegy-cli` for the umbrella command surface
- `cargo clippy -p elegy-cli -- -D warnings`
- For governed artifact changes, run `cargo run -p elegy-cli -- contracts export --output ../artifacts/contracts --create-archive --archive-output ../artifacts/distribution/elegy-contracts-bundle.zip` from the `rust/` directory first
  so the umbrella CLI can resolve the contract bundle from
  `artifacts/contracts/`.

## Where to read more

- Umbrella command surface and subcommand model: `rust/bin/elegy-cli/src/main.rs`
- Per-feature subcommands are documented in each feature's
  `rust/features/<feature>/DISTRIBUTION.md`
- Architecture index: [`docs/architecture/README.md`](../../../../docs/architecture/README.md)
