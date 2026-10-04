---
title: Elegy Memory architecture
status: active
owner: Elegy Memory
doc_kind: index
---

# Elegy Memory architecture

Memory is a local Rust memory engine and CLI backed by SQLite. Its source
implements storage, retrieval, salience gating, correction, consolidation and
evaluation. Its current readiness is **implemented**, not agent-routable;
source tests do not prove installed-host value or live-provider operation.
See the [readiness manifest](../../readiness.json) and
[qualification guide](../qualification.md) for evidence.

For development commands and source/test ownership start with
[CONTRIBUTING.md](../../CONTRIBUTING.md). This page explains the boundaries
needed to choose the right implementation path.

## Three persistence paths

| Path | Owner | Purpose and boundary |
| --- | --- | --- |
| SQLite memory engine | [`SqliteMemoryStore`](../../src/storage/sqlite_store.rs), [`schema.rs`](../../src/storage/schema.rs) | Stores `Memory` rows, embeddings, versions, links and learned retrieval data in an explicitly selected database. A store has one write scope; search visibility can include broader scopes. |
| Governed JSON artifacts | [`artifacts.rs`](../../src/artifacts.rs), [`LocalMemoryStore`](../../src/local_store.rs) | Validated summary-only envelopes, provenance, lifecycle and deterministic projections stored as local JSON artifacts. This is separate from the SQLite retrieval engine. Existing public types are re-exported from `lib.rs`. |
| Contextual-recall journal | [`recall.rs`](../../src/recall.rs), [`recall_store.rs`](../../src/storage/recall_store.rs) | Reads the source database without migration and records bounded events/feedback in a separate journal. It does not modify source memories or persist raw prompts/transcripts. |

## Runtime flow

The [CLI](../../src/cli.rs) parses commands, chooses the explicit database and
scope, wires optional providers, and formats output. Write workflows use the
[salience gate](../../src/gate.rs) before applying their documented
disposition. The [store](../../src/storage/sqlite_store.rs) owns persistence,
retrieval ranking, corrections and feedback learning; the [model](memory-model.md)
explains their semantics. Do not mistake the lower-level `MemoryStore::store`
method for the complete gate-aware write workflow.

Embeddings and optional LLM calls sit behind [traits](traits-and-interfaces.md).
Provider failures must preserve the documented write/fallback behavior.
SQLite is the implemented storage backend; no PostgreSQL store is present.

[`hosts/memory-mcp`](../../../../hosts/memory-mcp/AGENTS.md) owns the optional
MCP transport and host-specific authorization. The Python
[Codex hook](../../integrations/codex/README.md) invokes the existing recall CLI.
Neither boundary is required for core Memory development.

## Read only what the change needs

- [Feature matrix](mvp-scope.md): implemented, deferred and unsupported work.
- [Memory model](memory-model.md): scopes, ranking, confidence, decay and writes.
- [Storage schema](storage-schema.md): SQLite tables, vector layout and journal.
- [Migration framework](migration-framework.md): preservation and cutover rules.
- [Interface map](traits-and-interfaces.md): traits, concrete APIs and source symbols.
- [Contextual recall spec](../../../../docs/specs/memory-contextual-recall-v1/spec.md): binding, selection, privacy and event-bound feedback.
- [Eval spec](../../../../docs/specs/eval-harness-v1/spec.md) and
  [qualification guide](../qualification.md): regression checks and durable claims.

These local explanations follow the [repository architecture authority](../../../../docs/architecture/README.md),
ADRs and applicable specs. They do not grant a new milestone or readiness level.
