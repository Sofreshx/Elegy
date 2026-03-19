# Elegy Ecosystem Topology

## Purpose

This document defines the high-level organization of the Elegy ecosystem so package boundaries stay clear while the repositories are still evolving.

The main goal is to keep Elegy reusable across Holon and non-Holon projects while staying:

- LLM-agnostic
- provider-agnostic
- framework-agnostic

## Top-level decision

`Elegy` is the single main monorepo and contract home.

It owns the shared formalization, contract, governance, skill, and generation-oriented package families that can be consumed by many runtimes, plus the first-party Rust runtime subtree that executes or composes those governed artifacts.

The Rust runtime family should live inside the main Elegy repo under `rust/`. It should stay narrow rather than becoming the umbrella source of truth for contracts.

The current standalone `Elegy-Skills` and `Elegy-CLI` repositories should not be treated as the primary implementation surfaces right now. Until a later split is justified, the active design center is:

- the .NET package-family strategy in `Elegy/src`
- the first-party Rust runtime family in `Elegy/rust`

## Package families in the umbrella repo

### Core formalization and contracts

These packages define the reusable substrate:

- `Elegy.Formalization.Core`
- `Elegy.Formalization.Contracts`
- `Elegy.Formalization.Serialization`
- `Elegy.Formalization.Validation`
- `Elegy.Formalization.Governance`
- `Elegy.Formalization.Projections.Mermaid`

These packages should remain free of provider-specific SDKs, runtime-host assumptions, and framework-owned abstractions.

Integration-oriented extraction work should target these real package families directly. Planning documents should not refer to a synthetic `src/Elegy.Formalization/...` path, because no such source-package root exists in the repo today.

### Skills family

The skills family owns skill definitions, discovery, and dynamic materialization:

- `Elegy.Formalization.Skills`
- `Elegy.Formalization.Skills.Discovery`
- `Elegy.Formalization.DynamicSkills`

The intended v1 center of gravity is executable capability wrappers and formalized skill artifacts, not prompt-engine coupling.

### MCP formalization family

`Elegy.Formalization.Mcp` owns MCP-facing analysis and formalization concerns such as:

- server and tool descriptors
- MCP tool analysis
- skill generation from MCP tools
- governed MCP contract artifacts that sibling runtimes can consume

This package family should describe and transform MCP surfaces, but it should not absorb long-term host, transport, runtime, or tooling ownership that belongs in the in-repo Rust runtime family.

### Rust runtime family

The `rust/` subtree is the first-class home for:

- contract-consumer utilities in Rust
- runtime composition and policy-bounded execution
- filesystem and HTTP adapters
- MCP host and CLI layers
- future Rust replacements for behavior-heavy .NET MCP logic once parity is proven

It is not the authority for governed schemas or canonical skill contracts. It consumes and enforces those authorities.

### Tooling and generation family

`Elegy.Formalization.SkillForge` is the current best fit for the generation/tooling layer.

This is the place for:

- materialization flows
- generated registration metadata
- scaffolding or tool-output manifests
- future generated-tool or CLI-oriented output contracts

This layer should be treated as the generation/tooling family rather than as the definition of the user-facing `elegy` command-line experience.

### Agent-facing families

The current repo also has explicit agent-related package families:

- `Elegy.Formalization.Agents`
- `Elegy.Formalization.AgentFactory`

Those families are the current real landing zones for agent-facing primitives and construction helpers. They do not automatically imply that a broader `Elegy.Orchestration` family already exists.

## Integration-ready extraction targeting

For near-term extraction and migration work:

- canonical workflow/domain model types should target `Core`
- publishable exchange DTOs and boundary artifacts should target `Contracts`
- serialization helpers should target `Serialization`
- validation resolution and rule evaluation should target `Validation`
- governance policy/defaulting/pinning behavior should target `Governance`
- Mermaid output should target `Projections.Mermaid`
- agent primitives should target `Agents` unless and until a broader family is explicitly approved

If downstream consumers later need a single convenience surface, introduce a new facade package family or metapackage explicitly. Until then, adapters should compose the existing families rather than pretending a consolidated root already exists.

## Relationship between MCP, skills, and tooling

The dependency direction should remain one-way:

1. Core/contracts/governance at the bottom
2. MCP formalization and skills as peer families above the substrate
3. Tooling/generation above shared contracts and skill/MCP descriptors
4. Human-facing CLI shells on top of explicit public facades or direct package-family composition
5. Rust runtime-family crates consuming the governed contract surfaces without redefining them

That means:

- skills may import MCP analysis output, but core MCP runtime/transport concerns must not depend on skills
- tooling may depend on MCP and skill descriptors, but core skills and MCP contracts must not depend on tooling
- no package in the substrate should depend on model providers, agent frameworks, or application runtime glue

## CLI naming rule

Do not use `CLI` as the name of every command-related concern.

Two separate things exist:

- a human-facing `elegy` CLI or shell surface
- a tooling/generation layer that may emit runnable tools, manifests, or stubs

Those are related, but they are not the same subsystem.

For planning and package organization, prefer terms like `tooling`, `toolgen`, or `forge` for the generation layer. Reserve `elegy` CLI naming for the human-facing entrypoint.

## Agnostic boundary rules

To preserve reuse across Holon and other projects:

- skill definitions must not require a specific LLM vendor or model SDK
- MCP descriptors must not assume Holon DesktopHost, GitHub Copilot, or any single application host
- generation outputs must be based on explicit contracts and manifests rather than hidden framework behavior
- framework-specific integrations should live in adapters or runtime-family crates, not in core package families

## Phase 1 companion docs

The current substrate baseline is further defined in:

- [Substrate governance](substrate-governance.md)
- [Terminology](terminology.md)

## Phase 2 companion docs

The current skill-core authority decision is further defined in:

- [Skill Core V1](skill-core-v1.md)

## Split policy for future repos

If a package family later proves it needs its own release cadence, contributor base, or implementation language, it can split back out into a dedicated repo.

That split should happen only after:

- the package boundary is already stable
- at least two real consumers exist
- the split improves ownership more than it increases coordination cost

## Current practical stance

For now, the most coherent working model is:

- `Elegy` is the single main repo
- `src/` remains authoritative for formalization, schema, and contract families
- `rust/` is the first-party Rust runtime family for MCP behavior where Rust is the better implementation fit
- `Elegy-Skills` and `Elegy-CLI` should be treated as inactive placeholders unless and until the package boundaries are proven enough to justify separate repos
