---
created: 2026-03-19
updated: 2026-03-19
category: architecture
status: active
doc_kind: reference
---

# MCP Spec Baseline

## Purpose

Record the protocol baseline the Elegy repository is targeting during bootstrap and consolidation so runtime and contract work do not drift implicitly.

## Context

`Elegy` now contains both the authoritative `.NET` formalization families and the first-party Rust runtime workspace.

That means protocol-baseline drift would now affect:

- public documentation
- governed `.NET` contract artifacts exported from `src/`
- Rust runtime/host behavior under `rust/`
- downstream consumers that depend on the exported compatibility and schema bundle

## Baseline

Elegy is pinned to the **Model Context Protocol specification dated `2025-11-25`** for the current implementation baseline.

This means:

- documentation should refer to `2025-11-25` when describing supported MCP behavior
- future implementation work should not silently target `latest`
- the `.NET` formalization families and Rust runtime family should keep the same declared protocol target
- the exported contract bundle should stay aligned with that baseline

The currently implemented slice is still intentionally narrower than the full spec:

- resources-first behavior
- listing and reading behavior first in the live runtime path
- no implied support for tools, prompts, or other MCP surfaces unless documentation and implementation are updated together

## Upgrade policy

Spec upgrades are **explicit decisions**, not routine dependency drift.

Before changing the declared MCP baseline:

1. review the upstream MCP release and changelog
2. confirm the change is worth the migration cost
3. verify the Rust SDK and project implementation still match the required feature set
4. verify exported contract artifacts and compatibility notes remain coherent
5. update docs, tests, and workflows together
6. record the new baseline deliberately rather than treating it as an incidental dependency bump

Until that happens, the repository baseline remains `2025-11-25`.

## Related baselines

- implementation direction: Rust-first where runtime behavior dominates
- contract authority: `.NET` package families under `src/`
- runtime model: runtime composition
- current live runtime surface: resources-first MCP behavior
- OSS license baseline: Apache-2.0

## References

- [Architecture overview](architecture/README.md)
- [Repository README](../README.md)
- [Rust workspace README](../rust/README.md)