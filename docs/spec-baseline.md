---
created: 2026-03-19
updated: 2026-03-21
category: architecture
status: active
doc_kind: reference
---

# MCP Spec Baseline

## Purpose

Record the protocol baseline Elegy is targeting so governed contracts, exported bundles, and Rust tooling do not drift implicitly.

## Context

Elegy is currently centered on two implementation anchors:

- governed contract and policy artifacts under `contracts/`, `governance/`, `schemas/`, and `policies/`
- the first-party Rust workspace under `rust/`

Protocol-baseline drift would now affect:

- published schemas, fixtures, compatibility manifests, and bundle exports
- Rust contract consumers and operator crates such as `elegy-tooling`, `elegy-cli`, and `elegy-host-mcp`
- downstream consumers that rely on the exported contract bundle or Rust executable surfaces

## Baseline

Elegy is pinned to the **Model Context Protocol specification dated `2025-11-25`** for the current implementation baseline.

This means:

- documentation should refer to `2025-11-25` when describing supported MCP behavior
- future implementation work should not silently target `latest`
- governed contract artifacts and Rust operator surfaces should keep the same declared protocol target
- the exported contract bundle should stay aligned with that baseline

The currently implemented slice is still intentionally narrower than the full spec:

- the current contributor-facing executable slice is the Rust `elegy` CLI `author`, `analyze`, and `generate` path backed by `rust/crates/elegy-tooling`
- `elegy-host-mcp` exists as a thin stdio host over runtime-composed resources
- resources-first behavior remains the current live runtime posture
- no implied support for prompts, sampling, autonomous MCP-native self-authoring, or built-in skill-driven orchestration unless documentation and implementation are updated together

## Upgrade policy

Spec upgrades are **explicit decisions**, not routine dependency drift.

Before changing the declared MCP baseline:

1. review the upstream MCP release and changelog
2. confirm the change is worth the migration cost
3. verify `elegy-tooling`, `elegy-cli`, and any runtime-host behavior still match the required feature set
4. verify exported contract artifacts and compatibility notes remain coherent
5. update docs, contract exports, and validation flows together
6. record the new baseline deliberately rather than treating it as an incidental dependency bump

Until that happens, the repository baseline remains `2025-11-25`.

## Related baselines

- contract authority: `contracts/` and `governance/`
- exported machine-readable handoff: `artifacts/contracts`
- current operator slice: Rust CLI author/analyze/generate with narrow validation and inspection flows under `rust/`
- runtime model: runtime composition with a resources-first posture
- future target: broader MCP-hosted or skill-driven self-authoring only after it is implemented and validated in-repo
- OSS license baseline: Apache-2.0

## References

- [Architecture overview](architecture/README.md)
- [Distribution and downstream consumption](distribution.md)
- [Repository README](../README.md)
- [Rust workspace README](../rust/README.md)