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
- v2 skill definitions are the discovery authority
- CLI invocation templates are the default execution boundary
- MCP is an optional projection for MCP-native clients

## Why Elegy

- Keep agent-visible behavior explicit and versioned instead of hiding it
  behind ad hoc prompts or private host glue.
- Ship a general-purpose umbrella CLI plus dedicated binaries for the domains
  that need their own operator surfaces.
- Publish release assets, a machine-readable manifest, and SHA-256 checksums on
  GitHub Releases.
- Let downstream hosts install from release assets without depending on sibling
  checkouts or package feeds.

## Install

Latest stable release: [github.com/Sofreshx/Elegy/releases/latest](https://github.com/Sofreshx/Elegy/releases/latest)

Rolling prerelease from `main`: [github.com/Sofreshx/Elegy/releases/tag/main-snapshot](https://github.com/Sofreshx/Elegy/releases/tag/main-snapshot)

Distribution details: [docs/distribution.md](docs/distribution.md)

Published targets:

- Windows x64: `x86_64-pc-windows-msvc`
- Linux x64: `x86_64-unknown-linux-gnu`
- macOS ARM64: `aarch64-apple-darwin`

### For End Users

The installer bootstrap uses PowerShell 7+ and verifies downloaded assets
against the published `elegy-release-manifest-*.json` and
`elegy-release-checksums-*.json` files.

Most users should download `elegy-installer-<bundleVersion>.zip` from GitHub
Releases, extract it, and run the bundled `install-distribution.ps1`.

Install the latest stable release:

```powershell
# Run from the extracted elegy-installer archive directory.
pwsh ./install-distribution.ps1 -Destination ./tools/elegy -CliSurfaces elegy-cli -Force
```

Pin a specific release:

```powershell
pwsh ./install-distribution.ps1 -Tag v1.4.0 -Destination ./tools/elegy -CliSurfaces elegy-cli,elegy-mcp,elegy-planning,elegy-skills -Force
```

Track the rolling `main-snapshot` prerelease for latest-integration testing:

```powershell
pwsh ./install-distribution.ps1 -Tag main-snapshot -Destination ./tools/elegy-main -CliSurfaces elegy-cli -Force
```

Installed layout:

- `contracts/` - extracted governed contracts bundle
- `bin/<surface>/` - installed CLI binaries
- `wrappers/<surface>/` - installed wrapper surfaces when requested
- `install-receipt.json` - verification evidence and installed asset metadata

If you already have a repository checkout, the same installer is available at
`scripts/install-distribution.ps1`.

### Which Download To Use

| If you want... | Download |
| --- | --- |
| Simplest verified install path | `elegy-installer-<bundleVersion>.zip` |
| General-purpose `elegy` CLI | `elegy-cli-<cliVersion>-<target>.zip` |
| Contracts only | `elegy-contracts-<bundleVersion>.zip` |
| Dedicated memory CLI | `elegy-memory-<cliVersion>-<target>.zip` |
| Dedicated MCP CLI | `elegy-mcp-<cliVersion>-<target>.zip` |
| Dedicated planning CLI | `elegy-planning-<cliVersion>-<target>.zip` |
| Dedicated skill registry CLI | `elegy-skills-<cliVersion>-<target>.zip` |
| Wrapper surface for a dedicated tool family | `elegy-*-wrapper-<bundleVersion>.zip` |

Direct release asset families include:

- `elegy-cli-<cliVersion>-<target>.zip` - umbrella `elegy` binary
- `elegy-memory-<cliVersion>-<target>.zip` - dedicated local memory CLI
- `elegy-mcp-<cliVersion>-<target>.zip` - dedicated MCP CLI
- `elegy-planning-<cliVersion>-<target>.zip` - dedicated planning CLI
- `elegy-skills-<cliVersion>-<target>.zip` - dedicated skill registry CLI
- `elegy-contracts-<bundleVersion>.zip` - governed contracts bundle
- `elegy-installer-<bundleVersion>.zip` - installer bootstrap
- `elegy-*-wrapper-<bundleVersion>.zip` - dedicated wrapper surfaces

### For Contributors

If you want to build, test, or change Elegy itself, work from a repository
checkout instead of the installer archive.

```bash
git clone https://github.com/Sofreshx/Elegy.git
cd Elegy
cd rust
cargo build -p elegy-cli
cargo run -p elegy-cli -- --version --json
```

Read first:

- [CONTRIBUTING.md](CONTRIBUTING.md)
- [SECURITY.md](SECURITY.md)
- [docs/architecture/README.md](docs/architecture/README.md)

## Quick Start

After installing a release asset, try:

```bash
elegy agent check --json
elegy agent discover --query "repo status" --json
elegy repo status --json
elegy docs check --json
```

From a repo checkout, use the same flow through `cargo run`:

```bash
cd rust
cargo run -p elegy-cli -- agent check --json
cargo run -p elegy-cli -- agent discover --query "repo status" --json
cargo run -p elegy-cli -- repo status --json
```

## Agent Onboarding Flow

Host software should start with the `agent` surface instead of loading every
Elegy capability into context:

1. Validate the local setup:

   ```bash
   elegy agent check --json
   ```

2. Load the host integration packet:

   ```bash
   elegy agent manifest --json --profile ./tools/elegy-profile.json
   ```

3. Discover only what the task needs:

   ```bash
   elegy agent discover --query "memory search" --json --profile ./tools/elegy-profile.json
   elegy agent discover --query "memory search" --detail --json --profile ./tools/elegy-profile.json
   ```

4. Invoke the advertised CLI template from the selected capability. Hosts still
   enforce policy before running side-effecting capabilities.

Profiles are host-owned allowlists. They let a downstream app opt into a subset
of Elegy instead of exposing every built-in tool.

## Main Surfaces

| Surface | Purpose |
| --- | --- |
| `elegy agent manifest/check/discover` | Host onboarding, profile validation, and profile-filtered discovery. |
| `elegy skills list/search/resolve/get/capability/validate` | Umbrella compatibility surface over the built-in governed skill registry. |
| `elegy author/analyze/generate/validate/inspect` | Lower-level contributor tooling over governed artifacts and portable metadata. |
| `elegy contracts ...` | Export governed contract bundles and related metadata. |
| `elegy docs ...` | Repo-local ADR/spec scaffolding, objective docs validation, and docs index generation. |
| `elegy run` | Optional MCP stdio host over the same capability registry. |
| `elegy diagram ...` | Semantic diagram creation, mutation, explanation, and rendering. |
| `elegy mermaid ...` | Mermaid rendering, reverse projection, and narration. |
| `elegy observe ...` | Read-only OS/process/window/screen/clipboard/filesystem/system observation. |
| `elegy desktop ...` | Desktop automation with dry-run support for high-risk actions. |
| `elegy repo ...` | Read-only git status, diff, branch, and log inspection. |
| `elegy web ...` | Bounded HTTP fetch and reachability checks. |
| `elegy data ...` | JSON/YAML/TOML/CSV conversion, extraction, and schema validation. |
| `elegy notify ...` | Local toast and webhook notification helpers. |
| `elegy-memory` | Dedicated local memory CLI. |
| `elegy-planning` | Dedicated durable planning CLI for goals, roadmaps, plans, todos, issues, validation, and projections. |
| `elegy-mcp` | Dedicated MCP descriptor authoring and analysis CLI. |
| `elegy-skills` | Dedicated skill registry CLI with search, resolve, inspect, and built-in format validation. |

## Skill Tools

Elegy's skills product is registry-first:

- governed v2 skill definitions under `contracts/fixtures/skill-definition-v2.*.json` remain the discovery authority
- `elegy-skills` is the dedicated registry surface for searching, resolving, inspecting, and validating those governed skills
- `elegy skills ...` mirrors that functionality on the umbrella CLI as a compatibility surface
- Rust hosts can call the shared `rust/crates/elegy-skills` library directly for registry loading, profile filtering, search, resolve, capability inspection, and validation

Dedicated registry examples:

```bash
elegy-skills list --json
elegy-skills search --query "repo status" --json
elegy-skills resolve --query "repo status" --json
elegy-skills validate --file ./contracts/fixtures/skill-definition-v2.elegy-repo.json --json
```

## Capability Profiles

Profile schema: `contracts/schemas/agent-capability-profile.schema.json`.

Minimal example:

```json
{
  "schemaVersion": "agent-capability-profile/v1",
  "profileId": "generic-agent-host",
  "includeSkills": ["repo", "data"],
  "includeCapabilities": ["memory-search"],
  "excludeCapabilities": [],
  "alwaysIncludeRouter": true
}
```

Selection does not grant approval. A side-effecting capability selected by a
profile is visible and invokable only after the host applies its own policy.

## Optional MCP Projection

MCP is available for clients that prefer protocol tools:

```bash
elegy run --profile ./tools/elegy-profile.json
```

The same profile filters the MCP tool list. MCP should be treated as an adapter
over governed skills and CLI behavior, not as the primary Elegy integration
model.

## Project Status

- Elegy is public and installable today through GitHub Releases.
- The current authority roots are `contracts/`, `governance/`, `schemas/`,
  `policies/`, and the Rust workspace under `rust/`.
- Dedicated release assets and the release installer are supported surfaces for
  downstream use.
- The project is still hardening and consolidating around the current
  contracts-first and Rust-first shape.
- Downstream consumers should pin explicit semver releases; use
  `main-snapshot` for latest-integration validation, not as a long-term pinned
  contract.

## Documentation

- [Agent integration guide](docs/agent-integration.md)
- [Distribution and downstream consumption](docs/distribution.md)
- [Architecture index](docs/architecture/README.md)
- [Observe CLI guide](docs/architecture/observe-cli.md)
- [Contributing guide](CONTRIBUTING.md)
- [Security policy](SECURITY.md)
- [Code of conduct](CODE_OF_CONDUCT.md)
- [Changelog](CHANGELOG.md)

## Contributing From Source

Contributor workflow starts from a local checkout of the Rust workspace and the
repo-root validation scripts.

Typical local loop:

```bash
cd rust
cargo build -p elegy-cli
cargo run -p elegy-cli -- --version --json
cargo test --workspace --all-targets --all-features
```

When you touch governed artifacts, packaging, or release workflows, also use
the repo-root PowerShell validation commands in the next section.

## Development

Common Rust checks:

```bash
cd rust
cargo fmt --all --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-targets --all-features
```

Useful repo-root checks for governed artifacts and packaging:

```powershell
pwsh ./scripts/validate-package-boundaries.ps1
pwsh ./scripts/export-contracts.ps1
pwsh ./scripts/validate-canonical-outputs.ps1 -RequireGeneratedOutputs
```

## License

Elegy is licensed under [Apache 2.0](LICENSE).
