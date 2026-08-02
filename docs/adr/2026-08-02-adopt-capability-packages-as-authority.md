---
title: Adopt capability packages as the portable authority
status: accepted
date: 2026-08-02
owner: Elegy
---

# Adopt capability packages as the portable authority

## Context

Elegy has accumulated Codex plugin envelopes, MCP hosts, skills, and local
CLI surfaces. Treating each host surface as an Elegy plugin makes reuse and
trust difficult: a developer cannot tell which artifact owns the executable,
schemas, authorization posture, or release identity.

## Decision

The reusable unit is a host-neutral capability package. A package contains a
stable JSON-first executable contract, its capability catalog, schemas,
readiness evidence, optional guidance skills, supported targets, publisher
identity, and declared entrypoints and files. `elegy-package/v1` is the
package authority and `elegy-lock/v1` is the exact reviewed selection authority
for an agent setup.

`elegy-capability-catalog/v2` remains the operation authority. It carries
stable capability IDs, CLI/MCP interface kind, input and output schemas,
readiness, side-effect classification, and invocation metadata. CLI entries
are the portability baseline.

Codex plugin manifests, generic MCP configuration, Holon registrations, and
shell-agent instructions are derived projections. They must preserve package
identity, capability IDs, schemas, side-effect classification, and artifact
digests. A projection cannot grant authorization or replace the executable
contract. Skills are optional workflow guidance and never establish runtime
behavior or permission.

Small deterministic tools should use a direct JSON CLI. Elegy's generic
CLI-to-MCP bridge is the default protocol adapter. A native MCP server is
reserved for state, subscriptions, streaming, or session-oriented behavior.
Existing `elegy-plugin/v3` packages remain readable during migration, but new
package authority must not be inferred from a Codex manifest.

## Consequences

- A tool can live in an independent repository and be packaged without joining
  Elegy's monorepo.
- Exact locks, archive and per-file digests, install receipts, and fail-closed
  projections provide a reviewable trust boundary.
- Host integrations consume packages; they do not become competing package
  authorities.
- Native binaries remain trusted code. Sandboxing untrusted third-party code
  is a separate future decision.
- Business logic, prompts, workflows, approvals, credentials, and host UX stay
  outside Elegy's package authority.

## Initial trust lane

The first trusted lane is first-party packages with clean Windows and Linux
installation evidence, exact reviewed locks, and real client proof through
direct CLI and MCP. Packages without those receipts remain non-routable even
when their schemas and tests pass.
