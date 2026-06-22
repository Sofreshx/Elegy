---
created: 2026-03-24
updated: 2026-06-16
category: docs
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

Removed. Plugin packages have been retired in favor of the simplified `elegy-plugin/v1` manifest format.

## References

- [Distribution and downstream consumption](../distribution.md)
