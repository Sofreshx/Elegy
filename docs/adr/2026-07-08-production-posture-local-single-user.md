---
title: Production posture — local single-user
status: accepted
date: 2026-07-08
owner: Elegy Memory
---

# Production posture — local single-user

## Context

`elegy-memory` runs as a local CLI and MCP host, invoked by a single agent
process on a single machine. Building it against multi-tenant assumptions
(PostgreSQL, row-level security, enforced `tenant_id` isolation) up front
would add real complexity — connection pooling across processes, schema
migrations for a database it doesn't yet need, an auth model with no current
caller — for a deployment shape nothing in the repo asks for today. This ADR
picks the local single-user posture as the one to build for now, and records
what is explicitly deferred rather than silently dropped.

The alternative — designing for multi-tenant from the start — was rejected:
`tenant_id` columns already exist in the schema for forward compatibility,
but enforcing them end-to-end today would be speculative work with no
consumer to validate it against.

## Decision

`elegy-memory` deploys in a **local single-user** posture. The SQLite backend is the only supported production database. PostgreSQL and end-to-end `tenant_id` enforcement are deferred to a future multi-tenant workstream.

## Scope

### In scope (v1 production)

| Area | Decision |
|---|---|
| Database | SQLite bundled (`rusqlite` + FTS5). WAL mode. Vector storage uses a `sqlite-vec`-shaped schema (`vec0` virtual table when available, a plain-table fallback otherwise), but `sqlite-vec` is not currently a Cargo dependency, and vector search is a Rust-side scan rather than an accelerated KNN query — see `plugins/memory/docs/architecture/storage-schema.md`. |
| Concurrency | Connection pool (readers), dedicated writer. Single long-lived Tokio runtime. |
| Security | Prompt-injection hardening in LLM gate/consolidator prompts. Provider URL allow-list (no SSRF). API key via env var with redacted Debug. No tenant isolation enforcement. |
| Distribution | CLI binary (`elegy-memory`) + MCP adapters (`elegy-memory-mcp-stdio`, `elegy-memory-mcp-http`). |
| API stability | Trait-lifted capability surface. `#[non_exhaustive]` on public enums/structs. |

### Out of scope (deferred to v2+)

| Area | Reason |
|---|---|
| PostgreSQL backend | Single-user local doesn't need it. Schema design completed in docs, implementation deferred until multi-tenant requirement arises. |
| End-to-end `tenant_id` enforcement | Columns exist in schema. Enforcement (filtering all reads by tenant) deferred. Cross-agent isolation in MCP already uses `agent_id` filter. |
| Multi-tenant RLS | PostgreSQL-only feature. Deferred with Postgres backend. |
| Knowledge graph migration | Proto-graph + BFS sufficient for current scale. Full KG deferred per [plugins/memory/docs/architecture/mvp-scope.md](../../plugins/memory/docs/architecture/mvp-scope.md). |

## Not Yet Implemented

The following were part of the original design intent for v1 production
posture but do not exist in the codebase as of this writing. They are
recorded here as the target shape for a future change, not as current
behavior:

- **Durability tooling**: a CLI `integrity-check` command and a `backup --verify` path. Today durability rests on SQLite's own WAL guarantees; there is no dedicated verification command in `elegy-memory`'s CLI (`plugins/memory/src/cli.rs`).
- **Observability**: `tracing` spans and metrics on the core read/write paths (`search.duration`, `store.duration`, `gate.accept_rate`, `embedding.stale_ratio`). `RUST_LOG`-controlled logging exists; dedicated spans and metrics with these names do not yet.
- **Calibration metadata**: an eval harness (LoCoMo/FiFA/distractors) producing `{calibrated: true, confidence}` metadata stored alongside scoring parameters. No eval harness or `calibrated` field exists yet; see the separate eval-harness-v1 spec for the harness itself.

## SLOs

The following targets are the ones this posture is designed to meet once the
observability work above lands. They are targets, not currently measured
values — there is no dashboard or CI gate enforcing them today.

| SLO | Target | Measurement (once implemented) |
|---|---|---|
| Retrieval p95 | < 200ms | Tracing span `search.duration` |
| Write p95 (with embedding) | < 500ms | Tracing span `store.duration` |
| Gate acceptance rate | 60-80% | Metric `gate.accept_rate` |
| Stale embedding ratio | < 5% | Metric `embedding.stale_ratio` |
| Schema migration | Zero data loss | Snapshot + restore-from-`.elegy` before/after |

## Consequences

1. Single-user local means no horizontal scaling, no connection pooling across processes, no cloud deployment.
2. Simpler security model: no tenant isolation, no row-level security, no multi-user auth.
3. Faster iteration without multi-tenant complexity, at the cost of the durability tooling, observability, and calibration work listed above remaining unimplemented until a dedicated workstream picks them up.
4. Migration path to multi-tenant is documented but not built. Future workstream will add PostgreSQL backend + `tenant_id` enforcement + MCP auth hardening.
