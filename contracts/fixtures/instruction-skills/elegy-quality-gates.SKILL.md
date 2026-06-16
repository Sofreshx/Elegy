---
name: elegy-quality-gates
description: Use when reviewing the draft quality-gates generator fixture and future deterministic generator-as-plugin direction.
---

# Elegy Quality Gates

> Use when reviewing the draft quality-gates generator fixture and future
> deterministic generator-as-plugin direction.

This is a reference-only generator-as-plugin fixture. It intentionally does not
advertise a callable quality-gates command yet. A future quality-gates generator
will be a normal tool exposed through a normal plugin package.

## Quick Start

1. Inspect the package fixture:
   `elegy plugin verify --package contracts/fixtures/elegy-plugin-package.elegy-quality-gates.json --json`.
2. Inspect any draft generator definition:
   `elegy generator validate <definition> --json`.
3. Use only the declared validation capability until a concrete file-creating
   quality-gates generator exists.

## Guardrails

- Generators are tools, not a special Elegy runtime lane.
- Generator definitions are lightweight authoring metadata, not execution
  authority.
- Generated workflows are created by concrete generator tools. Do not invent a
  generic plan/materialize command path.
- Generated workflows are derived outputs. Do not treat them as plugin package
  authority.
- Backend-specific generator metadata belongs in the generator definition, not
  in the plugin package.

## Capability Index

| id | side-effect | purpose |
| -- | -- | -- |
| `quality-gates-definition-validate` | read-only | Validate a draft quality-gates generator definition |

## References

- Package fixture:
  `contracts/fixtures/elegy-plugin-package.elegy-quality-gates.json`
- Skill fixture:
  `contracts/fixtures/skill.elegy-quality-gates.json`
- Convention:
  `docs/specs/generator-backed-plugin-convention.md`
- Generator foundation:
  `docs/specs/elegy-generator-foundation.md`
