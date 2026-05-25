# --- .claude/rules/storage.md (dans le repo Elegy) ---
# Ce fichier se charge UNIQUEMENT quand Claude touche des fichiers dans storage/

---
paths:
  - "rust/crates/elegy-memory/src/storage/**"
---

Tu travailles sur le moteur de stockage SQLite d'Elegy-Memory.

Vérifie la cohérence avec @rust/crates/elegy-memory/docs/architecture/storage-schema.md.
Le schéma est géré dans `schema.rs` — toute modification = STOP + validation humaine.
Les migrations se font dans `ensure_schema()`, jamais de SQL brut ad-hoc ailleurs.
Tous les accès DB passent par le trait `MemoryStore` défini dans `traits.rs`.
