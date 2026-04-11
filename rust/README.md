# Elegy Rust Workspace

This subtree is the first-party Rust runtime family inside the main Elegy repo.

Its job is to implement behavior-heavy runtime concerns that are better served by Rust while continuing to consume governed contracts from the authoritative neutral roots under `contracts/` and `governance/`.

## Current slice

Imported or added so far:

- `elegy-contracts`
- `elegy-memory`
- `elegy-policy`
- `elegy-mcp`
- `elegy-skills`
- `elegy-tooling`
- `elegy-descriptor`
- `elegy-adapter-fs`
- `elegy-adapter-http`
- `elegy-runtime`
- `elegy-core`
- `elegy-host-mcp`
- `elegy-cli`

These crates currently provide:

- Rust-native models for governed Elegy contracts
- semantic validators that mirror the contract authority owned by `contracts/` and `governance/`
- in-repo conformance checks against the exported contract bundle under `artifacts/contracts`
- reusable policy validation for filesystem and HTTP runtime boundaries
- parity-first Rust implementations of MCP analyzer, generator, and discovery behavior
- internal Rust tooling for MCP descriptor authoring and MCP-to-skill generation from governed descriptor inputs
- normalized project, descriptor, and resource loading for the imported runtime stack
- policy-bounded filesystem resource composition and read behavior for static and filesystem families
- policy-bounded HTTP resource composition and bounded GET execution with redirect, timeout, and size-limit normalization
- family-neutral runtime composition and a caller-facing core facade over descriptor, policy, adapters, and MCP consumers
- a thin stdio MCP host that serves runtime-composed resources from the imported core/runtime layers
- a thin operator CLI for config validation, runtime validation, resource inspection, MCP descriptor authoring, MCP analysis, MCP-to-skill generation, and stdio host startup
- dedicated thin CLIs for bounded local memory, dedicated MCP descriptor authoring/analysis, and dedicated MCP-to-skill generation

## Direct system surfaces vs shared foundation crates

The preferred direct CLI/system surfaces in this workspace are:

- `elegy-memory` for the bounded memory system
- `elegy-mcp` for dedicated MCP descriptor authoring and analysis
- `elegy-skills` for dedicated MCP-to-skill generation
- `elegy` as the umbrella general/compatibility surface

External agents outside Elegy should load the associated skill guidance and invoke the matching dedicated `elegy-*` CLI directly when one exists. Elegy itself should not be described as internally calling or orchestrating those agents.

The main shared internal foundation crates under those surfaces include:

- `elegy-contracts`
- `elegy-policy`
- `elegy-descriptor`
- `elegy-adapter-fs`
- `elegy-adapter-http`
- `elegy-runtime`
- `elegy-core`
- `elegy-host-mcp`
- `elegy-tooling` as shared helper and compatibility infrastructure for descriptor and skill workflows

## Current posture

The bootstrap runtime stack is now imported in-repo from contracts through operator surfaces:

- descriptor and policy loading stay below the runtime layer
- runtime and core remain the reusable composition surfaces
- `elegy-host-mcp` stays thin over `elegy-core`
- `elegy-cli` stays thin over `elegy-core` plus the stdio host entrypoint
- `elegy-memory`, `elegy-mcp`, and `elegy-skills` stay thin wrapper surfaces over their owned Rust implementation crates
- `elegy-tooling` remains shared lower-level helper/compat infrastructure, not the preferred direct `elegy-skills` consumption surface

The next work in this subtree should focus on hardening and operating these imported surfaces in-repo rather than rebuilding them in parallel elsewhere.

## Operational posture

The Rust workspace lives inside the main `Elegy` monorepo, so contributor and governance posture is owned from the repository root.

Use these root-level docs for the authoritative public guidance:

- [`../CONTRIBUTING.md`](../CONTRIBUTING.md)
- [`../SECURITY.md`](../SECURITY.md)
- [`../CODE_OF_CONDUCT.md`](../CODE_OF_CONDUCT.md)
- [`../docs/spec-baseline.md`](../docs/spec-baseline.md)
- [`../CHANGELOG.md`](../CHANGELOG.md)

Historical standalone sibling repos, where they still exist, should be treated as archival references rather than the primary source of truth for the Rust runtime family.
