# Elegy Memory

## Start Here

- Read `plugins/memory/docs/architecture/mvp-scope.md` before implementing memory behavior.
- Read `plugins/memory/docs/architecture/memory-model.md` when changing scopes, scoring, decay, confidence, provenance, or state transitions.
- Read `plugins/memory/docs/architecture/storage-schema.md` before changing persistence.

## Boundaries

- This crate owns bounded local memory behavior and persistence. Host policy such as approval, promotion, freshness/currentness, and retrieval ranking stays outside this crate.
- Store distilled memories only. Never persist raw transcripts.
- Every memory needs provenance; do not create bypass writes around provenance or salience.
- Keep session, workspace, user, and agent scopes isolated unless an explicit API requests cross-scope behavior.
- Embedding work can fail or be unavailable; do not block memory writes on provider-backed embedding.

## Scope Discipline

- If `mvp-scope.md` marks a feature as later than MVP, keep it as scaffolding or explicit non-support rather than quietly shipping partial behavior.
- When CLI or agent-visible output changes, keep the Rust behavior, governed artifacts, and tests aligned.
