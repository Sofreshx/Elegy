# Reusable Memory and Context Boundary

## Purpose

This document gives the phase-1 reusable memory/context effort an explicit home in `Elegy`.

It defines the bounded split between neutral reusable memory authority that can later sit behind a CLI or other host-agnostic surface and the host-owned runtime behavior that remains in `SAASTools`.

This is a planning and governance boundary, not a claim that a full reusable memory runtime already exists in `Elegy`.

## Placement rule

1. If the surface is a portable schema, fixture, manifest, compatibility artifact, or neutral provenance/freshness vocabulary that multiple hosts must consume consistently, it may belong in `Elegy`.
2. If the surface depends on Holon or `SAASTools` retrieval policy, persistence, approvals, frontmatter parsing, promotion workflow, or product/runtime integration, it stays in `SAASTools`.
3. Contract extraction comes before runtime extraction. Do not move a retrieval engine or broad mutation flow into `Elegy` before the governed artifact family is explicit and validated.

## Phase 1 home in Elegy

For phase 1, `Elegy` is the bounded home for reusable memory/context work in these forms:

- governed schema families for portable memory/context records, summaries, or promotion descriptors
- fixtures and manifests that prove portability and compatibility
- neutral provenance, freshness, supersession, and invalidation vocabulary when those semantics must cross runtime boundaries
- a later thin CLI-capable neutral substrate only after the contract family is explicit and the executable need is proven

## What belongs in Elegy vs stays in SAASTools

| Concern | Placement | Notes |
| --- | --- | --- |
| Portable memory/context envelopes | `Elegy` | Governed artifacts under `contracts/` if multiple hosts must consume the same shape. |
| Provenance, freshness, supersession, and invalidation metadata that travel across boundaries | `Elegy` | Keep the vocabulary neutral; host enforcement still stays local. |
| Compatibility manifests and fixture corpus | `Elegy` | This is governed evidence, not app logic. |
| Host retrieval pipeline, ranking, and context shaping | `SAASTools` | Retrieval remains host-owned and product-shaped. |
| Persistence stores, promotion decisions, and mutation workflows | `SAASTools` | These remain product/runtime responsibilities. |
| Approvals, frontmatter parsing, and policy gates | `SAASTools` | These are host and product behaviors, not neutral contract authority. |
| Desktop app integration, session continuity, and user-facing runtime surfaces | `SAASTools` | Keep UI and runtime orchestration local. |
| Broad workspace or user memory CLI mutation | `SAASTools` for now | Not part of the phase-1 `Elegy` scope. |

## First contract-family direction

The first bounded contract family should target portable promoted-memory artifacts rather than a full memory engine.

Start with a small artifact family such as:

- a portable memory or context item envelope
- provenance, freshness, supersession, and invalidation metadata blocks
- a promotion or scope descriptor that classifies an item for work-unit, workspace, or user-level use
- validation-state and approval metadata that can travel with promoted artifacts without moving validation authority out of `SAASTools`

Keep turn-local retrieval state, transcript indexing, storage layout, approvals, and mutation flows out of this first family.

The initial Rust landing zone for typed portable summary/context artifacts is `rust/crates/elegy-memory`, which should stay limited to neutral models and bounded validation helpers for governed reusable artifacts.

The first executable landing zone is the existing `rust/crates/elegy-cli` surface, but only as a thin read-only validation and inspection layer over those governed artifacts.

## First bounded CLI slice

The first bounded CLI slice is now explicitly limited to read-only summary-only session-context inspection and validation.

- primary command: `elegy validate session-context --input <path>`
- companion contract view: `elegy inspect session-context`
- accepted input: JSON for the governed `summary-only-session-context-envelope` only
- validation mechanism: `elegy-memory` validated types and bounded field checks
- output modes: bounded text and JSON reports

This slice does not add or imply:

- mutation, promotion, persistence, or invalidation behavior
- resume orchestration, host retrieval, or current-artifact selection
- approval inference, freshness decisions, or host policy enforcement
- transcript-bearing payload support

## Frozen phase-1 authority notes

- `SAASTools` remains the planning and runtime authority for creation, validation, promotion, supersession decisions, freshness enforcement, and invalidation actions.
- `elegy validate session-context` is a neutral artifact-shape validator, not a host validation authority. A passing result does not make an artifact current, approved, promotable, or non-stale.
- `Elegy` provides neutral vocabulary and contract shapes for durable artifacts above turn scope, but it does not become the source of truth for host retrieval or policy.
- The phase-1 work-unit summary is treated as a distinct durable artifact, not merely a projected session summary, because resume and supersession chains need a stable portable shape.
- User-scope promotion requires explicit per-item approval in phase 1; `Elegy` may define the approval-bearing artifact shape, but it does not grant or infer approval.
- File-backed or mirrored adapters may serialize governed artifacts, but those adapters remain non-authoritative and cannot override host-owned validation or invalidation.

## Validation checkpoints and adapter posture

The phase-1 sequence is:

1. `SAASTools` authors or exports candidate governed summary-only artifacts.
2. `Elegy` validates only the neutral bounded artifact shape.
3. `SAASTools` re-validates before runtime use, promotion, or invalidation decisions.
4. Optional adapters may mirror or transport the artifact, but they do not become the source of truth.

Adapter posture is intentionally conservative:

- file-backed, mirrored, and bridge adapters are non-authoritative
- read-only inspect/import/export flows are acceptable
- mutation-capable adapter flows are out of scope for phase 1
- no adapter may infer approval, currentness, freshness, supersession, or invalidation on behalf of `SAASTools`

## Non-claims

- This does not claim that `Elegy` already owns Holon retrieval or persistence.
- This does not authorize broad CLI mutation of workspace or user memory.
- This does not assume embeddings, vector storage, or RAG as required substrate.
- This does not move current-state truth out of `SAASTools/docs/system/**`.

## Related

- [Extraction Matrix](extraction-matrix.md)
- [Elegy Substrate Governance](../architecture/substrate-governance.md)