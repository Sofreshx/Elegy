# Elegy

[![CI](https://github.com/Sofreshx/Elegy/actions/workflows/rust-ci.yml/badge.svg)](https://github.com/Sofreshx/Elegy/actions/workflows/rust-ci.yml)
[![Latest release](https://img.shields.io/github/v/release/Sofreshx/Elegy?display_name=tag&sort=semver)](https://github.com/Sofreshx/Elegy/releases/latest)
[![License](https://img.shields.io/badge/license-Apache--2.0-blue.svg)](LICENSE)

Elegy is a Rust toolkit for shipping governed local CLI capabilities to AI-agent
hosts. It keeps contracts and discovery metadata durable, exposes installable
binaries through GitHub Releases, and uses CLI invocation templates as the
default execution boundary.

Core model:

- governed artifacts are the durable authority
- Rust implements reusable executable behavior over those artifacts
- skill definitions are the discovery authority for agent capabilities
- CLI invocation templates are the default execution boundary
- generated mirrors, wrapper roots, Codex plugin projections, and MCP tool
  lists are derived adapter surfaces
- MCP is an optional projection for MCP-native clients

## Repository Model

| Area | Purpose |
| --- | --- |
| `contracts/` | Governed schemas, fixtures, manifests, package metadata, and discovery artifacts. |
| `governance/`, `schemas/`, `policies/` | Version, inventory, schema-line, boundary, and formalization policy. |
| `rust/` | First-party Rust libraries and binaries that consume governed artifacts. |
| `src/Elegy-*` | Contributor-navigation and wrapper-package overlays, not implementation roots. |
| `.agents/skills/**`, `.github/skills/**` | Rendered skill mirrors for host and contributor routing. |
| `artifacts/` | Generated bundles, archives, and validation outputs. |

When those surfaces disagree, prefer the governed artifact roots and the
smallest relevant architecture or spec document under `docs/`.

## Install

Latest stable release: [github.com/Sofreshx/Elegy/releases/latest](https://github.com/Sofreshx/Elegy/releases/latest)

Rolling prerelease from `main`: [github.com/Sofreshx/Elegy/releases/tag/main-snapshot](https://github.com/Sofreshx/Elegy/releases/tag/main-snapshot)

See [docs/distribution.md](docs/distribution.md) for the full package matrix
and asset family descriptions.

Published targets:

- Windows x64: `x86_64-pc-windows-msvc`
- Linux x64: `x86_64-unknown-linux-gnu`
- macOS ARM64: `aarch64-apple-darwin`

### PowerShell installer

Download `elegy-installer-<bundleVersion>.zip` from GitHub Releases, extract,
and run the bundled `install-distribution.ps1`:

```powershell
pwsh ./install-distribution.ps1 -Destination ./tools/elegy -CliSurfaces elegy-cli -Force
```

Pin a specific release:

```powershell
pwsh ./install-distribution.ps1 -Tag vX.Y.Z -Destination ./tools/elegy -CliSurfaces elegy-cli,elegy-mcp,elegy-planning -Force
```

Track the rolling `main-snapshot` prerelease:

```powershell
pwsh ./install-distribution.ps1 -Tag main-snapshot -Destination ./tools/elegy-main -CliSurfaces elegy-cli -Force
```

The same installer is also available at `scripts/install-distribution.ps1` from
a repo checkout.

### Bash installer

On Linux or macOS, use the Bash installer from a repo checkout:

```bash
bash ./scripts/install-distribution.sh -Tag vX.Y.Z -Destination ./tools/elegy -CliSurfaces elegy-cli -Force
```

### Installed layout

- `contracts/` - extracted governed contracts bundle
- `bin/<surface>/` - installed CLI binaries
- `wrappers/<surface>/` - installed wrapper surfaces when requested
- `install-receipt.json` - verification evidence and installed asset metadata

### From source

```bash
git clone https://github.com/Sofreshx/Elegy.git
cd Elegy/rust
cargo build -p elegy-cli
cargo run -p elegy-cli -- --version --json
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

From a repo checkout, use `cargo run -p elegy-cli -- ...` with the same
arguments.

## Main Surfaces

| Surface | Purpose |
| --- | --- |
| `elegy` | General-purpose CLI: agent onboarding, skills, contracts, configuration, docs, mermaid, diagram, observe, desktop, repo, web, data, notify, MCP host, and lower-level author/analyze/generate/validate/inspect. |
| `elegy-memory` | Dedicated local memory CLI. |
| `elegy-mcp` | Dedicated MCP descriptor authoring and analysis CLI. |
| `elegy-planning` | Dedicated durable planning CLI for goals, roadmaps, plans, and todos. |
| `elegy-skills` | Dedicated governed skill registry CLI. |
| `elegy-configuration` | Dedicated deterministic configuration materialization CLI. |
| `elegy-documentation` | Dedicated documentation authority CLI. |

## Wrapper and Skill Surfaces

Wrapper roots under `src/Elegy-*` package bounded handoff surfaces for
downstream repositories. They are not authority roots and they do not replace
the Rust crates or governed JSON contracts.

Most wrappers delegate to dedicated `elegy-*` Rust binaries. The current
`elegy-obsidian` wrapper is different: it wraps the official Obsidian Desktop
CLI and keeps Obsidian vault content non-authoritative. Durable planning state
continues to live in `elegy-planning` and SQLite.

Rendered `SKILL.md` files under `.agents/skills/**`, `.github/skills/**`, and
wrapper-local `skills/**` directories are routing mirrors. The governed
`contracts/fixtures/skill.*.json` files remain the skill authority.

## Configuration Materialization

The umbrella CLI and dedicated `elegy-configuration` binary support
deterministic materialization and drift verification of agent-facing repo and
home assets from governed templates and profiles.

```bash
elegy configuration list --json
elegy configuration apply --profile-id repo-opencode-minimal --target . --dry-run --json
elegy-configuration apply --package ./contracts/fixtures/elegy-plugin-package.demo-config.json --profile-id demo-profile --target . --dry-run --json
```

See [docs/architecture/README.md](docs/architecture/README.md) for built-in
templates and profile details.

## Skill Tools

Elegy's skills product is registry-first. Governed skill definitions under
`contracts/fixtures/skill.*.json` are the discovery authority. The `elegy-skills`
CLI provides search, resolve, inspect, and validation. The umbrella `elegy skills ...`
surface mirrors this for convenience.

```bash
elegy-skills list --json
elegy-skills search --query "repo status" --json
elegy-skills validate --file ./contracts/fixtures/skill.elegy-repo.json --json
```

## Plugin Packages

`elegy-plugin-package/v1` is the portable package contract for bundling
governed skill definitions, capability projections, tool requirements, and
publishing metadata into a single host-facing surface. Plugin packages are the
primary setup path for bringing governed capabilities to LLM hosts.

Setup flow:

```bash
elegy plugin new --template cli-tool --output ./my-plugin
# edit ./my-plugin/elegy-plugin-package.json
elegy plugin verify --package ./my-plugin/elegy-plugin-package.json --json
elegy plugin install-check --package ./my-plugin/elegy-plugin-package.json --install-receipt ./tools/elegy/install-receipt.json --json
```

`elegy plugin verify` checks package consistency against referenced skill
definitions, capability projections, side-effect classes, and subset
declarations. `elegy plugin install-check` checks declared tool requirements
against an install receipt and optional binary probes. Both commands emit a
readiness receipt (`ready` | `partial` | `blocked`) governed by
`contracts/schemas/elegy-plugin-readiness-v1.schema.json`. The receipt is the
machine-readable answer to "what can this package actually do on this host right
now?"

Authority schemas:

- `contracts/schemas/elegy-plugin-package.schema.json` — package contract
- `contracts/schemas/elegy-plugin.lock.json` — pinned contract bundle version
- `contracts/schemas/elegy-plugin-readiness-v1.schema.json` — readiness receipt

Boundaries: the package is a portable contract bundle, not a runtime,
marketplace, auth store, approval record, or secret/session container. Hosts own
install, auth, approvals, runtime sessions, and execution policy.

See the [Elegy Plugin Package Model](docs/architecture/elegy-plugin-package-model.md)
for the full model, and the [Plugin Tool Availability spec](docs/specs/plugin-tool-availability.md)
for the verify-only contract rules.

Codex plugin projection (`elegy generate codex-plugin`) is one optional derived
projection target, not the main plugin setup path.

## Optional MCP Projection

```bash
elegy run --profile ./tools/elegy-profile.json
```

MCP is an adapter over governed skills and CLI behavior. Side-effecting tools
stay blocked unless the host passes `--allow-side-effects`. Prefer `--dry-run`
for one-off invocations.

## Documentation

- [Agent integration guide](docs/agent-integration.md)
- [Distribution and downstream consumption](docs/distribution.md)
- [Architecture index](docs/architecture/README.md)
- [Ecosystem topology](docs/architecture/ecosystem-topology.md)
- [Substrate governance](docs/architecture/substrate-governance.md)
- [Contributing guide](CONTRIBUTING.md) | [Security policy](SECURITY.md)
- [Code of conduct](CODE_OF_CONDUCT.md) | [Changelog](CHANGELOG.md)

## Contributing From Source

```bash
cd rust
cargo build -p elegy-cli
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
cd rust
cargo fmt --all --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-targets --all-features
```

Repo-root validation for governed artifacts and packaging:

```powershell
pwsh ./scripts/validate-package-boundaries.ps1
pwsh ./scripts/export-contracts.ps1
pwsh ./scripts/validate-canonical-outputs.ps1 -RequireGeneratedOutputs
```

## License

Elegy is licensed under [Apache 2.0](LICENSE).
