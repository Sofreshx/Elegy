---
title: Adopt elegy-generator foundation
status: proposed
date: 2026-06-16
owner: Elegy
---

# Adopt elegy-generator foundation

## Context

Elegy already has several deterministic generation-adjacent paths: MCP
descriptor to skill generation, Codex plugin projection, plugin package
scaffolding, configuration materialization, and documentation creation. These
paths are useful, but they do not yet share a contract family for future
agent-invokable solved units across development work such as UI creation,
tool wrapping, infrastructure workflow scaffolding, release-lane templates, and
quality gates.

The research note
[Deterministic Development Units for Agentic Engineering](../research/deterministic-development-units.md)
sets the broader direction: repeated agent/developer work should be promoted
into governed deterministic capabilities instead of repeatedly reconstructed
from prompt context. That note is intentionally a research baseline, not a
runtime spec.

## Decision

Adopt an `elegy-generator.*` contract family and a small Rust validation
foundation under `elegy-tooling`.

- Define governed manifest, check, registry, and meta-contract schemas under
  `contracts/schemas/`.
- Treat v0.1 as definition metadata only: validation, registry loading, schema
  checks, and unsupported-backend reporting.
- Expose a thin `elegy generator ...` CLI surface from the umbrella CLI.
- Keep existing `elegy plugin new`, `elegy generate skills`, and
  `elegy generate codex-plugin` behavior unchanged.
- Use `schemaVersion` for contract identity to match current Elegy governed
  artifact conventions.
- Keep top-level schemas strict and reserve `extensions` for forward-compatible
  track-specific data.

## Alternatives

Option A: implement a concrete UI or workflow generator first. Rejected because
it would force domain-specific fields before the shared validation, registry,
and unsupported-capability semantics are stable.

Option B: make `elegy plugin new` the generator foundation. Rejected because
plugin package scaffolding is a specific existing lane, not the durable
meta-contract for solved development units.

Option C: keep the direction as research only. Rejected because the next useful
step is now small enough to govern and validate without choosing a real backend.

## Consequences

- Positive: future generator tools can share identity, extension, validation,
  and registry mechanics.
- Positive: agents get a machine-readable validation surface before any
  file-emitting generator exists.
- Positive: unsupported future backends and check kinds are represented
  explicitly instead of silently passing.
- Negative: v0.1 adds contract surface area before a real generator exists.
- Negative: follow-up work must quickly add a small vertical slice so the
  foundation does not remain abstract.

## Links

- [Generator capabilities foundation spec](../specs/elegy-generator-foundation.md)
- [Generator capabilities foundation roadmap](../roadmaps/generator-capabilities-foundation.md)
- [Deterministic Development Units for Agentic Engineering](../research/deterministic-development-units.md)
