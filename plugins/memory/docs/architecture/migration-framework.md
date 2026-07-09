# Migration Framework

- **Source of truth:** `plugins/memory/src/storage/schema.rs`,
  `plugins/memory/src/storage/sqlite_store.rs`,
  `FLIGHT_RECORDER.md` (WU15, WU16, WU17)
- **Last reviewed:** 2026-06-15

---

## A — Framework actuellement implémenté

### A.1 Invariant central

La migration est **preserve-only** sur les lignes source-de-vérité (`memories`
content + metadata) : elles sont byte-identiques avant et après migration. Seules
les entrées dérivées et recalculables (`scope_config`) peuvent changer.

**Preuve :** `init_database_preserves_memory_rows_while_migrating_retrieval_weights`
dans `schema.rs:754-830` — snapshot complet de `memories` avant et après
migration, vérifié byte-à-byte (y compris contenu long, tags, timestamps, scope,
state, custom metadata, compteurs d'accès).

**Atomicité :** La migration entière s'exécute dans une seule transaction SQLite
ouverte par `init_database()` (`schema.rs:69-73`) :

```rust
let transaction = connection.transaction()?;
create_schema(&transaction)?;
initialize_scope_config(&transaction)?;
run_migrations(&transaction, &[
    &SchemaAdditiveMigration,         // ADD COLUMN sur tables dérivées
    &ScopeConfigSemanticMigration,    // re-clamp scope_config
])?;
verify_schema_version(&transaction)?;
transaction.commit()?;
```

**Preuve de rollback :** `retrieval_scoring_migration_rolls_back_atomically`
(`schema.rs:843-907`) — applique `migrate_retrieval_scoring_config` dans une
transaction, rollback, vérifie que `scope_config` est identique à l'état
antérieur.

### A.2 Types de migration existants

Deux types sont implémentés dans le code actuel :

#### A.2.1 SchemaAdditive

Ajout de colonnes avec `ALTER TABLE ADD COLUMN`. N'affecte que les tables
dérivées, jamais la table source `memories`.

Déclencheur : nouvelle fonctionnalité nécessitant un champ persistant.

**Implémentation :** `SchemaAdditiveMigration` (`schema.rs`) — struct
implémentant le trait `Migration` avec la capability `SchemaAdditive`.
Appelle `ensure_memory_embeddings_columns()` et
`ensure_memory_corrections_columns()` via le runner.

**Tests :**
- `init_database_adds_embedding_content_hash_column_for_existing_databases`
- `schema_additive_migration_adds_columns_via_runner`

Preuve de rollback : encapsulée dans la transaction unique de
`init_database` — un rollback annule l'ajout.

#### A.2.2 ScopeConfigSemantic

Mise à jour des valeurs dans `scope_config` (table dérivée, recalculable).

Déclencheur : changement de la sémantique des poids de scoring ou des seuils
par défaut.

**Implémentations — deux niveaux :**

1. **Runtime bootstrap** dans `initialize_scope_config()` (`schema.rs`) —
   `INSERT OR IGNORE` des defaults pour toute clé manquante, et mise à jour
   des legacy thresholds (`dedup_threshold: 0.92 → 0.85`,
   `novelty_doubt_threshold: 0.85 → 0.80`,
   `merge_similarity_threshold: 0.92 → 0.85`) via
   `UPDATE ... WHERE value = old_default`. S'exécute à chaque init.

2. **Migration versionnée** `ScopeConfigSemanticMigration` (`schema.rs`) —
   struct implémentant le trait `Migration` avec la capability
   `ScopeConfigWrite`. Appelle `migrate_retrieval_scoring_config()` via le
   runner : re-clamp les poids de scoring (similarity, recency, access,
   priority) au-dessus des `SAFE_*_CEILING` et enregistre
   `retrieval_scoring_version`.

**Tests (migration) :**
- `init_database_migrates_retrieval_scoring_weights_to_safe_bounds`
- `init_database_preserves_memory_rows_while_migrating_retrieval_weights`
- `retrieval_scoring_migration_rolls_back_atomically`
- `scope_config_semantic_migration_records_version_via_runner`
- `scope_config_semantic_migration_idempotent_on_rerun`

Preuve de rollback : encapsulée dans la transaction unique.

#### A.2.3 Non implémenté

- **DerivedRebuild** — reconstruction d'index dérivés (FTS5, vec_memories).
  Le trait `Migration` et l'enum `MigrationCapability` supportent ce type,
  mais aucune implémentation concrète n'existe encore. `todo!()` réservé pour
  v1/v2 selon `mvp-scope.md`.

### A.3 Invariant ENFORCÉ (double barrière)

L'invariant "source-of-truth preserve-only" est mécaniquement garanti par :

1. **Trait `Migration` avec capability-split** — chaque migration déclare ses
   capabilities via `capabilities()`. Le runner (`run_migrations`) rejette
   toute tentative d'écrire sur une colonne protégée d'une capability non
   déclarée.

2. **Triggers SQLite au niveau colonne** — créés par
   `create_protective_triggers()` avant la première migration et détruits par
   `drop_protective_triggers()` après la dernière. Tout `UPDATE` ou `DELETE`
   sur les colonnes protégées de `memories` (content, scope, state, provenance,
   etc. — 14 colonnes) déclenche `RAISE(ABORT)`.

3. **verify() bloquant** — chaque migration doit passer `verify()` après
   `run()` avant que son enregistrement dans `migration_runs` soit inséré.
   Un échec de verify() provoque le rollback complet de la transaction
   d'initialisation.

### A.4 Politique scope_config pour changements sémantiques

Le code actuel applique différentes stratégies selon le type de changement :

| Stratégie | Quand | Implémentation |
|---|---|---|
| **Pure query-time** | Changement non persistant, activable par env var | `RetrievalScoringMode::SimilarityOnly` via `ELEGY_RETRIEVAL_SCORING_MODE` (`sqlite_store.rs:71-72, 4916-4926`) |
| **Re-clamp** | Nouveaux plafonds de sécurité pour poids existants | `migrate_retrieval_scoring_config()` (`schema.rs:368-378`) : `UPDATE scope_config SET value = ?2 WHERE key = ?1 AND CAST(value AS REAL) > ?3` |
| **Rescale** | Pas implémenté | — |
| **Reset** | Pas implémenté | — |

Le re-clamp WU15 est la seule stratégie deployée en production. Il préserve les
valeurs personnalisées sous le plafond.

### A.5 Limitations connues (déjà loggées)

1. **T = 0.03 fitted au canary `fr_q07`.**
   (`FLIGHT_RECORDER.md:2634`) — Le seuil de protection de similarité est
   calibré sur le gap observé de `fr_q07` (~0.03147). Il devrait être revisité
   contre la distribution réelle des gaps de similarité plutôt que traité comme
   universel. Les candidats dans la bande de quasi-égalité (`gap < T`) restent
   affinables par les signaux secondaires (recency, access, priority).

2. **Poids recency/priority non validés empiriquement.**
   (`FLIGHT_RECORDER.md:2634-2635`) — Les poids de scoring par défaut et les
   plafonds d'apprentissage n'ont pas été validés sur un corpus de production.
   Ils restent des valeurs raisonnables non calibrées.

3. **DerivedRebuild non implémenté.**
   Le type `DerivedRebuild` est supporté par le trait `Migration` et l'enum
   `MigrationCapability` mais n'a pas d'implémentation concrète. Réservé
   v1/v2.

### A.6 Procédure contributeur — ajouter une migration

Pour ajouter une nouvelle migration scope_config de type `ScopeConfigSemantic`
:

1. **Définir la nouvelle version** dans `schema.rs` :
   - Incrémenter `CURRENT_RETRIEVAL_SCORING_VERSION` (ex: `"2"` → `"3"`)
   - Si nécessaire, ajouter un nouveau plafond de sécurité (`SAFE_*_CEILING`)

2. **Créer une struct implémentant `Migration`** dans `schema.rs` :
   - Nom unique via `name()` (utilisé comme clé d'idempotence dans
     `migration_runs`)
   - Capability `ScopeConfigWrite` dans `capabilities()`
   - Logique de migration dans `run()` (re-clamp, rescale, reset)
   - Vérification dans `verify()` (version lue depuis `scope_config`)

3. **Enregistrer la migration** dans `init_database()` en l'ajoutant au slice
   passé à `run_migrations()`.

4. **Écrire les tests :**
   - Test d'intégrité : snapshot `memories` avant/après, vérifier
     byte-identical
   - Test de rollback : transaction → appliquer → rollback → vérifier
     `scope_config` restauré
   - Test de clamps : valeurs hors limites sont réduites, valeurs sous le
     seuil sont préservées
   - Patterns existants : voir `schema.rs:900-955` pour le trait, les tests
     Phase 1 (runner) et Phase 3 (SchemaAdditive, ScopeConfigSemantic)

5. **Mettre à jour la documentation** des limitations si les nouveaux plafonds
   changent la calibration empirique.

Pour une migration de type `SchemaAdditive` (ADD COLUMN) :

1. Écrire la fonction `ensure_*_columns()` existante ou ajouter une nouvelle
2. Créer une struct implémentant `Migration` avec capability `SchemaAdditive`
3. Appeler la fonction depuis `run()` et vérifier avec `table_column_exists()`
   dans `verify()`
4. Enregistrer dans `init_database()` via `run_migrations()`

Pour une migration de type `Reembed` (recalcul d'embeddings) :

1. Instancier `ReembedMigration` avec un provider et un profile_id
2. Le runner appelle `run_staging() → verify_staging() → run_cutover()` via
   le trait `Migration`
3. Voir Phase 2 tests pour les patterns de test

---

## B — Reembed staging+cutover (implémenté WU17 Phase B suite)

Le chemin Reembed complet est implémenté et intégré dans le chemin de production :

| Type | Mutation autorisée | Déclencheur | Statut |
|---|---|---|---|
| **ReembedMigration** | Recalcul d'embeddings dans `reembed_staging` → `memory_embeddings` / `vec_memories` | Changement de modèle d'embedding, embeddings stale | ✅ Implémenté |
| **DerivedRebuild** | Drop + recréer index FTS5, vec_memories | Changement de schéma d'index | ❌ Réservé v1/v2 |

### B.1 ReembedMigration — intégration

- **Type exposé :** `ReembedMigration` et `run_migrations` sont publics dans `elegy_memory::storage`.
- **CLI :** La commande `reembed` route via `ReembedMigration` + `run_migrations()`. Le chemin direct `sqlite_store::reembed_stale_memories` est retiré.
- **Scope filter :** `ReembedMigration::with_scope()` limite le re-embedding à un scope spécifique.
- **Runner idempotence :** `migration_runs` bloque la ré-exécution d'un run déjà enregistré.

### B.2 Invariants

| Invariant | Mécanisme |
|---|---|
| Table `memories` jamais mutée | Capability `Reembed` + triggers colonne-level sur `memories` via `run_migrations()` |
| Cutover atomique | `run_staging()` → `verify_staging()` → `run_cutover()` dans la même transaction |
| Garde de course (hash de contenu) | `run_cutover()` compare `content_sha256` du staging avec le contenu courant ; mismatch ⇒ `embedding_stale = 1` conservé |
| Fail-fast provider down | `check_provider_health()` avant `run_staging()` — échec ⇒ abort sans staging |
| Staging orphelin | `run_staging()` nettoie `reembed_staging` et `reembed_pending_retry` dont le `memory_id` n'existe plus dans `memories` |
| Reprise après interruption | Si staging incomplet (`staged < active`), le staging existant est supprimé et le run recommence ; si complet, le run est skippé |
| No-loss sur provider failure mid-run | `verify_staging()` exige `staged + retry == active` ; les mémoires en échec restent dans `pending_retry` pour le prochain run |

### B.3 Procédure — utiliser ReembedMigration

```rust
use rusqlite::Connection;
use elegy_memory::storage::{ReembedMigration, run_migrations};

let connection = Connection::open("my_memory.db")?;
let migration = ReembedMigration::new(
    Box::new(|content| {
        // Appeler l'embedding provider ici
        Ok((vec![0.0f32; 768], 768))
    }),
    "my-profile-v1",
).with_scope(MemoryScope::Workspace);

run_migrations(&connection, &[&migration])?;
```
