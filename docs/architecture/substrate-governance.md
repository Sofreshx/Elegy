# Elegy Substrate Governance

## Purpose

This document is the canonical Phase 1 governance baseline for the Elegy umbrella repo.

It defines:

- the substrate package boundary
- the current higher-level package-family boundary
- allowed source-package dependency direction
- the rules for promoting concepts into public package families
- the shared-contract governance model for schemas, fixtures, and conformance artifacts

This document is intentionally narrower than the broader ecosystem topology doc. The topology doc explains the high-level repo relationship. This document explains the concrete source-package governance needed before later phases can safely expand the public surface.

## Package tiers

Elegy package families currently sit in four conceptual tiers.

### Tier 1: substrate

These packages are the base layer and must stay free of provider-specific SDKs, framework ownership, and runtime-host assumptions.

| Package | Responsibility | Allowed source dependencies |
| --- | --- | --- |
| `Elegy.Formalization.Core` | Core abstractions and domain primitives | none |
| `Elegy.Formalization.Contracts` | Publishable contract resources and integration boundary artifacts | `Elegy.Formalization.Core` |
| `Elegy.Formalization.Serialization` | Serialization support over core models | `Elegy.Formalization.Core` |
| `Elegy.Formalization.Validation` | Validation utilities and rule evaluation for formalization artifacts | `Elegy.Formalization.Core` |
| `Elegy.Formalization.Governance` | Governance metadata and enforcement helpers | `Elegy.Formalization.Core` |
| `Elegy.Formalization.Projections.Mermaid` | Projection output from core structures | `Elegy.Formalization.Core` |

### Tier 2: definition families

These packages define reusable capabilities on top of the substrate.

| Package | Responsibility | Allowed source dependencies |
| --- | --- | --- |
| `Elegy.Formalization.Skills` | Skill definitions and lifecycle state | `Elegy.Formalization.Core` |
| `Elegy.Formalization.Skills.Discovery` | Skill discovery and indexing surfaces | `Elegy.Formalization.Core`, `Elegy.Formalization.Skills` |
| `Elegy.Formalization.Monitoring` | Monitoring-oriented formalization surfaces | `Elegy.Formalization.Core` |
| `Elegy.Formalization.Agents` | Agent-facing formalization primitives | `Elegy.Formalization.Core` |

### Tier 3: derived and transformation families

These packages are allowed to compose the substrate plus definition families, but they still must not become runtime-host ownership surfaces.

| Package | Responsibility | Allowed source dependencies |
| --- | --- | --- |
| `Elegy.Formalization.DynamicSkills` | Dynamic skill activation and runtime-oriented materialization helpers | `Elegy.Formalization.Core`, `Elegy.Formalization.Skills`, `Elegy.Formalization.Monitoring` |
| `Elegy.Formalization.Mcp` | MCP-facing analysis, descriptor transformation, and MCP-derived projections | `Elegy.Formalization.Skills` |
| `Elegy.Formalization.AgentFactory` | Agent construction helpers | `Elegy.Formalization.Core`, `Elegy.Formalization.Agents`, `Elegy.Formalization.Governance` |

### Tier 4: tooling and materialization

This is where deterministic generation and materialization concerns belong.

| Package | Responsibility | Allowed source dependencies |
| --- | --- | --- |
| `Elegy.Formalization.SkillForge` | Materialization, generated registration metadata, and generation-oriented outputs | `Elegy.Formalization.Core`, `Elegy.Formalization.Skills`, `Elegy.Formalization.DynamicSkills`, `Elegy.Formalization.Governance` |

## Dependency direction rules

The following rules are mandatory until a later architecture decision changes them explicitly:

1. Substrate packages may only reference substrate packages.
2. Definition families may depend on the substrate, but not on tooling, runtime adapters, or repo-external frameworks.
3. Derived or transformation families may depend on the substrate and on definition families, but they may not absorb runtime transport or host ownership.
4. Tooling families may depend on substrate and definition outputs, but lower tiers must never depend upward on tooling.
5. Human-facing CLI shells and runtime adapters remain outside this source-package dependency policy. They are top-layer consumers, not substrate-shaping inputs.
6. Cross-family consumer facades or metapackages are not implicit. If the repo later needs a convenience package such as a formalization facade, it must be introduced as a new explicitly-governed package family rather than referenced through synthetic paths in planning docs.

## Mixed-language monorepo rule

The Elegy repo now hosts two first-party implementation families:

1. `.NET` package families under `src/` and `tests/`
2. a Rust runtime family under `rust/`

The .NET package-boundary policy in this document applies only to the `.NET` source packages under `src/`.

The Rust runtime family is governed separately through its Cargo workspace, Rust lint configuration, and runtime-focused tests. Rust crates are allowed to consume governed contracts from the authoritative .NET contract families, but they must not silently redefine schema or canonical skill authority.

## Public package graduation rule

A concept should become a public package family only when all of the following are true:

1. The concept has a stable responsibility that cannot be explained as a temporary implementation helper.
2. The concept has a coherent dependency story that does not require upward references into tooling, adapters, or product hosts.
3. The concept has at least one validation harness or consumer path that exercises it through public types rather than internal implementation shortcuts.
4. The concept improves package clarity more than it increases coordination cost.

If those conditions are not met, keep the capability inside an existing family until the abstraction proves itself.

This applies to proposed facade or orchestration families too. A document may describe a future consumer-facing facade, but extraction planning must still target the current real package families until that facade passes graduation and exists in `src/`.

## Core contract change policy

Changes to substrate contracts are considered core contract changes when they alter any of the following:

- public types or members in substrate packages
- serialized schema shape or required fields in publishable contract artifacts
- semantics of compatibility manifests or conformance fixtures
- dependency-direction policy between source package families

Core contract changes must update the relevant docs, fixtures, and validation paths in the same change.

## Shared-contract governance

Shared contracts, fixtures, and conformance artifacts are governed centrally in the umbrella repo.

That means:

1. The authoritative source lives in `Elegy`, not in downstream consuming repos.
2. Versioning rules are defined here first, then consumed elsewhere.
3. First-party Rust crates in the same repo and any downstream consumers should consume published artifacts or versioned files, not co-own the truth through copy-paste drift.
4. Coordinated change procedures are required before a contract family becomes a multi-repo dependency.

The primary governed artifacts in Phase 1 are:

- `Directory.Build.props` package version baseline
- `schemas/schema-version.json` schema version baseline
- contract schemas and fixtures in `src/Elegy.Formalization.Contracts/Resources`
- compatibility manifest and compatibility matrix artifacts

## Fixture and conformance corpus rule

Fixtures are not example clutter. They are governed compatibility evidence.

Every publishable schema family should eventually have:

- at least one minimal valid fixture
- at least one compatibility manifest entry
- a machine-readable compatibility description where cross-repo consumers depend on the contract

When a schema or manifest is changed, the fixture corpus must be reviewed in the same change.

## Enforcement surfaces

Phase 1 enforcement lives in these surfaces:

- `scripts/validate-package-boundaries.ps1` validates source-package dependency direction
- `scripts/export-contracts.ps1` validates and exports governed contract artifacts
- `.github/workflows/versioning-governance.yml` validates SemVer and schema-governance coupling
- `.github/workflows/package-boundaries.yml` validates package boundaries and architecture governance tests
- `tests/Elegy.Formalization.Core.Tests/Architecture/*` pins the package-boundary and governed-contract rules in code
- `rust/` workspace validation will pin the Rust runtime-family side of the monorepo as that subtree is imported and expanded

## Phase 1 completion standard

Phase 1 is only complete when the substrate is not just described but enforced.

The minimum bar is:

1. package tiers are documented
2. dependency direction is machine-validated
3. version and schema governance are documented and checked
4. publishable fixtures and manifests are validated from the repo root
5. terminology is explicit enough that later phases do not have to redefine the same concepts
