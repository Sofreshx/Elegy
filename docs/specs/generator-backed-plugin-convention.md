---
spec_id: generator-backed-plugin-convention
title: Generator-As-Plugin Convention
status: draft
type: contract
owner: Elegy
created: 2026-06-16
updated: 2026-06-16
doc_kind: spec
summary: Convention for packaging deterministic generator tools as ordinary Elegy plugin capabilities without creating a special generator runtime lane. The current authoring flow is hand-edited and dev-driven; a polished host-driven authoring lane is tracked as a deferred goal.
---

# Generator-As-Plugin Convention

## Authoring Lane: Current vs. Future

The plugin package contract is settled. The current authoring flow is
hand-edited and dev-driven: an author writes the skill v2, the manifest, and
the package JSON against the schema, then runs `elegy plugin verify` and
`elegy generator validate` and iterates. The `elegy plugin new` command is a
one-shot scaffolder for the starter file set; it is **not** a full authoring
tool.

A polished, host-driven authoring lane (`elegy plugin author` or
`elegy plugin doctor`, a `generator` template kind, `definitionRef` resolution
in the validator, structured error codes) is tracked as a deferred goal. See
[GOAL-20260616-01](../issues/unresolved-goals.md#goal-20260616-01).

The rest of this spec describes the contract: a generator-backed plugin is an
ordinary plugin package plus a generator manifest. It does not describe how a
host creates one — that authoring lane is the deferred goal.

## Problem

Elegy now has a lightweight generator definition foundation and a portable
plugin package model. Generator tools should use the normal plugin path instead
of creating a special execution lane. If every generator invents its own package
shape, agents will get inconsistent discovery and install behavior.

## Decision

Generator-backed plugins use the existing `elegy-plugin-package/v1` package
shape. The package describes capability discovery, invocation, side effects,
tool requirements, documentation, and host projection. The generator definition
describes the deterministic tool's purpose, inputs, outputs, implementation
reference, compatibility, and extensions.

Do not add generator-specific fields to `elegy-plugin-package/v1` for this
slice. Backend-specific generator metadata belongs in the generator definition
and the concrete generator tool, not in the package schema.

## Package Pattern

A generator-as-plugin package contains:

- `components.skillDefinitions[]` with a governed skill definition for the
  generator capability.
- `components.instructionSkills[]` with agent guidance for safe use.
- `components.capabilityProjections[]` for callable generator capabilities
  that actually exist.
- `components.toolRequirements[]` for the concrete tool binary.
- `components.docs[]` pointing to the generator and plugin convention docs.
- `hostPolicyHints` matching the strongest side effect exposed by the package.

The reference package is
`contracts/fixtures/elegy-plugin-package.elegy-quality-gates.json`.

## Capability Rules

Generator-backed plugin capabilities are ordinary tool capabilities.

- Do not advertise a generator capability until the command/tool exists.
- File-creating generators declare their side effects through the existing skill
  and package side-effect metadata.
- Do not invent generic plan/materialize commands for generators.
- Capability projections must match the referenced skill definition's
  `hostProjection.capabilityProjections[]`.
- Generated files are derived outputs. They do not become plugin package
  authority and must be reproducible from the generator definition plus concrete
  tool implementation.

## Fixture Posture

Maintained plugin package fixtures use package-local component paths. A package
located under `contracts/fixtures/` references sibling skill definitions as
`skill.<name>.json` and package-local instruction skills under
`instruction-skills/`.

This keeps `elegy plugin verify`, host projection, and portable package export
behavior aligned. Repository-root paths such as `contracts/fixtures/...` must
not be used in `definitionRef` for maintained package fixtures.

## Validation

Use the standard package and generator validation stack:

```text
elegy plugin verify --package <package> --json
elegy generator validate <definition> --json
cargo test -p elegy-contracts
```

## Authoring Path

Today, authoring a generator-backed plugin is a hand-edited flow. The
`elegy plugin new` scaffolder writes a starter file set; the rest is the
author's job against the schema, the generator manifest schema, and the
reference fixture.

1. Scaffold a starter: `elegy plugin new --template cli-tool --output ./my-plugin`
2. Author the skill definition (governed v2) carrying the generator capability.
3. Write the generator manifest (`elegy-generator.manifest/v0`) describing
   the deterministic tool.
4. Edit the package JSON to declare capability projections, tool
   requirements, side effects, and policy hints.
5. Run `elegy plugin verify` and `elegy generator validate`, then iterate.
6. For host projection evidence: `elegy generate codex-plugin --package
   <package> --output-dir <dir> --force`.

The reference package
[`contracts/fixtures/elegy-plugin-package.elegy-quality-gates.json`](../fixtures/elegy-plugin-package.elegy-quality-gates.json)
demonstrates the full shape. See
[Authoring Path in the Plugin Package Model doc](../architecture/elegy-plugin-package-model.md#setup-flow)
for the same flow in more detail.

The `elegy plugin new` scaffolder and the future host-driven authoring lane
(see [Authoring Lane: Current vs. Future](#authoring-lane-current-vs-future)
above and
[GOAL-20260616-01](../issues/unresolved-goals.md#goal-20260616-01)) do not
conflict — they serve different purposes. `plugin new` writes a starter file
set; the future `elegy plugin author` lane would walk the user (or a
harness) through the full authoring decisions and drive the verify loop.