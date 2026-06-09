# Elegy

[![CI](https://github.com/Sofreshx/Elegy/actions/workflows/rust-ci.yml/badge.svg)](https://github.com/Sofreshx/Elegy/actions/workflows/rust-ci.yml)
[![Latest release](https://img.shields.io/github/v/release/Sofreshx/Elegy?display_name=tag&sort=semver)](https://github.com/Sofreshx/Elegy/releases/latest)
[![License](https://img.shields.io/badge/license-Apache--2.0-blue.svg)](LICENSE)

Elegy is a Rust toolkit for shipping governed local CLI capabilities to AI-agent
hosts. It keeps contracts and discovery metadata durable, exposes installable
binaries through GitHub Releases, and uses CLI invocation templates as the
default execution boundary.

Core model:

- governed contracts are the durable authority
- skill definitions are the discovery authority
- CLI invocation templates are the default execution boundary
- MCP is an optional projection for MCP-native clients

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
pwsh ./install-distribution.ps1 -Tag v1.4.0 -Destination ./tools/elegy -CliSurfaces elegy-cli,elegy-mcp,elegy-planning -Force
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
bash ./scripts/install-distribution.sh -Tag v1.4.0 -Destination ./tools/elegy -CliSurfaces elegy-cli -Force
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
