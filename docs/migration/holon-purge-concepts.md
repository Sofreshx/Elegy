# Holon Purge Concept Decision Table

Historical-only migration note: this table records pre-zero-dotnet ownership assumptions used during the purge analysis. References to `.NET`, `Directory.Build.props`, and `Elegy.Formalization.*` describe historical overlap, not the current authority model. Current canonical roots are `contracts/`, `governance/`, and `rust/`.

Scope: historical concept-level ownership decisions that had to be made before deleting the remaining `.NET` surfaces.

Decision meanings:

- `Rust executable semantics` - the concept survives as executable behavior owned by Rust crates or CLI tooling.
- `File artifacts only` - the concept survives only as governed files, with no dedicated compiled package.
- `Intentionally dropped` - the concept does not survive unless later re-proven.

## Concept table

| Concept | Historical pre-purge owner | Recommended steady-state owner | Status | Notes and blocking proof |
| --- | --- | --- | --- | --- |
| Schema files and fixtures | `src/Elegy.Formalization.Contracts/Resources` | `File artifacts only` | Recommended | Keep the files as authority, but stop treating the `.csproj` as the authority boundary. |
| Compatibility manifest and compatibility matrix | `src/Elegy.Formalization.Contracts/Resources` plus export script | `File artifacts only` | Recommended | Keep the files, replace `.NET` packaging and export assumptions. |
| Contract bundle assembly and release packaging | `elegy contracts export` in the Rust workspace, with `scripts/export-contracts.ps1` as a compatibility wrapper | `Rust executable semantics` | Implemented for current bundle production | Rust now builds `artifacts/contracts` and the versioned contracts archive without `dotnet`; any remaining package-feed discussion is historical compatibility tracking only. |
| Package or release version authority | `Directory.Build.props` | `governance/version-policy.json` plus `schemas/schema-version.json` | Implemented | Version authority is now file-native; the `Directory.Build.props` reference is historical only. |
| Schema version authority | `schemas/schema-version.json` | `File artifacts only` | Recommended | This file can survive with minimal change. |
| Governance policy data | Policy files and current governance scripts | `File artifacts only` | Recommended | Keep policy as files regardless of whether execution moves to Rust. |
| Governance enforcement and resolution semantics | `Elegy.Formalization.Governance` | `Rust executable semantics` | Pending implementation | Rust currently lacks equivalent enforcement and resolution ownership. |
| Skill discovery and indexing | `Elegy.Formalization.Skills.Discovery` | `Rust executable semantics` | Pending implementation | This is one of the clearest Rust gaps and a hard blocker for early purge. |
| Forge or materialization semantics | `Elegy.Formalization.SkillForge` and related helpers | `Rust executable semantics` | Pending implementation | Keep only the behavior that still clears the burden-of-proof bar for a CLI or tooling repo. |
| MCP descriptor and shape contracts | `Elegy.Formalization.Mcp` and contract resources | `File artifacts only` | Recommended | Keep shape and exchange contracts as governed files. |
| MCP analyzer, generator, search, and resolve semantics | `Elegy.Formalization.Mcp` plus Rust parity crates | `Rust executable semantics` | In progress | Rust already has parity-first work; this must become the real consumed path. |
| Executable validation behavior | `Elegy.Formalization.Validation` | `Rust executable semantics` | Pending implementation | Keep file-owned schemas, but move runtime validation behavior off `.NET`. |
| Mermaid projection semantics | Removed from compiled Elegy packages; rendering moved to the active SAASTools workflow product surface | `Intentionally dropped` | Completed in current migration slice | Preserve no dedicated compiled Elegy package unless a later Rust tooling consumer proves it necessary. |
| Agent helper semantics | `Elegy.Formalization.Agents` and `Elegy.Formalization.AgentFactory` | `Intentionally dropped` | Recommended | Keep only smaller contract shapes elsewhere if a real need appears. |
| Monitoring helper semantics | `Elegy.Formalization.Monitoring` | `Intentionally dropped` | Recommended | Retain only if future runtime observability proves it needs governed files or Rust ownership. |

## Historical hard blockers before the first delete tranche

The first delete tranche should not start until these concept decisions are backed by real replacement proof:

- governance enforcement exists outside compiled `.NET`
- discovery and indexing exist outside compiled `.NET`
- forge or materialization semantics are either implemented in Rust or explicitly reduced
- release and archive production keep using the Rust exporter path and do not regress back to `scripts/export-contracts.ps1` as the source of logic

## Historical delete-last concept cluster

This list records why the authority-oriented `.NET` packages could not be deleted earlier. It is not the current repo-center model:

- file-owned schema and compatibility authority still compiled and released through `.NET`
- skill semantics still partially anchored in `.NET`
- governance semantics still anchored in `.NET`
- version authority was still anchored in `Directory.Build.props`

Those concepts had to be resolved before packages such as `Core`, `Contracts`, `Skills`, and `Governance` could be removed cleanly.