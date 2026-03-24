---
created: 2026-03-24
updated: 2026-03-24
category: governance
status: active
doc_kind: reference
---

# Unresolved Goals

## Purpose

Track non-active carryover goals that remain after a workflow closes.

## Carryover Goals

### GOAL-20260324-01

Goal Statement: Make the prepared wrapper and CLI surfaces live on GitHub through remote push/tag/release execution and verify hosted distribution workflows end to end.

Status: partial / deferred by approved local-only scope.

Resume When: remote publishing is authorized, then execute the release flow and verify downloadable wrapper archives from GitHub.

Source Artifact: [Distribution and downstream consumption](../distribution.md)

Owner: workflow-orchestrator

First Seen: 2026-03-24

Last Reviewed: 2026-03-24

### GOAL-20260324-02

Goal Statement: Implement the reverse Mermaid-to-LLM-friendly-description direction as a later bounded CLI/tooling slice.

Status: deferred / not-complete.

Resume When: a later Mermaid program slice is approved to add reverse conversion in the Elegy Rust tooling surface.

Source Artifact: [rust/crates/elegy-mermaid/src/lib.rs](../../rust/crates/elegy-mermaid/src/lib.rs)

Owner: workflow-orchestrator

First Seen: 2026-03-24

Last Reviewed: 2026-03-24

### GOAL-20260324-03

Goal Statement: Add user-facing documentation for the new Mermaid CLI command and its scope boundaries.

Status: deferred / not-complete.

Resume When: a documentation lane is opened for the new `elegy mermaid render` command and its supported canonical inputs.

Source Artifact: [rust/crates/elegy-cli/src/main.rs](../../rust/crates/elegy-cli/src/main.rs)

Owner: workflow-orchestrator

First Seen: 2026-03-24

Last Reviewed: 2026-03-24

## References

- [Distribution and downstream consumption](../distribution.md)