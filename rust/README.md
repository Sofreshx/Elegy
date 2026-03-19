# Elegy Rust Workspace

This subtree is the first-party Rust runtime family inside the main Elegy repo.

Its job is to implement behavior-heavy runtime concerns that are better served by Rust while continuing to consume governed contracts from the authoritative `.NET` package families under `src/`.

## Current slice

Imported or added so far:

- `elegy-contracts`
- `elegy-policy`
- `elegy-mcp`
- `elegy-descriptor`
- `elegy-adapter-fs`
- `elegy-adapter-http`
- `elegy-runtime`
- `elegy-core`
- `elegy-host-mcp`
- `elegy-cli`

These crates currently provide:

- Rust-native models for governed Elegy contracts
- semantic validators that mirror the contract authority owned by the `.NET` packages
- in-repo conformance checks against the exported contract bundle under `artifacts/contracts`
- reusable policy validation for filesystem and HTTP runtime boundaries
- parity-first Rust implementations of MCP analyzer, generator, and discovery behavior
- normalized project, descriptor, and resource loading for the imported runtime stack
- policy-bounded filesystem resource composition and read behavior for static and filesystem families
- policy-bounded HTTP resource composition and bounded GET execution with redirect, timeout, and size-limit normalization
- family-neutral runtime composition and a caller-facing core facade over descriptor, policy, adapters, and MCP consumers
- a thin stdio MCP host that serves runtime-composed resources from the imported core/runtime layers
- a thin operator CLI for config validation, runtime validation, resource inspection, and stdio host startup

## Current posture

The bootstrap runtime stack is now imported in-repo from contracts through operator surfaces:

- descriptor and policy loading stay below the runtime layer
- runtime and core remain the reusable composition surfaces
- `elegy-host-mcp` stays thin over `elegy-core`
- `elegy-cli` stays thin over `elegy-core` plus the stdio host entrypoint

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
