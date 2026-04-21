# Elegy

Elegy is a Rust-based toolkit for making local CLI capabilities easy for AI agents to discover, reason about, and invoke safely. It combines governed contracts, schema-backed skill definitions, an umbrella CLI, dedicated helper CLIs, and an MCP host that exposes the same capabilities as runtime tools.

The goal is practical agent adoption: an agent should be able to run one discovery command, learn what Elegy can do, inspect exact invocation templates, understand side effects, and call the right capability through either subprocesses or MCP.

## What Elegy Provides

- **Runtime skill discovery** through `elegy skills list`, `elegy skills search`, and `elegy skills describe`.
- **V2 skill definitions only** in `contracts/fixtures/skill-definition-v2.*.json`, with per-capability implementation, input, output, execution, governance, and discovery metadata.
- **MCP resource and tool hosting** through `elegy run`, backed by the same v2 skill registry used by the CLI.
- **Agent-friendly JSON envelopes** with command path, status, diagnostics, summary, payload data, and optional `dataSchema` references.
- **Structured stdin workflows** for diagram and Mermaid commands, including JSON `DiagramPatch` input via `--patch-stdin`.
- **Perception and action primitives** through `observe` and `desktop`, with desktop mutations supporting `--dry-run`.
- **Utility skills** for repo inspection, web fetches, data conversion/validation, notifications, MCP descriptor handling, and local memory.

## Quick Start

From the Rust workspace:

```bash
cd rust
cargo run -p elegy-cli -- --version --json
cargo run -p elegy-cli -- skills list --json
cargo run -p elegy-cli -- skills describe --skill-id diagram --json
```

Build the umbrella CLI:

```bash
cd rust
cargo build -p elegy-cli
```

Run the MCP host over stdio:

```bash
elegy run
```

By default, MCP tool calls that have side effects are blocked unless the call is a dry run. Start the host with side-effect execution enabled only when the harness has its own approval policy:

```bash
elegy run --allow-side-effects --tool-timeout-seconds 30
```

## Main CLI Surfaces

| Surface | Purpose |
| --- | --- |
| `elegy skills list/search/describe` | Runtime discovery over the built-in v2 skill registry. |
| `elegy run` | MCP stdio host serving resources and skill-backed tools. |
| `elegy diagram create/patch/narrate/render` | Structured semantic diagram creation, mutation, explanation, and rendering. |
| `elegy mermaid render/reverse/narrate` | Mermaid projection, bounded reverse projection, and narrative summaries. |
| `elegy observe ...` | Read-only OS/process/window/screen/clipboard/filesystem/system observation. |
| `elegy desktop ...` | Desktop automation with dry-run support for high-risk actions. |
| `elegy repo ...` | Read-only git status, diff, branch, and log inspection. |
| `elegy web ...` | Bounded HTTP fetch and reachability checks. |
| `elegy data ...` | JSON/YAML/TOML/CSV conversion, extraction, and schema validation. |
| `elegy notify ...` | Local toast and webhook notification helpers. |
| `elegy-memory` | Dedicated local memory MVP CLI. |
| `elegy-mcp` | Dedicated MCP descriptor authoring and analysis CLI. |
| `elegy-skills` | Dedicated MCP-to-v2-skill generation CLI. |

## Agent Usage Pattern

1. Discover compactly:

   ```bash
   elegy skills search --query "diagram" --json
   ```

2. Expand only the needed skill:

   ```bash
   elegy skills describe --skill-id diagram --json
   ```

3. Use `capabilities[].implementation.arguments` as the exact invocation template.

4. Check `capabilities[].execution.hasSideEffects` before invoking mutations.

5. Prefer stdin-capable commands when available:

   ```bash
   echo '{"addNodes":[{"id":"api","label":"API"}]}' \
     | elegy diagram patch --input diagram.json --patch-stdin --output diagram.json --json
   ```

## Contracts

The authoritative contract bundle lives in `contracts/`:

- `contracts/schemas/skill-definition-v2.schema.json`
- `contracts/fixtures/skill-definition-v2.*.json`
- `contracts/manifests/compatibility-manifest.json`
- additional schemas and fixtures for invocation, execution, MCP descriptors, memory records, failures, and events

V1 skill definitions have been removed during early development. Consumers should target v2 directly.

## Repository Layout

```text
contracts/             Governed schemas, fixtures, manifests, and support metadata
docs/                  Architecture, distribution, and integration guidance
governance/            Policy and governance notes
rust/                  Rust workspace
rust/crates/elegy-cli  Umbrella CLI binary (`elegy`)
rust/crates/elegy-host-mcp  MCP stdio host
rust/crates/elegy-contracts Shared contract types and built-in skill registry
```

## Development

Common checks:

```bash
cd rust
cargo fmt
cargo test -p elegy-contracts -p elegy-mcp -p elegy-tooling -p elegy-host-mcp -p elegy-cli -p elegy-skills
```

Distribution workflows and release asset details are documented in [docs/distribution.md](docs/distribution.md). Agent-oriented invocation guidance lives in [docs/agent-integration.md](docs/agent-integration.md).
