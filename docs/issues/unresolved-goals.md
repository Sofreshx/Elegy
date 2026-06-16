---
created: 2026-03-24
updated: 2026-06-16
category: governance
status: active
doc_kind: reference
---

# Unresolved Goals

## Purpose

Track non-active carryover goals that remain after a workflow closes.

## Carryover Goals

### GOAL-20260324-01

Goal Statement: Keep the hosted distribution lane healthy by continuously verifying that push, tag, and release execution refresh GitHub Release assets end to end.

Status: active.

Resume When: the hosted publish lane drifts, `main-snapshot` stops tracking `main`, or downloadable release assets need to be revalidated after workflow changes.

Source Artifact: [Distribution and downstream consumption](../distribution.md)

Owner: workflow-orchestrator

First Seen: 2026-03-24

Last Reviewed: 2026-05-28

### GOAL-20260616-01

Goal Statement: Expose new-plugin authoring as a programmable, host-driven capability so agent harnesses (such as Holon) can help users create plugin packages end to end — proposing skill identity and capability names, suggesting side-effect classes from skill metadata, scaffolding the manifest, skill, and package JSON together, and iterating on validator feedback until the package verifies. The current `elegy plugin new` CLI is a one-shot scaffold for humans in a coding environment; it has no decision-support, no validation loop, and no in-harness entry point. Holon (or any agent host) cannot today offer its user "create a new plugin package from this skill idea" as a real workflow.

Status: deferred. The current schema, validator, CLI, and reference fixtures are sufficient for human authoring in a coding environment. Authoring-as-a-host-capability work is intentionally out of scope for the current plugin package consolidation slice.

Why this matters:

- A harness user who wants a new plugin today must hand-author the package JSON, skill v2, generator manifest, and tool requirements. The harness can help narratively but has no tool surface to drive the creation.
- A new plugin loop is the missing bridge between the existing generator definition foundation and the actual arrival of file-emitting generator consumers (RM-generator-capabilities-foundation-002). Without it, the foundation has no use path.
- Iterating on validator findings is the most expensive step today. A harness that can run `elegy plugin verify` against its own authoring actions and present structured errors back to the user turns a multi-file hand-edit into a conversation.

Resume When: at least one of the following signals appears:

- Holon (or any other host) explicitly requests a plugin-authoring capability.
- A first file-emitting generator consumer (RM-generator-capabilities-foundation-002) lands and needs authoring tooling to be exercisable from inside a host.
- The `elegy plugin new` template surface gains a new template kind for generator-backed plugins.
- A user asks the host "create a new plugin" and the host has no tool to call.

A future implementation will likely add:

- A first-class authoring tool lane (for example, `elegy plugin author` or `elegy plugin new --author` and a corresponding `elegy plugin doctor`) that:
  - Scaffolds plugin JSON + skill v2 + generator manifest + instruction skill in one shot.
  - Walks the user (or harness agent) through capability IDs, side-effect class, tool binary, and generator backend reference with sensible defaults derived from the referenced skill.
  - Runs `elegy plugin verify` on the result and returns structured, actionable findings.
  - Supports an iterative edit-and-re-verify loop without forcing the user to re-scaffold.
- A new `generator` template kind under `PluginTemplateKind` for the one-shot scaffold path.
- A host-callable surface (CLI subcommand, JSON envelope, and optionally an MCP tool) so harnesses like Holon can drive the workflow from their own UI.
- Validator coverage of `definitionRef` resolution so R2.3 (subset marker) and R2.5 (side-effect tightening) fire on production-shape packages, not just inline ones — the iteration loop only earns its keep when the validator's verdicts are trustworthy.
- A clear, machine-readable authoring contract so the host does not have to re-derive schema or convention details. The existing schema, model doc, generator convention, and reference package already cover this; the gap is an explicit "authoring API" surface rather than ad-hoc human reading.

Source Artifacts: [Elegy Plugin Package Model](../architecture/elegy-plugin-package-model.md), [Generator-As-Plugin Convention](../specs/generator-backed-plugin-convention.md), [Elegy Plugin Package V1 Unification ADR](../adr/2026-06-16-elegy-plugin-package-v1-unification.md), [Generator Capabilities Foundation roadmap](../roadmaps/generator-capabilities-foundation.md), [Holon plugin install path](../specs/host-neutral-plugin-install.md), [Agent integration doc](../agent-integration.md)

Owner: elegy-tooling (in coordination with whoever owns the host harness that will consume this)

First Seen: 2026-06-16

Last Reviewed: 2026-06-16

## References

- [Distribution and downstream consumption](../distribution.md)
- [Elegy Plugin Package Model](../architecture/elegy-plugin-package-model.md)
- [Generator-As-Plugin Convention](../specs/generator-backed-plugin-convention.md)
- [Elegy Plugin Package V1 Unification ADR](../adr/2026-06-16-elegy-plugin-package-v1-unification.md)
- [Generator Capabilities Foundation roadmap](../roadmaps/generator-capabilities-foundation.md)
