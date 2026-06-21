# Rust Consolidation

## Purpose

This document records the current consolidation decision after the legacy source and test tree was removed from the repo.

The main Elegy repo is now the intended long-term home for both:

- the governed artifact roots that define contracts, compatibility, and policy
- the first-party Rust workspace that carries the executable, runtime, and operator-facing surfaces

## Current consolidated shape

The repository now converges on this shape:

- `contracts/` and `policies/` remain the authored authority roots
- `artifacts/contracts` remains the generated downstream handoff surface
- `rust/` is the in-repo Cargo workspace for reusable executable behavior
- `src/Elegy-*/install.ps1` are thin install passthroughs only; they do not reopen the removed source-package story
- root docs and root scripts define the contributor and validation path

This is no longer a story about keeping a removed legacy package tree authoritative. The current question is simpler: which responsibilities belong in governed artifacts, which belong in Rust executable crates, and which should stay consumer-local.

## What stays authoritative now

The following remain canonical in the repo today:

- governed schemas and fixtures under `contracts/`
- version and release policy under `contracts/schemas/`
- formalization policy under `policies/`
- export and validation scripts at the repo root

These are the durable coordination surfaces that downstream consumers should rely on.

## What Rust owns now

The Rust workspace is the first-party home for:

- governed-contract consumption in executable form
- MCP descriptor authoring, analysis, and skill generation tooling
- the dedicated `elegy-memory`, `elegy-mcp`, `elegy-planning`, `elegy-skills`, and `elegy-configuration` binaries
- runtime composition and bounded adapter behavior
- the thin stdio MCP host
- the human-facing `elegy` CLI

The currently shipped self-authoring surface is the Rust CLI path for:

- `author mcp`
- `analyze mcp`
- `generate skills`
- `generate codex-plugin`

Those commands are backed by shared Rust crates led by `rust/crates/elegy-mcp` and `rust/crates/elegy-tooling`, exposed through both the umbrella `elegy` CLI and the dedicated `elegy-mcp` / `elegy-skills` binaries, and exercised by CLI and tooling tests in the Rust workspace.

## What is still a target

The repo should not currently claim more than it proves.

These remain forward-looking targets rather than completed surfaces:

- built-in MCP-native self-authoring as a settled product surface
- skill-driven self-authoring loops presented as already integrated operator behavior
- broad autonomous agent workflows layered directly into the runtime by default
- claims that REST/OpenAPI ingestion, hosted MCP runtime execution, or autonomous registration are already shipped because the thin dedicated CLIs now exist

`elegy-host-mcp` exists, and the CLI includes runtime validation, inspection, and run entrypoints, but those facts do not by themselves justify a claim that the broader self-authoring experience is already delivered.

## Crate publishing policy

All crates are blocked from crates.io with `publish = false`.
Distribution flows through GitHub Releases, binary artifacts, wrapper
surfaces, and agent-facing skill + MCP projections — not through
`cargo install`.  See `docs/adr/2026-06-15-block-crates-io-publishing.md`
for the decision record and the procedure to re-enable publishing for
a specific crate.

## Replacement rule

Prefer governed artifacts when the responsibility is:

- schema authority
- fixture and compatibility evidence
- policy and version governance
- machine-readable handoff for downstream consumers

Prefer Rust when the responsibility is:

- reusable executable behavior
- descriptor analysis or generation
- runtime composition and bounded adapters
- operator-facing CLI or host behavior

Keep the capability in consuming repos when the responsibility is:

- app-specific endpoints or transport wrappers
- auth, tenancy, persistence, and local orchestration
- prompt assembly tied to a specific host or product shell

## Validation and export posture

Contributor-facing validation should point to the smallest real flows that still exist: repo-root PowerShell bundle scripts plus Rust workspace checks.

### Contracts and exports

```powershell
pwsh ./scripts/export-contracts.ps1 -CreateArchive
pwsh ./scripts/validate-canonical-outputs.ps1 -RequireGeneratedOutputs
```

### Rust executable surfaces

Run from `rust/`:

```bash
cargo fmt --all --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-targets --all-features
```

Docs should point contributors only at repo-root bundle scripts and Rust workspace checks that still run in this repo.

## Current next sequence

1. keep hardening the Rust CLI, tooling crates, and host/runtime surfaces that ship from the in-repo workspace
2. keep the governed contract, policy, and export roots under `contracts/` and `policies/` cleanly versioned and validated with the repo-root PowerShell bundle scripts
3. finish removing stale docs that still imply deleted source, test, or package-family centers
4. only document broader built-in self-authoring or MCP-hosted operator experiences once the Rust workspace proves them as runnable, contributor-facing surfaces

## Validation posture

Validation now centers on the repo-root PowerShell bundle scripts and the Rust workspace checks.

- repo-root scripts validate exported contracts, canonical outputs, and package boundaries
- Rust workspace checks validate formatting, linting, and tests for the shipped executable surface
- docs should describe only those runnable validation paths unless new contributor-facing flows are added
