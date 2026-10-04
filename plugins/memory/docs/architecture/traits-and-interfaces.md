---
title: Memory interfaces and extension points
status: active
owner: Elegy Memory
doc_kind: guide
---

# Memory interfaces and extension points

The signatures live in [traits.rs](../../src/traits.rs), the data model in
[types.rs](../../src/types.rs), and failures in [error.rs](../../src/error.rs).
This guide explains which boundary to extend instead of maintaining a second
copy of the Rust declarations. Generate searchable API documentation with
`cargo doc --locked -p elegy-memory --no-deps` from the repository root.

## Traits

| Trait | Responsibility | Current implementations / source |
| --- | --- | --- |
| `MemoryStore` | Scoped CRUD, metadata patches, retrieval, embeddings, lifecycle, contradictions, health and purge | [`SqliteMemoryStore`](../../src/storage/sqlite_store.rs) |
| `EmbeddingProvider` | Embedding generation, batch fallback, dimensions and model identity | [embedding module](../../src/embedding/mod.rs), OpenAI/Ollama adapters and circuit breaker |
| `LlmProvider` | Optional completion for contradiction classification and consolidation | [LLM module](../../src/llm/mod.rs), OpenAI/Ollama adapters |
| `SalienceGate` | Return an explicit write disposition for a candidate and existing store | [`DefaultSalienceGate`](../../src/gate.rs) |
| `MemoryConsolidator` | Propose consolidation actions | [`SimpleConsolidator` / `LlmConsolidator`](../../src/consolidator.rs) |
| `MemoryObservability` | Synchronous host-facing health, contradictions, export and purge | [`SqliteMemoryStore`](../../src/storage/sqlite_store.rs) |
| `ForgettingPolicy` | Rank memories for budget eviction using `RetentionContext` | [forgetting policies](../../src/forgetting.rs); budget execution stays in the store |

`MemoryStore::get` tracks retrieval; `get_raw` is the non-tracking lookup.
`list` is an exact-scope inventory; search and duplicate detection use the
visibility rules in the [memory model](memory-model.md). A trait method is
not automatically a complete CLI write workflow: keep salience/provenance
handling in the caller path that owns the operation.

`MetadataUpdate` distinguishes an omitted field from `OptionalFieldUpdate::Clear`
and `Set(value)`. `MemoryFilter` is the list filter; `SearchQuery` is the
retrieval query. `GateDecision` and `ConsolidationAction` encode outcomes
rather than assuming every input becomes a new active memory.

## Concrete SQLite APIs

These capabilities are implemented on `SqliteMemoryStore`, not on the
`MemoryStore` trait. Use their [source](../../src/storage/sqlite_store.rs)
and [unit tests](../../src/storage/sqlite_store/tests.rs) when changing them.
Avoid expanding a shared trait solely to mirror the concrete implementation.

| Capability | Symbols to find | Additional validation |
| --- | --- | --- |
| Correction and history | `correct_memory`, `list_corrections`, `list_versions`, `rollback_to_version` | `--test cli`, `--test integration` |
| Corroboration and scope promotion | `corroborate`, `promote_memory_to`, `run_promotion_pass` | `--test cli`, `--test integration` |
| Feedback and learned weights | `record_feedback`, `compute_learned_weights`, `scope_config` | `--test cli`, `--test eval` |
| Budget and eviction policy | `enforce_budget`, `enforce_budget_with_policy` | `--test qualification`, `--test cli` |
| Links and graph traversal | `record_link`, `list_links`, `delete_link`, `traverse_links` | `--test cli` |
| Poisoning detection | `detect_poisoning`; CLI quarantine orchestration is in [cli.rs](../../src/cli.rs) | `--test cli` |
| Sharing | `export_for_sharing`, `import_shared` | `--test cli` |

CLI output types can contain presentation fields beyond the store's core types.
Changes to JSON envelopes also require [conformance tests](../../tests/conformance.rs).
Provider defaults and timeout values are defined next to each adapter, not here.

## Separate artifact and recall interfaces

[artifacts.rs](../../src/artifacts.rs) owns summary-only envelopes, governed
records, provenance, local lifecycle, validation and deterministic projections.
[local_store.rs](../../src/local_store.rs) persists those JSON artifacts.
They do not implement SQLite retrieval or the `MemoryStore` trait. Root
re-exports keep existing `elegy_memory::…` import paths stable.

[recall.rs](../../src/recall.rs) and [recall_store.rs](../../src/storage/recall_store.rs)
implement the separate contextual-recall source reader and event journal;
the [recall spec](../../../../docs/specs/memory-contextual-recall-v1/spec.md)
owns their contract. Do not reuse legacy `record_feedback` for event-bound
recall feedback: the latter must not mutate source memories or learned weights.

Use the [contributor validation matrix](../../CONTRIBUTING.md#validate-the-change)
to select the command, then run the full Memory suite for a Rust/API change.
