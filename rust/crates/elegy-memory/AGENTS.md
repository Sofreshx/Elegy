# Elegy Memory

## Required Docs

- Read `docs/architecture/mvp-scope.md` before implementing memory behavior.
- Read `docs/architecture/memory-model.md` when changing scopes, scoring, decay, confidence, provenance, or state transitions.
- Read `docs/architecture/storage-schema.md` before changing persistence.

## Non-Negotiables

- Store distilled memories only. Never persist raw transcripts.
- Every memory needs provenance; do not create bypass writes around provenance or salience.
- Keep session, workspace, user, and agent scopes isolated unless an explicit API requests cross-scope behavior.
- Embedding work can fail or be unavailable; do not block memory writes on provider-backed embedding.
- If `mvp-scope.md` marks a feature as later than MVP, keep it as a skeleton/no-op rather than shipping behavior.

## Validation

- Prefer `cargo test -p elegy-memory` for memory-only changes.
- Add or update CLI tests when command output, defaults, or JSON envelopes change.
