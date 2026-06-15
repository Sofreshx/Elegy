# .claude/rules/storage.md for Elegy
# This rule applies only when Claude touches memory storage files.

---
paths:
  - "rust/crates/elegy-memory/src/storage/**"
---

You are working on the SQLite storage layer for `elegy-memory`.

Check coherence with @rust/crates/elegy-memory/docs/architecture/storage-schema.md.
The schema is managed in `schema.rs`; schema changes require an explicit human confirmation before editing.
Migrations belong in `ensure_schema()`, not ad hoc SQL elsewhere.
All database access should flow through the `MemoryStore` trait boundary defined in `traits.rs`.
Do not store raw transcripts, bypass salience/provenance, or mix memory scopes implicitly.
