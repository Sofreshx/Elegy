# Elegy

[![CI](https://github.com/Sofreshx/Elegy/actions/workflows/rust-ci.yml/badge.svg)](https://github.com/Sofreshx/Elegy/actions/workflows/rust-ci.yml)
[![Latest release](https://img.shields.io/github/v/release/Sofreshx/Elegy?display_name=tag&sort=semver)](https://github.com/Sofreshx/Elegy/releases/latest)
[![License](https://img.shields.io/badge/license-Apache--2.0-blue.svg)](LICENSE)

Elegy is a Rust toolkit for shipping governed local CLI capabilities to AI-agent
hosts. Governed artifacts stay in the repo. Dedicated `elegy-*` binaries expose
the executable surfaces. CLI invocation is the default integration boundary.
MCP is optional.

Core model:

- governed plugin artifacts stay co-located with owning bundled plugins
- Rust implements reusable behavior over those artifacts
- `SKILL.md` files are the skill discovery authority
- dedicated `elegy-*` binaries are the shipped surfaces
- `elegy-run` is the MCP host adapter

## Repository Model

| Area | Purpose |
| --- | --- |
| `plugins/` | Bundled installable plugin packages. |
| `tools/` | Standalone CLI crates that are not plugin packages. |
| `hosts/` | Host adapters and transport servers. |
| `skills/` | Standalone skill-only packages. |
| `marketplace-wrappers/` | Public metadata wrappers for external/private plugin archives. |
| `shared/` | Reusable Rust libraries and platform tooling. |
| `distribution/` | Canonical release and surface catalog. |
| `docs/` | Architecture, ADRs, specs, governance, and operations docs. |
| `artifacts/` | CI-generated bundles, archives, and validation outputs. |

When those surfaces disagree, prefer the smallest relevant architecture or spec
document under `docs/`, then the owning package manifest and
`distribution/surfaces.json`.

## Install

Latest stable release: [github.com/Sofreshx/Elegy/releases/latest](https://github.com/Sofreshx/Elegy/releases/latest)

Rolling prerelease from `main`: [github.com/Sofreshx/Elegy/releases/tag/main-snapshot](https://github.com/Sofreshx/Elegy/releases/tag/main-snapshot)

Published targets:

- `x86_64-pc-windows-msvc`
- `x86_64-unknown-linux-gnu`
- `aarch64-apple-darwin`

Plugin-packaged surfaces ship as portable release archives named
`<surface>-plugin-<target>.zip`. Skill-only packages use
`<surface>-plugin-any.zip`. Use `elegy-plugin-packaging` to install or export
them.

```bash
elegy-plugin-packaging install --archive elegy-planning-plugin-x86_64-pc-windows-msvc.zip
elegy-plugin-packaging export --plugin plugins/planning --host codex --output ./export
```

Codex-native consumers should use the generated marketplace projection asset
named `elegy-codex-marketplace-<target>.zip`. Extract it to a Codex marketplace
directory, register that marketplace, then install the selected plugin:

```bash
codex plugin marketplace add <CODEX_HOME>/marketplaces/elegy --json
codex plugin add elegy-planning@elegy --json
```

Non-plugin surfaces ship as standalone binaries. See
[docs/distribution.md](docs/distribution.md) for the release index and each
binary's `DISTRIBUTION.md` for install details.

### From source

```bash
git clone https://github.com/Sofreshx/Elegy.git
cd Elegy
cargo build
cargo run -p elegy-tooling --bin elegy-plugin-packaging -- verify --plugin plugins/planning
cargo run -p elegy-planning -- --json version
```

Read first: [CONTRIBUTING.md](CONTRIBUTING.md), [SECURITY.md](SECURITY.md),
[docs/architecture/README.md](docs/architecture/README.md).

## Shipped Binaries

Each binary owns its own distribution note. Adding a new binary does not
require editing this README.

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
| `elegy-skills` | `tools/skills/` | [DISTRIBUTION.md](tools/skills/DISTRIBUTION.md) |
| `elegy-configuration` | `tools/configuration/` | [DISTRIBUTION.md](tools/configuration/DISTRIBUTION.md) |
| `elegy-documentation` | `plugins/documentation/` | [DISTRIBUTION.md](plugins/documentation/DISTRIBUTION.md) |
| `elegy-memory-mcp-stdio` | `hosts/memory-mcp/` | [DISTRIBUTION.md](hosts/memory-mcp/DISTRIBUTION.md) |
| `elegy-memory-mcp-http` | `hosts/memory-mcp/` | [DISTRIBUTION.md](hosts/memory-mcp/DISTRIBUTION.md) |
| `elegy-codegraph` | `tools/codegraph/` | [DISTRIBUTION.md](tools/codegraph/DISTRIBUTION.md) |

## Skill Surfaces

Plugin-owned skills live under
`plugins/{plugin-name}/skills/elegy-{skill-id}/SKILL.md`. Standalone skill-only
packages live under `skills/elegy-{skill-id}/SKILL.md`. The `elegy-skills` CLI
discovers skills from plugin manifests and `skills/elegy-*` packages, failing
on duplicate skill IDs.

## Configuration Materialization

`elegy-configuration` materializes and verifies governed repo and home assets
from plugin-owned templates and profiles.

```bash
elegy-configuration list --json
elegy-configuration apply --profile-id repo-opencode-minimal --target . --dry-run --json
```

See [docs/architecture/README.md](docs/architecture/README.md) for built-in
templates and profile details.

## Skill Tools

Elegy's skills product is registry-first. Plugin-owned skills under
`plugins/{plugin-name}/skills/` and standalone packages under `skills/elegy-*`
are the discovery authority. The `elegy-skills` CLI provides search, resolve,
inspect, and validation.

`elegy-skills list/search/describe/resolve/validate --json`

## Plugins

`elegy-plugin/v1` is the minimal plugin manifest format for `.elegy-plugin/plugin.json`.
Plugins declare identity and Agent Skills (SKILL.md) in a single filesystem directory.
The `ElegyPluginV1` Rust type defines the contract. Generated JSON schemas under
`shared/plugin-sdk/schemas/` provide machine-readable projections.

Setup flow:

```bash
elegy-plugin-packaging verify --plugin ./my-plugin
```

Release configuration uses `distribution/surfaces.json` as the central release catalog.

The generated marketplace lives at `.elegy/marketplace.json`:

```bash
elegy-plugin-packaging marketplace list --source . --json
elegy-plugin-packaging marketplace search planning --source . --json
elegy-plugin-packaging marketplace status --source . --plugin elegy-planning --json
elegy-plugin-packaging marketplace update elegy-planning --source . --json
```

Boundaries: the plugin manifest is a metadata envelope, not a runtime,
marketplace, auth store, approval record, or secret/session container. Hosts own
install, auth, approvals, runtime sessions, and execution policy.

## Optional MCP Projection

```bash
elegy-run
```

MCP is an adapter over governed skills and CLI behavior. Side-effecting tools
stay blocked unless the host is started with `--allow-side-effects`.

## Documentation

- [Agent integration guide](docs/agent-integration.md)
- [Distribution index (thin)](docs/distribution.md) — per-binary notes live in
  each binary's `DISTRIBUTION.md`
- [Architecture index](docs/architecture/README.md)
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
