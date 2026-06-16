---
title: elegy-generator foundation
status: draft
owner: Elegy
created: 2026-06-16
updated: 2026-06-16
doc_kind: spec
summary: Foundation mechanics for lightweight deterministic generator definitions, validation checks, and registries.
schema_version: elegy-generator-foundation/v0
---

# elegy-generator foundation

## Problem

Future deterministic development units need a small shared definition shape
before Elegy creates concrete generator tools for UI, infrastructure, workflows,
quality gates, or release-lane scaffolding. Without a common definition shape,
each lane would invent its own identity, inputs, output intent, implementation
reference, and extension metadata.

## Goals

- Define a small `elegy-generator.*` governed contract family for generator
  definitions and validation metadata.
- Validate strict top-level contract shape while preserving namespaced
  `extensions`.
- Load loose registries from directories and resolve contracts by ID.
- Run v0.1 schema checks and report unsupported future check kinds cleanly.
- Distinguish schema validation, semantic validation, and runtime capability
  support in JSON output.

## Non-Goals

- Do not implement a concrete generator tool.
- Do not migrate `elegy plugin new` or current projection generators.
- Do not add a new `PluginTemplateKind` variant for generator packages in
  v0.1. The scaffolder lane (`elegy plugin new --template cli-tool`) and the
  future host-driven authoring lane (`elegy plugin author`, a `Generator`
  template kind, `definitionRef` resolution in the validator) are explicitly
  out of scope here and tracked as a deferred goal. See
  [`GOAL-20260616-01`](../issues/unresolved-goals.md#goal-20260616-01).
- Do not define a final taxonomy for all generator kinds, checks, backends, UI
  graphs, workflow IRs, or infrastructure templates.
- Do not make MCP export part of v0.1.
- Do not define a generic plan/materialize/receipt workflow.

## Contracts

The v0.1 contract family consists of:

| Contract | Purpose |
|---|---|
| `elegy-generator.contract-meta/v0` | Shared identity and extension shape. |
| `elegy-generator.manifest/v0` | Lightweight deterministic generator definition. |
| `elegy-generator.check/v0` | Validation check declaration. |
| `elegy-generator.registry/v0` | Optional explicit registry catalog. |

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

Schema-valid but runtime-unknown `kind` values return a warning. Unsupported
`checkKind` and backend values return structured validation output. They must
not panic, silently pass, or claim a concrete generator tool exists.

## CLI Surface

The umbrella CLI exposes:

```text
elegy generator validate <file> --json
elegy generator show <file> --json
elegy generator registry list <path> --json
elegy generator registry resolve <id> <path> --json
elegy generator check run <file> --context <path> --json
```

There is no generic generator apply command in this foundation. Concrete
generators are ordinary tools exposed through ordinary plugin packages.

## Acceptance Criteria

- [x] Valid generator fixtures pass schema validation.
- [x] Unknown top-level fields fail schema validation.
- [x] Unknown `extensions` content validates and round-trips semantically.
- [x] Unknown manifest `kind` warns without failing schema validation.
- [x] Unsupported `checkKind` returns unsupported rather than success.
- [x] Unsupported backend returns unsupported rather than success.
- [x] No generic generator planning/materialization command is advertised.
- [x] Existing plugin and projection generation behavior is unchanged.

## Validation

- `cargo test -p elegy-tooling`
- `cargo test -p elegy-cli --test generator`
- `pwsh ./scripts/validate-canonical-outputs.ps1`
- `pwsh ./scripts/validate-package-boundaries.ps1`
- `elegy-documentation --json check --project .`

Current unrelated docs-check failures must be tracked separately if present.
