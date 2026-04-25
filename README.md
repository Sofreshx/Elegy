# Elegy

Elegy is a Rust toolkit that makes local CLI capabilities discoverable,
selectable, and safe for AI-agent hosts to invoke. Its core model is:

- governed contracts are the durable authority
- v2 skill definitions are the discovery authority
- CLI invocation templates are the default execution boundary
- MCP is an optional projection for MCP-native clients

## Quick Start

From the Rust workspace:

```bash
cd rust
cargo run -p elegy-cli -- --version --json
cargo run -p elegy-cli -- agent check --json
cargo run -p elegy-cli -- agent manifest --json
cargo run -p elegy-cli -- agent discover --query "repo status" --json
```

Build the umbrella CLI:

```bash
cd rust
cargo build -p elegy-cli
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
| `elegy skills list/search/describe` | Raw runtime discovery over the built-in v2 skill registry. |
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
| `elegy-mcp` | Dedicated MCP descriptor authoring and analysis CLI. |
| `elegy-skills` | Dedicated MCP-to-v2-skill generation CLI. |

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

## Contracts

The authoritative contract bundle lives in `contracts/`:

- `contracts/schemas/skill-definition-v2.schema.json`
- `contracts/schemas/agent-capability-profile.schema.json`
- `contracts/fixtures/skill-definition-v2.*.json`
- `contracts/manifests/compatibility-manifest.json`
- additional schemas for invocation, responses, failures, memory records, MCP
  descriptors, and events

## Development

Common checks:

```bash
cd rust
cargo fmt
cargo test -p elegy-contracts -p elegy-host-mcp -p elegy-cli
```

Distribution workflows are documented in [docs/distribution.md](docs/distribution.md).
Agent integration guidance lives in [docs/agent-integration.md](docs/agent-integration.md).
