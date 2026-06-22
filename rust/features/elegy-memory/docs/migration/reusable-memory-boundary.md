# Reusable Memory and Context Boundary

## Purpose

This document defines the WU-001 boundary that now underpins the shipped `elegy-memory` V1 family.

`Elegy` is limited to local, non-authoritative artifact-management semantics for governed reusable memory artifacts. `SAASTools` remains the runtime and policy authority.

The existing `summary-only-session-context-envelope` remains unchanged. The shipped memory surface and its governed skill authority chain are documented in `../architecture/v1.md`. This migration note keeps the extraction boundary and release posture separate from that implementation-facing entrypoint.

## Placement rule

1. Portable schemas, fixtures, manifests, compatibility metadata, and support metadata for reusable memory artifacts may live in `Elegy`.
2. Local artifact-management semantics in `Elegy` are limited to import, list, show, export, supersede, and tombstone operations over governed artifacts.
3. Those semantics are non-authoritative. They may describe local lineage or local artifact state, but they do not establish currentness, approval, freshness policy, retrieval ranking, runtime validity, or promotion.
4. Retrieval pipelines, persistence stores, approval gates, frontmatter parsing, freshness policy, retrieval ranking, runtime validation, promotion, and production currentness remain in `SAASTools`.
5. Contract and governance extraction comes before runtime extraction. Do not widen this phase into runtime/store ownership, broad mutation flows, or skill-authority changes.

## Shipped Elegy-memory V1 family

The shipped `elegy-memory` V1 family is intentionally narrow:

- portable contract and support metadata for reusable memory artifacts
- local non-authoritative import semantics
- local non-authoritative list and show semantics
- local non-authoritative export semantics
- local non-authoritative supersede and tombstone semantics

This planned family does not add or imply:

- current-artifact selection
- approval inference or approval policy
- freshness policy or freshness enforcement
- retrieval ranking or host context shaping
- runtime validation authority
- promotion authority or production promotion workflow
- persistence-store ownership or broad mutation workflows

## What belongs in Elegy vs stays in SAASTools

| Concern | Placement | Notes |
| --- | --- | --- |
| Portable reusable-memory artifacts, fixtures, manifests, and support metadata | `Elegy` | Governed artifacts may live under `contracts/`. |
| Local import, list, show, export, supersede, and tombstone semantics | `Elegy` | These semantics are local and non-authoritative only. |
| Compatibility manifests and fixture corpus | `Elegy` | This is governed evidence, not app logic. |
| Currentness, approval, freshness policy, and retrieval ranking | `SAASTools` | These remain host-owned policy decisions. |
| Runtime validation and promotion authority | `SAASTools` | `Elegy` does not become operational authority. |
| Persistence stores, mutation workflows, and product runtime integration | `SAASTools` | These remain product/runtime responsibilities. |
| Skill authority and `SKILL.md` governance | `Elegy` governed artifacts | Governed definition remains authoritative, discovery remains derived, and rendered `.github/skills/elegy-memory/SKILL.md` stays non-authoritative. |

## Local non-authoritative artifact-management semantics

Within this boundary, the planned meanings are:

- `import`: ingest a governed artifact copy into a local `Elegy` surface without making it current, approved, fresh, or promotable
- `list`: enumerate locally present governed artifacts and their declared lineage metadata
- `show`: inspect a specific governed artifact and its declared lineage metadata
- `export`: emit a portable copy of a governed artifact and its local lineage metadata
- `supersede`: record local lineage that one local artifact copy supersedes another, without deciding runtime currentness
- `tombstone`: record local withdrawal of a local artifact copy, without invalidating runtime use in `SAASTools`

These are artifact-management semantics only. They are not host runtime decisions.

## Reserved to SAASTools

`SAASTools` explicitly retains authority for:

- currentness
- approval
- freshness policy and freshness enforcement
- retrieval ranking and runtime context shaping
- runtime validation
- promotion and production promotion
- invalidation for runtime use

No `Elegy` artifact, mirror, or local adapter may override those decisions.

## Release posture

The next memory-family change, when introduced, should be an additive minor release on the existing 1.x line because it adds bounded local artifact-management vocabulary without changing the existing `summary-only-session-context-envelope` or moving runtime authority out of `SAASTools`.

A major bump is reserved for a real breaking change to an already published contract or support promise.

## Non-claims

- This does not claim that `Elegy` owns retrieval, ranking, validation, promotion, or persistence.
- This does not authorize broad workspace or user-memory mutation.
- This does not change the existing summary-only session-context envelope.
- This does not make rendered `SKILL.md` output authoritative.
- This does not add runtime/store/CLI implementation beyond boundary and governance updates.

## Related

- [Elegy-memory V1](../architecture/v1.md)
- [Extraction Matrix](extraction-matrix.md)
- [Research note: memory retention and removal guidance](../research/retention-removal.md)
- [Elegy Substrate Governance](../architecture/substrate-governance.md)