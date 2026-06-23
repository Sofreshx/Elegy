# `elegy-documentation` — distribution

## What this binary does

Dedicated CLI for the authority-aware documentation inspector. Provides
`inspect | map | check | new (adr|spec|note)` and `index` flows over the
Elegy documentation doctrine. Validates frontmatter, freshness, ADR/spec
classification, and the `docs/docs-index.md` content bundle shape.

`elegy docs ...` (the umbrella CLI) is the compatibility path; the dedicated
binary is the authoritative lane for documentation work.

## Binary surface

- **Crate:** `rust/features/elegy-documentation/`
- **Binary name:** `elegy-documentation`
- **Source:** `rust/features/elegy-documentation/src/main.rs`
- **Library consumers:** `rust/bin/elegy-cli` (umbrella `elegy docs`
  subcommands), Rust CI (the `docs-check-v2` job in
  `.github/workflows/rust-ci.yml`).

## Distribution shape

- **CLI archive asset family:** `elegy-documentation-<cliVersion>-<target>.zip`
- **Wrapper archive:** `elegy-documentation-wrapper-<bundleVersion>.zip`
- **Versioning:** follows workspace `version`.

## Install

```bash
# Canonical installer (recommended)
bash ./scripts/install-distribution.sh -Tag vX.Y.Z -Destination ./tools/elegy -CliSurfaces elegy-documentation -Force
```

```powershell
# Native-pwsh entry point: thin shim that forwards all args to bash (requires bash in PATH)
pwsh ./scripts/install-distribution.ps1 -Tag vX.Y.Z -Destination ./tools/elegy -CliSurfaces elegy-documentation -Force
```

## Build from source

```bash
cd rust
cargo build -p elegy-documentation
cargo run -p elegy-documentation -- check --project .. --json
```

## Validation

- `cargo test -p elegy-documentation`
- The CI `docs-check-v2` job in `.github/workflows/rust-ci.yml` runs
  `cargo run -p elegy-documentation -- check --project ..` on every PR.

## Where to read more

- Documentation practices doctrine (the central ADR/spec classification rules):
  [`docs/architecture/documentation-practices.md`](../../../../docs/architecture/documentation-practices.md)
- ADR/spec CLI contract:
  [`docs/specs/documentation-practices-skill-and-cli.md`](../../../../docs/specs/documentation-practices-skill-and-cli.md)
- Docs YAML configuration: [`.elegy/docs.yaml`](../../../../.elegy/docs.yaml)
