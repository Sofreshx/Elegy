# Elegy

[![CI](https://github.com/Sofreshx/Elegy/actions/workflows/rust-ci.yml/badge.svg)](https://github.com/Sofreshx/Elegy/actions/workflows/rust-ci.yml)
[![Latest release](https://img.shields.io/github/v/release/Sofreshx/Elegy?display_name=tag&sort=semver)](https://github.com/Sofreshx/Elegy/releases/latest)
[![License](https://img.shields.io/badge/license-Apache--2.0-blue.svg)](LICENSE)

Elegy is a Rust toolkit for shipping governed local CLI capabilities to AI-agent
hosts. Governed artifacts stay in the repo. Dedicated `elegy-*` binaries expose
the executable surfaces. CLI invocation is the default integration boundary.
MCP is optional.

## Current readiness

Elegy currently exposes **no agent-routable surface by default**. Source behavior exists for many packages, but none has yet earned `usable` through both a clean packaged installation and a non-fixture end-to-end task. See the generated [ecosystem readiness matrix](docs/readiness.md) before installing or invoking anything.

`implemented` means source behavior and checks exist. It does not mean usable, shipped, or production-ready. The default marketplace is therefore intentionally empty.

Automation Packs remain a separate delivery lane. Their cross-repository
relationship to Elegy is governed by the
[Automation Program ecosystem decision](https://github.com/Sofreshx/elegy-automation-program/blob/main/docs/adr/2026-07-21-automation-ecosystem-governance.md).

Core model:

- plugins are reusable adapters to data sources, databases, platforms, APIs,
  local systems, or executable/CLI boundaries
- domain and business logic stays in libraries, applications, Automation Packs,
  or product-local commands
- Rust implements reusable behavior over those artifacts
- `SKILL.md` files are the skill discovery authority
- dedicated `elegy-*` binaries are implementation surfaces whose readiness is
  proven separately
- `elegy-run` is the MCP host adapter

## Repository Model

| Area | Purpose |
| --- | --- |
| `plugins/` | Historical runtime root containing one active adapter plus tools and adapter candidates; path is not classification. |
| `tools/` | Standalone CLI crates that are not plugin packages. |
| `hosts/` | Host adapters and transport servers. |
| `skills/` | Standalone skill-only packages. |
| `marketplace-wrappers/` | Historical or blocked external integration metadata; not default plugin discovery. |
| `shared/` | Reusable Rust libraries and platform tooling. |
| `distribution/` | Canonical release and surface catalog. |
| `docs/` | Architecture, ADRs, specs, governance, and operations docs. |
| `artifacts/` | CI-generated bundles, archives, and validation outputs. |

When those surfaces disagree, prefer the smallest relevant architecture or spec
document under `docs/`, then the owning package manifest and
`distribution/surfaces.json`.

## Maintainer use from source

There is no generally recommended Elegy installation today. Maintainers can
inspect and exercise implemented surfaces from a source checkout:

```bash
git clone https://github.com/Sofreshx/Elegy.git
cd Elegy
cargo build
cargo run -p elegy-tooling --bin elegy-plugin-packaging -- verify --plugin plugins/accounts
cargo run -p elegy-accounts -- --help
```

These commands are source verification, not clean-install or real-task receipts.

Read first: [CONTRIBUTING.md](CONTRIBUTING.md), [SECURITY.md](SECURITY.md),
[docs/architecture/README.md](docs/architecture/README.md).

## Implemented source binaries

This list says what can be built from the workspace, not what is agent-routable.
The [readiness matrix](docs/readiness.md) is authoritative for current claims.

| Binary | Crate | Per-feature note |
| --- | --- | --- |
| `elegy-run` | `hosts/host-mcp/` | [DISTRIBUTION.md](hosts/host-mcp/DISTRIBUTION.md) |
| `elegy-contracts` | `shared/core/` | _No dedicated distribution note yet_ |
| `elegy-plugin-packaging` | `shared/tooling/` | [docs/distribution.md](docs/distribution.md) |
| `elegy-desktop` | `plugins/desktop/` | [DISTRIBUTION.md](plugins/desktop/DISTRIBUTION.md) |
| `elegy-observe` | `plugins/observe/` | [DISTRIBUTION.md](plugins/observe/DISTRIBUTION.md) |
| `elegy-memory` | `plugins/memory/` | [DISTRIBUTION.md](plugins/memory/DISTRIBUTION.md) |
| `elegy-mcp` | `plugins/mcp/` | [DISTRIBUTION.md](plugins/mcp/DISTRIBUTION.md) |
| `elegy-planning` | `plugins/planning/` | [DISTRIBUTION.md](plugins/planning/DISTRIBUTION.md) |
| `elegy-configuration` | `tools/configuration/` | [DISTRIBUTION.md](tools/configuration/DISTRIBUTION.md) |
| `elegy-documentation` | `plugins/documentation/` | [DISTRIBUTION.md](plugins/documentation/DISTRIBUTION.md) |
| `elegy-memory-mcp-stdio` | `hosts/memory-mcp/` | [DISTRIBUTION.md](hosts/memory-mcp/DISTRIBUTION.md) |
| `elegy-memory-mcp-http` | `hosts/memory-mcp/` | [DISTRIBUTION.md](hosts/memory-mcp/DISTRIBUTION.md) |
| `elegy-codegraph` | `tools/codegraph/` | [DISTRIBUTION.md](tools/codegraph/DISTRIBUTION.md) |

## Skill Surfaces

An adapter may bundle optional workflow guidance under
`plugins/{name}/skills/`. Standalone guidance lives under
`skills/elegy-{skill-id}/SKILL.md` and must be installed through the target
host's normal skill lane. A skill is not executable discovery authority and
does not enter the Elegy marketplace by itself.

## Configuration Materialization

`elegy-configuration` materializes and verifies governed repo and home assets
from plugin-owned templates and profiles.

```bash
elegy-configuration list --json
elegy-configuration apply --profile-id repo-opencode-minimal --target . --dry-run --json
```

See [docs/architecture/README.md](docs/architecture/README.md) for built-in
templates and profile details.

## Skill Validation

Each active adapter owns any optional skills it bundles. Validate the entire
adapter package, including its typed capability catalog, with:

`elegy-plugin-packaging verify --plugin plugins/accounts`

Skill authoring and body audits belong to the `elegy-skill-authoring` skill;
they are not mediated by a central runtime registry.

## Plugins

An Elegy plugin is a reusable system adapter, not a synonym for any package of
business logic. Client Radar, AI Radar, and Question Studio are intentionally
not Elegy plugins; they belong in product libraries, tools, or applications.

`elegy-plugin/v2` declares identity, connection posture, and readiness authority
in `.elegy-plugin/plugin.json`. The manifest and schema prove structure only.
They do not prove installed behavior.

Setup flow:

```bash
elegy-plugin-packaging verify --plugin ./my-plugin
```

Release configuration uses `distribution/surfaces.json` as the central release catalog.
Only active `adapter-plugin` entries with a typed capability catalog may set
`packaging: plugin`.

The generated default marketplace lives at `.elegy/marketplace.json` and
contains only validated `usable` or `production` surfaces. It is currently
empty. Maintainers can inspect the project catalog explicitly with
`--include-incubating` without rewriting that default; install or export then
requires `--allow-incubating`.

```bash
elegy-plugin-packaging marketplace list --source . --json
elegy-plugin-packaging marketplace list --source . --include-incubating --json
```

Boundaries: the plugin manifest is a metadata envelope, not a runtime,
marketplace, auth store, approval record, or secret/session container. Hosts own
install, auth, approvals, runtime sessions, and execution policy.

## Optional MCP Projection

```bash
elegy-run
```

MCP is an optional projection over governed capabilities and CLI behavior. Side-effecting tools
stay blocked unless the host is started with `--allow-side-effects`.

## Documentation

- [Agent integration guide](docs/agent-integration.md)
- [Distribution index (thin)](docs/distribution.md) — per-binary notes live in
  each binary's `DISTRIBUTION.md`
- [Architecture index](docs/architecture/README.md)
- [Evidence-backed readiness](docs/readiness.md)
- [Deprecations and reclassification](docs/deprecations.md)
- [Ecosystem topology](docs/architecture/ecosystem-topology.md)
- [Substrate governance](docs/architecture/substrate-governance.md)
- [Contributing guide](CONTRIBUTING.md) | [Security policy](SECURITY.md)
- [Code of conduct](CODE_OF_CONDUCT.md)

## Contributing From Source

```bash
cargo build
cargo test --workspace --all-targets --all-features
```

When touching governed artifacts, packaging, or release workflows, also use the
repo-root validation commands below.

For documentation-only changes, prefer the dedicated documentation checker:

```bash
elegy-documentation check --project . --json
```

## Development

Common Rust checks:

```bash
cargo fmt --all --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-targets --all-features
```

Repo-root validation for governed artifacts and packaging:

```bash
cargo run -p elegy-core --bin elegy-contracts -- --project . contracts validate
cargo run -p elegy-documentation -- check --project .
```

## License

Elegy is licensed under [Apache 2.0](LICENSE).
