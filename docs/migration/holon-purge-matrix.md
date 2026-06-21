# Holon Purge Matrix

Historical-only migration note: this matrix tracks the overlap and delete sequencing that existed during the zero-dotnet purge. References to `.NET`, `Directory.Build.props`, and package-family surfaces here describe migration-era overlap, not the current repo authority model.

Scope: staged removal of Holon-oriented responsibility and all compiled C# or .NET from the steady-state Elegy repo.

Steady-state target:

- Rust crates, CLI or similar tool binaries, docs, tests, workflows, and governed file artifacts remain.
- Compiled C# projects, dotnet-based CI requirements, and NuGet as a primary distribution lane do not remain.
- Temporary overlap is allowed only when the surface is deprecated, non-expanding, owned, validated, and tied to a sunset gate.

The `src/Elegy-*/install.ps1` files are thin install passthroughs only and do not reopen the old compiled package-family story. They are not compiled packages, authority layers, implementation centers, or release surfaces.

Disposition meanings:

- `Rust-owned replacement` - keep the capability, but move executable ownership to Rust.
- `Static artifact authority` - keep the authority only as governed files, not as a compiled .NET package.
- `Temporary deprecated overlap` - keep the current .NET surface only until a replacement or removal gate is satisfied.
- `Intentional removal` - delete the capability instead of replacing it if it does not clear the burden-of-proof bar.

## Source package matrix

| Surface | Current role | Disposition | Planned steady-state owner | Deletion gate |
| --- | --- | --- | --- | --- |
| `src/Elegy.Formalization.Core` | Shared model primitives and canonical semantics currently anchored in `.NET`. | `Temporary deprecated overlap` | Split between governed file artifacts and Rust semantics once the concept table is resolved. | Delete only after the retained semantics have moved to file or Rust ownership and all consumers are cut over. |
| `src/Elegy.Formalization.Contracts` | JSON schemas, fixtures, compatibility metadata, and bundle-oriented contract shape. | `Temporary deprecated overlap` | `src/` package disappears; governed files remain as the authority. | Delete only after file-native authority and bundling are validated without `dotnet`. |
| `src/Elegy.Formalization.Skills` | Canonical skill definitions and lifecycle semantics. | `Temporary deprecated overlap` | Governed skill files and or Rust executable semantics, depending on concept decisions. | Delete only after retained skill semantics no longer require compiled `.NET`. |
| `src/Elegy.Formalization.Serialization` | Convenience serialization helpers over core models. | `Intentional removal` | File formats plus Rust consumers; no dedicated `.NET` package survives. | Delete after consumers stop depending on the `.NET` serializer path. |
| `src/Elegy.Formalization.Validation` | Validation helpers and rule evaluation. | `Temporary deprecated overlap` | Rust executable validation plus file-owned schema inputs if retained. | Delete after the validation concept owner is resolved and replacement proof exists. |
| `src/Elegy.Formalization.Governance` | Governance metadata and resolution semantics. | `Temporary deprecated overlap` | Rust executable governance over file-owned policy artifacts if retained. | Delete after governance enforcement no longer depends on compiled `.NET`. |
| `src/Elegy.Formalization.Projections.Mermaid` | Mermaid projection helper. | `Intentional removal` | Removed; rendering now stays in the active SAASTools workflow product surface. | Completed in the current migration slice. |
| `src/Elegy.Formalization.Monitoring` | Monitoring-oriented formalization helpers. | `Intentional removal` | No dedicated steady-state owner unless future runtime observability proves a real need. | Delete when no consumer-class depends on the package. |
| `src/Elegy.Formalization.Skills.Discovery` | Skill indexing, search, and resolve helpers. | `Rust-owned replacement` | Rust discovery and indexing lane. | Delete after Rust discovery passes CI, fixtures, examples, and consumer-class gates. |
| `src/Elegy.Formalization.DynamicSkills` | Dynamic skill activation and materialization helpers. | `Rust-owned replacement` | Rust runtime or tooling lane. | Delete after retained materialization behavior is implemented in Rust or intentionally dropped. |
| `src/Elegy.Formalization.SkillForge` | Forge and generation semantics plus generated metadata. | `Rust-owned replacement` | Rust CLI or tooling lane. | Delete after forge semantics are either implemented in Rust or explicitly dropped. |
| `src/Elegy.Formalization.Mcp` | MCP descriptors, analysis, and projections currently split between authority and executable behavior. | `Temporary deprecated overlap` | Governed file artifacts for shape plus Rust executable MCP semantics. | Delete after MCP concept ownership is split cleanly and the Rust path is consumed by real runtime flows. |
| `src/Elegy.Formalization.Agents` | Agent-facing primitives. | `Intentional removal` | No standalone package survives unless a reduced contract shape moves elsewhere. | Delete when the concept table confirms no retained agent package is needed. |
| `src/Elegy.Formalization.AgentFactory` | Agent construction helpers. | `Intentional removal` | No steady-state owner. | Delete after no consumer-class depends on the helper surface. |

## Test project matrix

All current `.NET` test projects are delete-bound surfaces. None survive in the steady state.

| Surface | Mirrors | Disposition | Deletion gate |
| --- | --- | --- | --- |
| `tests/Elegy.Formalization.Core.Tests` | Core package plus package-boundary governance tests | `Temporary deprecated overlap` | Delete after file-native or Rust-native governance validation replaces the current architecture tests. |
| `tests/Elegy.Formalization.Serialization.Tests` | Serialization package | `Intentional removal` | Delete after serializer behavior is either dropped or proven through Rust or artifact validation. |
| `tests/Elegy.Formalization.Validation.Tests` | Validation package | `Temporary deprecated overlap` | Delete after Rust or artifact validation replaces the retained checks. |
| `tests/Elegy.Formalization.Governance.Tests` | Governance package | `Temporary deprecated overlap` | Delete after governance enforcement is validated without `.NET`. |
| `tests/Elegy.Formalization.Skills.Tests` | Skills package | `Temporary deprecated overlap` | Delete after retained skill semantics move off compiled `.NET`. |
| `tests/Elegy.Formalization.Skills.Discovery.Tests` | Skills.Discovery package | `Intentional removal` | Delete after Rust discovery coverage exists. |
| `tests/Elegy.Formalization.DynamicSkills.Tests` | DynamicSkills package | `Intentional removal` | Delete after Rust replacement or intentional feature drop is proven. |
| `tests/Elegy.Formalization.SkillForge.Tests` | SkillForge package | `Intentional removal` | Delete after Rust forge coverage or intentional feature drop is proven. |
| `tests/Elegy.Formalization.Mcp.Tests` | MCP package | `Temporary deprecated overlap` | Delete after Rust MCP runtime path plus golden or parity coverage is in place. |
| `tests/Elegy.Formalization.Agents.Tests` | Agents package | `Intentional removal` | Delete with the package once no retained concept requires it. |
| `tests/Elegy.Formalization.AgentFactory.Tests` | AgentFactory package | `Intentional removal` | Delete with the package once no retained concept requires it. |
| `tests/Elegy.Formalization.Monitoring.Tests` | Monitoring package | `Intentional removal` | Delete with the package unless observability surfaces are explicitly retained. |
| `tests/Elegy.Formalization.Projections.Mermaid.Tests` | Mermaid package | `Intentional removal` | Deleted with the Mermaid package in the current migration slice. |

## Script and workflow matrix

| Surface | Current role | Disposition | Planned steady-state owner | Deletion gate |
| --- | --- | --- | --- | --- |
| `scripts/export-contracts.ps1` | Compatibility shim that delegates contract export to the Rust CLI bundler. | `Temporary deprecated overlap` | Rust-owned bundling path. | Delete after local callers and docs no longer need the shim. |
| `scripts/validate-package-boundaries.ps1` | Enforces the current repo-boundary policy through the legacy `package-boundaries` compatibility lane. | `Intentional removal` | No steady-state owner unless a Rust-native equivalent is later justified. | Delete after the compatibility path is no longer needed. |
| `scripts/bump-version.ps1` | Updates `schemas/schema-version.json`; `Package*` parameter names remain only for CLI compatibility. | `Temporary deprecated overlap` | Rust-native or file-native version tooling. | Keep the compatibility interface only as long as downstream callers still depend on it. |
| `.github/workflows/rust-ci.yml` | Primary Rust validation lane, now reading the checked-in contract bundle without a PowerShell export step. | `Rust-owned replacement` | Keep workflow as a dotnet-free Rust validation lane. | Keep the lane dotnet-free and prevent regressions that reintroduce export-time dependencies. |
| `.github/workflows/distribution-artifacts.yml` | Builds repo-local Rust CLI archives and the contracts bundle as CI artifacts; package-feed overlap is tracked separately as a historical consumer-migration concern, not a current producer lane here. | `Rust-owned replacement` | Rust-first artifact workflow. | Keep unless artifact packaging is consolidated elsewhere; do not reframe this workflow as a current `.nupkg` producer. |
| `.github/workflows/publish-distribution.yml` | Publishes release-attached Rust CLI archives and the contracts bundle; package-feed overlap is tracked separately as a historical consumer-migration concern, not a current producer lane here. | `Rust-owned replacement` | Rust-first release workflow plus artifact publication. | Keep unless release-asset publication is consolidated elsewhere; do not reframe this workflow as a current NuGet publisher. |
| `.github/workflows/package-boundaries.yml` | Runs repo-boundary validation through the legacy `package-boundaries` compatibility name, with contract export delegated to Rust. | `Temporary deprecated overlap` | Rust-native or file-native governance validation if still needed. | Delete or rename only after the compatibility entrypoint is no longer needed. |
| `.github/workflows/versioning-governance.yml` | Validates file-native version governance in `schemas/schema-version.json`; any `Directory.Build.props` language is historical compatibility tracking only. | `Temporary deprecated overlap` | File-native version and schema governance lane. | Delete or rename only after compatibility naming is no longer needed. |
| `.github/workflows/security.yml` | Mixed Rust and `.NET` security analysis, including C# CodeQL. | `Temporary deprecated overlap` | Rust-focused security lane. | Delete or rewrite the C# build path after no compiled `.NET` remains. |
| `.github/workflows/ws3-formalization-governance.yml` | Shared governance workflow driven by PowerShell assets. | `Temporary deprecated overlap` | Keep or replace as tooling after the concept table resolves governance ownership. | Re-evaluate only after governance semantics and scripting strategy are settled. |

## Published-surface disposition table

| Published surface | Current producer | Current consumer class | Disposition | Deletion or retention rule |
| --- | --- | --- | --- | --- |
| Historical GitHub Packages `.nupkg` surface | No current in-repo producer; tracked only as historical or external-overlap migration evidence. | Known SAASTools package-feed expectation plus unknown external consumers | `Historical or external-overlap tracking` | Do not treat this as a current Elegy production lane. Keep tracking until the surface is either proved closed-world, deprecated with a notice window, or retired through a validated consumer-class cutover. |
| `artifacts/distribution/elegy-contracts-*.zip` | `cargo run --manifest-path ./rust/Cargo.toml -p elegy-cli -- contracts export --create-archive` | Artifact-bundle consumers and release downloads | `Static artifact authority` | Keep the surface, while continuing to decouple the wider release lane from `.NET`. |
| `artifacts/contracts/**` | `cargo run --manifest-path ./rust/Cargo.toml -p elegy-cli -- contracts export` | Repo-local Rust crates, tests, examples, and external contract consumers | `Static artifact authority` | Keep the surface on the Rust or file-native preparation path. |
| Reusable WS3 governance workflow asset | `.github/workflows/ws3-formalization-governance.yml` | Caller repos using `workflow_call` | `Temporary deprecated overlap` | Cannot delete until callers are known or the workflow is formally deprecated. |

## Initial delete-first tranche

The current recommended first delete tranche remains:

- `src/Elegy.Formalization.Skills.Discovery`
- `src/Elegy.Formalization.DynamicSkills`
- `src/Elegy.Formalization.SkillForge`
- `src/Elegy.Formalization.AgentFactory`
- matching `.NET` test projects and any package-boundary or workflow references tied only to those packages

Those surfaces are the best fit for early removal because they are behavior-heavy and already align with Rust-first ownership better than the remaining authority-oriented packages.