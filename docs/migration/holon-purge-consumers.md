# Holon Purge Consumer Map

Scope: consumer classes, published surfaces, and cutover evidence requirements for the staged Elegy purge.

Deletion rule: no `.NET` surface may be deleted just because a repo-local replacement exists. Each deletion needs either:

- repo-local proof that the replacement works without `dotnet`
- consumer-class proof that the replacement or intentional removal is acceptable
- published-surface proof that the previous lane is closed-world, deprecated with a notice window, or blocked from deletion pending evidence

## Consumer-class map

| Consumer class | Current dependency | Current evidence | Expected cutover proof |
| --- | --- | --- | --- |
| Repo-local Rust crates | `artifacts/contracts/**`, schema files, compatibility metadata, and current export flow assumptions | `rust-ci.yml` now reads the checked-in contract bundle directly and targeted Rust contract-consumer tests pass without `dotnet` | Keep Rust CI, examples, and acceptance tests running from a clean checkout without `dotnet` or PowerShell export dependencies. |
| Repo-local CLI and binary users | `elegy-cli` and future release assets | CLI exists, but release and distribution are not yet Rust-first | A published CLI binary path plus clean-checkout validation must exist before NuGet leaves the primary story. |
| Artifact-bundle consumers | `artifacts/contracts/**` and `elegy-contracts-*.zip` | `docs/distribution.md` already describes this handoff surface | The bundle must continue to exist with stable shape and a non-`.NET` producer. |
| Known downstream SAASTools or Holon expectations | Package-feed and workflow-formalization expectations | Adjacent repo docs already describe a `github-elegy` package source and Elegy package seam for workflow formalization | A replacement path or an explicit intentional-removal decision must be documented before deleting published package surfaces. |
| Unknown external package or bundle consumers | GitHub Packages outputs and release assets | Repo-local evidence is incomplete by default | Each published surface must be treated as closed-world validated, externally deprecated with a notice window, or blocked from deletion pending evidence. |
| Caller repos using the WS3 reusable workflow | `.github/workflows/ws3-formalization-governance.yml` and its PowerShell assets | The workflow is a publishable reusable asset even if current callers are not all visible here | Do not remove or rewrite this lane until caller impact is understood or a deprecation path is published. |

## Published-surface rules

For every published or publishable surface, record one of these states before deletion:

- `Closed-world validated` - the maintainers can prove all consumers are known and cut over.
- `Externally deprecated` - the surface enters a notice window with release-note and documentation updates.
- `Blocked pending evidence` - deletion is not allowed yet because consumer impact is unknown.

This rule applies to:

- GitHub Packages `.nupkg` outputs
- release-attached contract bundles
- `artifacts/contracts/**` as a machine-readable handoff surface
- reusable workflow assets that callers may depend on outside this repo

## Temporary overlap governance

Every overlap surface must record:

- exact surface name
- owner
- non-expansion rule
- validation lane
- sunset trigger
- latest allowed removal milestone or version
- retirement evidence

Overlap that misses its sunset trigger is not silently extended. It must be escalated and re-approved explicitly.

## Release rollback rule

Before a release or distribution lane is removed, capture:

- the last-known-good tag or baseline
- the workflow path that produced the old lane
- toolchain assumptions needed to reproduce it
- the replacement lane and the evidence that it has shipped successfully

If that rollback package does not exist, deletion of the old published surface is blocked.

## First cutover priorities

The highest-value consumer cutovers to prove early are:

1. keep repo-local Rust CI dotnet-free and prevent regressions that reintroduce `scripts/export-contracts.ps1`
2. the contracts bundle stays on the Rust exporter path and does not regress back to `.NET` or PowerShell-owned logic
3. at least one known downstream expectation for current Elegy package or artifact consumption is either migrated or explicitly deprecated

Those three proofs unlock the first delete tranche more safely than starting with package deletions alone.