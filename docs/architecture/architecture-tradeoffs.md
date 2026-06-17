---
title: Architecture Tradeoffs
status: active
owner: eleg
doc_kind: reference
updated: 2026-03-24
---

# Architecture Tradeoffs

## Executive summary

The current architecture is the stronger fit for Elegy as it exists today: `src\Elegy-*` are thin wrapper surfaces, not implementation centers, while `rust/` is the implementation workspace and executable surface for shared Rust behavior. The governed roots under `contracts/`, `schemas/`, and `policies/` remain the authority layer.

Removing `rust/` would not automatically improve runtime performance. It would mostly change structure, ownership, and packaging. Unless another clear implementation center replaces it, the result would likely be more churn, more ambiguity, and weaker coherence.

For `elegy-memory`, `elegy-mcp`, and `elegy-skills`, the current dedicated surfaces look like the right architecture for external agents that consume Elegy directly through the associated skill and dedicated CLI. My main caveat is that the wrapper surfaces must stay thin and should not quietly become new centers of implementation or policy.

## Current architecture advantages

### Clear separation of roles

- `src\Elegy-*` are wrapper surfaces only.
- `rust/` is the implementation workspace and executable surface for reusable Rust behavior.
- governed contract and policy roots keep the durable authority outside executable code.

That separation makes it easier to know where truth lives, where behavior lives, and where integration shells live.

### Better coherence for shared behavior

Shared runtime logic, validation, generation, and CLI behavior can be reused from one Rust workspace instead of being duplicated across multiple surfaces.

That is especially useful when the same behavior needs to power:

- the dedicated `elegy-memory` CLI
- the dedicated `elegy-mcp` CLI
- the dedicated `elegy-skills` CLI
- the broader compatibility surface

### Cleaner governance boundary

The repo can keep contracts and policies stable while letting executable behavior evolve in `rust/` without turning the wrapper surfaces into pseudo-implementations.

### Better fit for external-agent workflows

The dedicated `elegy-*` surfaces give external agents a focused entrypoint. That is simpler than requiring them to navigate a larger general-purpose interface when they only need one bounded system.

## Current architecture disadvantages / risks

### Wrapper surfaces can be mistaken for implementation centers

`src\Elegy-*` are intentionally thin, but thin shells can accumulate accidental logic over time if the boundary is not enforced.

### Rust workspace can become the new gravity well

If too much behavior is centralized in `rust/` without clear modular boundaries, the workspace can become difficult to reason about, even if the repo is structurally cleaner than before.

### More moving parts than a single-surface repo

There is more architectural surface area to maintain:

- wrapper shells
- Rust crates
- governed artifact roots
- compatibility and governance docs

That is acceptable, but it requires discipline.

## No-`rust` proposal advantages

### Smaller visible tree

Removing `rust/` would make the repository look simpler at a glance.

### Fewer language-specific conventions in the top-level layout

If all implementation were moved elsewhere, the repo would appear less divided by language-specific ownership.

### Potential fit for a fully externalized implementation center

A no-`rust` layout could make sense if Elegy were intentionally reduced to contracts, docs, and thin integration pointers while another repo or another implementation center owned the executable work.

## No-`rust` proposal disadvantages / risks

### No automatic performance gain

Removing `rust/` does not inherently make the system faster. Runtime performance comes from the implementation chosen and how it is built, not from deleting a directory.

### Higher structural churn

If `rust/` is removed without a clear replacement, the repo would need to redistribute behavior, tooling, and ownership somewhere else. That creates churn in docs, build paths, release surfaces, and contributor expectations.

### Loss of a clear executable center

Today `rust/` is the implementation workspace and executable surface for shared Rust behavior. Removing it without a replacement would weaken the repo’s ability to express where reusable execution lives.

### More risk of drift between shells and behavior

If the wrapper surfaces remain, but the implementation center disappears, the repo can end up with shells that point to nowhere, or with logic scattered across less coherent places.

### Likely worse than the current architecture unless replaced deliberately

The no-`rust` idea only works if another implementation center is defined with the same or better clarity. Otherwise it is mostly simplification by subtraction.

## Specific assessment of `elegy-memory`, `elegy-mcp`, `elegy-skills`

### `elegy-memory`

This looks like a good dedicated surface for the local memory system because it is a bounded capability with a clear consumer story. It benefits from a focused CLI and a shared implementation workspace underneath it.

### `elegy-mcp`

This is a good fit for a dedicated surface because MCP-related behavior often needs a distinct operator path, analysis flow, and contract-aware implementation. Keeping it separate helps prevent MCP-specific logic from being buried in a generic shell.

### `elegy-skills`

This also fits well as a dedicated surface because skill generation and skill-facing workflows are naturally bounded and easier to consume when they have their own explicit entrypoint.

### Overall judgment on the three systems

The three dedicated `elegy-*` surfaces appear to be the right architecture for external agents consuming the system directly via the associated skill and dedicated CLI.

#### Caveat

The caveat is that these surfaces should remain thin wrappers over shared implementation and governed contracts. If they start accumulating unique business logic, policy, or orchestration, they stop being good surfaces and start becoming hidden implementation centers.

## Recommendation / bottom line

The current architecture is the better choice.

Keep:

- `src\Elegy-*` as wrapper surfaces only
- `rust/` as the implementation workspace and executable surface for shared Rust behavior
- the governed roots as the authority layer
- the dedicated `elegy-memory`, `elegy-mcp`, and `elegy-skills` surfaces for bounded external-agent use

Do not remove `rust/` unless you already have a deliberate replacement implementation center with equal clarity and ownership. Otherwise the repo will likely gain churn, not quality, and it will not gain performance by default.
