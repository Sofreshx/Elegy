---
title: elegy-generator foundation
status: draft
owner: Elegy
created: 2026-06-16
updated: 2026-06-16
doc_kind: spec
summary: Foundation mechanics for governed deterministic generator and solved-unit contracts, validation checks, registries, dry-run planning, and receipts.
schema_version: elegy-generator-foundation/v0
---

# elegy-generator foundation

## Problem

Future deterministic development units need shared mechanics before Elegy
chooses concrete generator backends for UI, tools, infrastructure, workflows, or
release-lane scaffolding. Without a common contract and receipt foundation, each
lane would invent its own manifest shape, validation semantics, unsupported
runtime behavior, and evidence format.

## Goals

- Define a small `elegy-generator.*` governed contract family.
- Validate strict top-level contract shape while preserving namespaced
  `extensions`.
- Load loose registries from directories and resolve contracts by ID.
- Run v0.1 schema checks and report unsupported future check kinds cleanly.
- Plan manifests as a dry run, emit receipts, and never write generated files.
- Distinguish schema validation, semantic validation, and runtime capability
  support in JSON output.

## Non-Goals

- Do not implement a concrete generator backend.
- Do not migrate `elegy plugin new` or current projection generators.
- Do not define a final taxonomy for all solved-unit kinds, checks, backends,
  UI graphs, workflow IRs, or infrastructure templates.
- Do not make MCP export part of v0.1.

## Contracts

The v0.1 contract family consists of:

| Contract | Purpose |
|---|---|
| `elegy-generator.contract-meta/v0` | Shared identity and extension shape. |
| `elegy-generator.manifest/v0` | Generator-capable or solved-unit manifest. |
| `elegy-generator.check/v0` | Validation check declaration. |
| `elegy-generator.registry/v0` | Optional explicit registry catalog. |
| `elegy-generator.receipt/v0` | Evidence from validation, checks, and planning. |

All contracts use `schemaVersion` for identity. Top-level fields are strict;
track-specific data belongs in `extensions` until promoted into a stable
contract revision.

## Runtime Behavior

`elegy-tooling` owns the reusable runtime helpers:

- load a contract JSON file
- infer the generator contract schema from `schemaVersion`
- validate against the governed JSON schema
- run semantic validation for known v0.1 expectations
- scan a directory for loose registry entries
- resolve contracts by `id`
- run `schema` checks only
- plan manifests as unsupported/dry-run receipts when no backend exists

Schema-valid but runtime-unknown `kind` values return a warning. Unsupported
`checkKind` and backend values return structured unsupported results. They must
not panic, silently pass, or claim generation happened.

## CLI Surface

The umbrella CLI exposes:

```text
elegy generator validate <file> --json
elegy generator show <file> --json
elegy generator registry list <path> --json
elegy generator registry resolve <id> <path> --json
elegy generator check run <file> --context <path> --json
elegy generator manifest plan <file> --input <k=v> --json
```

The command is `manifest plan`, not `manifest run`, because v0.1 has no
file-emitting backend.

## Acceptance Criteria

- [x] Valid generator fixtures pass schema validation.
- [x] Unknown top-level fields fail schema validation.
- [x] Unknown `extensions` content validates and round-trips semantically.
- [x] Unknown manifest `kind` warns without failing schema validation.
- [x] Unsupported `checkKind` returns unsupported rather than success.
- [x] Unsupported backend returns unsupported rather than success.
- [x] `manifest plan` produces a receipt and writes no generated files.
- [x] Existing plugin and projection generation behavior is unchanged.

## Validation

- `cargo test -p elegy-tooling`
- `cargo test -p elegy-cli --test generator`
- `pwsh ./scripts/validate-canonical-outputs.ps1`
- `pwsh ./scripts/validate-package-boundaries.ps1`
- `elegy-documentation --json check --project .`

Current unrelated docs-check failures must be tracked separately if present.
