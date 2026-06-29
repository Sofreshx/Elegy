# Elegy

[![CI](https://github.com/Sofreshx/Elegy/actions/workflows/rust-ci.yml/badge.svg)](https://github.com/Sofreshx/Elegy/actions/workflows/rust-ci.yml)
[![Latest release](https://img.shields.io/github/v/release/Sofreshx/Elegy?display_name=tag&sort=semver)](https://github.com/Sofreshx/Elegy/releases/latest)
[![License](https://img.shields.io/badge/license-Apache--2.0-blue.svg)](LICENSE)

Elegy is a Rust toolkit for shipping governed local CLI capabilities to AI-agent
hosts. It keeps contracts and discovery metadata durable, exposes installable
binaries through GitHub Releases, and uses CLI invocation templates as the
default execution boundary.

Core model:

- governed artifacts are co-located with owning plugins
- Rust implements reusable executable behavior over those artifacts
- skill definitions (SKILL.md) are the discovery authority for agent capabilities
- CLI invocation is the default execution boundary
- MCP is an optional adapter for MCP-native clients

## Repository Model

| Area | Purpose |
| --- | --- |
| `docs/governance/` | Operational policy (workflow/environment/branch enforcement modes). |
| `hosts/` | Thin CLI entrypoints and umbrella host crates. |
| `plugins/` | Self-contained capability crates — each owns its schemas, fixtures, and configuration inside the plugin directory. |
| `shared/` | Cross-cutting library crates (e.g. `shared/core/` holds shared types). |
| `plugins/<name>/skills/` | Plugin-owned skill definitions. |
| `artifacts/` | CI-generated bundles, archives, and validation outputs. |

When those surfaces disagree, prefer the governed artifacts within their owning
plugin and the smallest relevant architecture or spec document under `docs/`.

## Install

Latest stable release: [github.com/Sofreshx/Elegy/releases/latest](https://github.com/Sofreshx/Elegy/releases/latest)

Rolling prerelease from `main`: [github.com/Sofreshx/Elegy/releases/tag/main-snapshot](https://github.com/Sofreshx/Elegy/releases/tag/main-snapshot)

Per-binary install commands and asset family names live in each binary's
per-feature distribution note. The top-level [docs/distribution.md](docs/distribution.md)
is a thin index: release channels, published targets, asset family patterns,
and a per-binary link list.

Published targets:

- Windows x64: `x86_64-pc-windows-msvc`
- Linux x64: `x86_64-unknown-linux-gnu`
- macOS ARM64: `aarch64-apple-darwin`

### Install

Plugin surfaces ship as portable plugin archives (`.plugin.zip`). The `elegy-plugin-packaging` CLI
verifies, packs, exports, and installs plugin packages.

```bash
# Install a plugin from a local archive
elegy-plugin-packaging install --archive elegy-planning-v0.1.0.plugin.zip

# Export for a specific host
elegy-plugin-packaging export --plugin plugins/planning --host codex --output ./export
```

Non-plugin surfaces ship as standalone CLI binaries. See [docs/distribution.md](docs/distribution.md)
for the per-binary install guide.

### Installed layout

- `bin/<surface>/` — installed CLI binaries
- `bundle/` — assembled governed artifacts from plugin directories
- `install-receipt.json` — verification evidence and installed asset metadata

### From source

```bash
git clone https://github.com/Sofreshx/Elegy.git
cd Elegy
cargo build
# Verify a plugin package
cargo run -p elegy-tooling -- verify --plugin plugins/planning
cargo run -p elegy-planning -- --version --json
```

Read first: [CONTRIBUTING.md](CONTRIBUTING.md), [SECURITY.md](SECURITY.md),
[docs/architecture/README.md](docs/architecture/README.md).

## Quick Start

After installing a release asset:

```bash
elegy agent check --json
elegy agent discover --query "repo status" --json
elegy repo status --json
elegy docs check --json
```

From a repo checkout, use `cargo run -p <crate> -- ...` with the same
arguments, using the appropriate dedicated binary crate.

## Per-binary surface

Each binary owns its own distribution note. Adding a new binary does not
require editing this README.

| Binary | Crate | Per-feature note |
| --- | --- | --- |
| `elegy-run` | `hosts/host-mcp/` | [DISTRIBUTION.md](hosts/host-mcp/DISTRIBUTION.md) |
| `elegy-contracts` | `shared/core/` | _No dedicated distribution note yet_ |
| `elegy-desktop` | `plugins/desktop/` | [DISTRIBUTION.md](plugins/desktop/DISTRIBUTION.md) |
| `elegy-observe` | `plugins/observe/` | [DISTRIBUTION.md](plugins/observe/DISTRIBUTION.md) |
| `elegy-memory` | `plugins/memory/` | [DISTRIBUTION.md](plugins/memory/DISTRIBUTION.md) |
| `elegy-mcp` | `plugins/mcp/` | [DISTRIBUTION.md](plugins/mcp/DISTRIBUTION.md) |
| `elegy-planning` | `plugins/planning/` | [DISTRIBUTION.md](plugins/planning/DISTRIBUTION.md) |
| `elegy-skills` | `plugins/skills/` | [DISTRIBUTION.md](plugins/skills/DISTRIBUTION.md) |
| `elegy-configuration` | `plugins/configuration/` | [DISTRIBUTION.md](plugins/configuration/DISTRIBUTION.md) |
| `elegy-documentation` | `plugins/documentation/` | [DISTRIBUTION.md](plugins/documentation/DISTRIBUTION.md) |
| `elegy-memory-mcp-stdio` | `plugins/memory-mcp/` | [DISTRIBUTION.md](plugins/memory-mcp/DISTRIBUTION.md) |
| `elegy-memory-mcp-http` | `plugins/memory-mcp/` | [DISTRIBUTION.md](plugins/memory-mcp/DISTRIBUTION.md) |
| `elegy-codegraph` | `plugins/codegraph/` | [DISTRIBUTION.md](plugins/codegraph/DISTRIBUTION.md) |

## Skill Surfaces

Elegy ships dedicated `elegy-*` Rust binaries for each capability surface.

Plugin-owned skills live under `plugins/<name>/skills/elegy-<name>/SKILL.md`.
Standalone skill-only packages live at the repo root (`elegy-<name>/SKILL.md`).
The `elegy-skills` CLI discovers skills from plugin manifests first, then standalone
root packages, failing on duplicate skill IDs.

## Configuration Materialization

Dedicated binaries (e.g., `elegy-configuration`) support
deterministic materialization and drift verification of agent-facing repo and
home assets from governed templates and profiles.

```bash
elegy configuration list --json
elegy configuration apply --profile-id repo-opencode-minimal --target . --dry-run --json
elegy-configuration apply --profile-id demo-profile --target . --dry-run --json
```

See [docs/architecture/README.md](docs/architecture/README.md) for built-in
templates and profile details.

## Skill Tools

Elegy's skills product is registry-first. Plugin-owned skills under
`plugins/<name>/skills/` and standalone root packages are the discovery authority.
The `elegy-skills` CLI provides search, resolve, inspect, and validation.

```bash
elegy-skills list --json
elegy-skills search --query "repo status" --json
elegy-skills describe --skill-id elegy-repo --json
```

## Plugins

`elegy-plugin/v1` is the minimal plugin manifest format for `.elegy-plugin/plugin.json`.
Plugins declare identity and Agent Skills (SKILL.md) in a single filesystem directory.
The `ElegyPluginV1` struct (a Rust type in the plugin infrastructure) defines the
in-code contract; there is no standalone JSON schema file.

Setup flow:

```bash
elegy-plugin-packaging verify --plugin ./my-plugin
```

Release configuration uses `distribution/surfaces.json` as the central release catalog.

Boundaries: the plugin manifest is a metadata envelope, not a runtime,
marketplace, auth store, approval record, or secret/session container. Hosts own
install, auth, approvals, runtime sessions, and execution policy.

## Optional MCP Projection

```bash
elegy run --profile ./tools/elegy-profile.json
```

MCP is an adapter over governed skills and CLI behavior. Side-effecting tools
stay blocked unless the host passes `--allow-side-effects`. Prefer `--dry-run`
for one-off invocations.

## Documentation

- [Agent integration guide](docs/agent-integration.md)
- [Distribution index (thin)](docs/distribution.md) — per-binary notes live in
  each binary's `DISTRIBUTION.md`
- [Architecture index](docs/architecture/README.md)
- [Ecosystem topology](docs/architecture/ecosystem-topology.md)
- [Substrate governance](docs/architecture/substrate-governance.md)
- [Contributing guide](CONTRIBUTING.md) | [Security policy](SECURITY.md)
- [Code of conduct](CODE_OF_CONDUCT.md) | [Changelog](CHANGELOG.md)

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
```

## License

Elegy is licensed under [Apache 2.0](LICENSE).
