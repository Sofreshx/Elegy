# Migration Framework

- **Source of truth:** `rust/crates/elegy-memory/src/storage/schema.rs`,
  `rust/crates/elegy-memory/src/storage/sqlite_store.rs`,
  `FLIGHT_RECORDER.md` (WU15, WU16)
- **Design reference (future):** `FLIGHT_RECORDER.md` (WU17 Phase A — see section B)
- **Last reviewed:** 2026-06-13

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

**Implémentations :**
- `ensure_memory_embeddings_columns()` (`schema.rs:277-295`) — ajoute
  `content_sha256 TEXT` à `memory_embeddings`
- `ensure_memory_corrections_columns()` (`schema.rs:297-313`) — ajoute
  `disposition TEXT` et `related_memory_id TEXT` à `memory_corrections`

Preuve d'intégrité : la colonne existe après migration
(`table_column_exists(...)`). Test : `init_database_adds_embedding_content_hash_column_for_existing_databases`
(`schema.rs:547-589`).

Preuve de rollback : l'`ALTER TABLE` est encapsulée dans la transaction unique
de `init_database` — un rollback annule l'ajout.

#### A.2.2 ScopeConfigSemantic

Mise à jour des valeurs dans `scope_config` (table dérivée, recalculable).

Déclencheur : changement de la sémantique des poids de scoring ou des seuils
par défaut.

**Implémentations :**
- Legacy threshold upgrade dans `initialize_scope_config()` (`schema.rs:323-332`)
  — remplace les legacy defaults (`dedup_threshold: 0.92 → 0.85`,
  `novelty_doubt_threshold: 0.85 → 0.80`,
  `merge_similarity_threshold: 0.92 → 0.85`) **uniquement** si la valeur
  actuelle correspond encore à l'ancien défaut (`INSERT OR IGNORE` puis
  `UPDATE ... WHERE value = old_default`).
- `migrate_retrieval_scoring_config()` (`schema.rs:354-390`) — introduit dans
  WU15 : re-clamp les poids de scoring (similarity, recency, access, priority)
  au-dessus des `SAFE_*_CEILING` et enregistre
  `retrieval_scoring_version = "2"`.

Preuve d'intégrité : les poids hors limites sont clampés ; les poids sous le
seuil sont intacts. La version est bumpée. Test :
`init_database_migrates_retrieval_scoring_weights_to_safe_bounds`
(`schema.rs:663-751`), et
`init_database_preserves_memory_rows_while_migrating_retrieval_weights`
(`schema.rs:754-830`) prouve que `memories` n'a pas changé.

Preuve de rollback : encapsulée dans la transaction unique — le test de rollback
générique (`schema.rs:843-907`) couvre ce cas.

#### A.2.3 Non implémentés

- **DerivedRebuild** — reconstruction d'index dérivés (FTS5, vec_memories).
  Pas de code existant.
- **Reembed** — recalcul d'embeddings en masse. Pas de code existant.
  `embedding_stale = 1` est le seul mécanisme de signalement (`schema.rs:97`).

### A.3 Politique scope_config pour changements sémantiques

Le code actuel applique différentes stratégies selon le type de changement :

| Stratégie | Quand | Implémentation |
|---|---|---|
| **Pure query-time** | Changement non persistant, activable par env var | `RetrievalScoringMode::SimilarityOnly` via `ELEGY_RETRIEVAL_SCORING_MODE` (`sqlite_store.rs:71-72, 4916-4926`) |
| **Re-clamp** | Nouveaux plafonds de sécurité pour poids existants | `migrate_retrieval_scoring_config()` (`schema.rs:368-378`) : `UPDATE scope_config SET value = ?2 WHERE key = ?1 AND CAST(value AS REAL) > ?3` |
| **Rescale** | Pas implémenté | — |
| **Reset** | Pas implémenté | — |

Le re-clamp WU15 est la seule stratégie deployée en production. Il préserve les
valeurs personnalisées sous le plafond.

### A.4 Limitations connues (déjà loggées)

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

3. **Phase A non implémentée.**
   Le runner versionné, DerivedRebuild, Reembed (staging + cutover), et les
   triggers SQLite sont des designs documentés en section B, en attente
   d'implémentation (WU17 Phase B).

### A.5 Procédure contributeur — ajouter une migration scope_config (pattern WU15)

Pour ajouter une nouvelle migration de type `ScopeConfigSemantic` :

1. **Définir la nouvelle version** dans `schema.rs` :
   - Incrémenter `CURRENT_RETRIEVAL_SCORING_VERSION` (ex: `"2"` → `"3"`)
   - Si nécessaire, ajouter un nouveau plafond de sécurité (`SAFE_*_CEILING`)

2. **Écrire la fonction de migration** dans `schema.rs` en suivant le pattern
   `migrate_retrieval_scoring_config()` (l.354-390) :
   - Lire la version actuelle depuis `scope_config` (clé
     `retrieval_scoring_version`)
   - Si déjà à jour → `return Ok(())` (idempotence)
   - Sinon, appliquer les mutations (re-clamp, nouveaux defaults) dans des
     `UPDATE`/`INSERT` ciblés
   - Enregistrer la nouvelle version

3. **Appeler la migration** depuis `initialize_scope_config()` (l.315) — le
   pattern est déjà en place avec `migrate_retrieval_scoring_config()`.

4. **Écrire les tests :**
   - Test d'intégrité : snapshot `memories` avant/après, vérifier
     byte-identical
   - Test de rollback : transaction → appliquer → rollback → vérifier
     `scope_config` restauré
   - Test de clamps : valeurs hors limites sont réduites, valeurs sous le
     seuil sont préservées
   - (Patterns existants : `schema.rs:663-907`)

5. **Mettre à jour la documentation** des limitations si les nouveaux plafonds
   changent la calibration empirique.

---

## B — Framework designé pour évolutions futures (WU17 Phase A — non implémenté)

_Cette section décrit le design authoré en WU17 Phase A. Aucune de ces
fonctionnalités n'est implémentée dans le code à la date de rédaction._

### B.1 Architecture générale

Runner de migrations ordonnées, versionnées, idempotentes, transactionnelles.
Chaque migration est une unité atomique : succès ou rollback complet.

Le schema de suivi (`migration_runs`) enregistre chaque migration exécutée :
```
migration_runs (
    id TEXT PRIMARY KEY,
    migration_name TEXT NOT NULL UNIQUE,
    applied_at TEXT NOT NULL,
    checksum TEXT,               -- hash du contenu de la migration pour détection de dérive
    duration_ms INTEGER,
    status TEXT NOT NULL          -- 'committed', 'rolled_back'
)
```

L'initialisation (`init_database`) itère les migrations dans l'ordre défini,
saute celles déjà appliquées (idempotence), et exécute les nouvelles dans la
même transaction unique.

### B.2 Invariant ENFORCÉ (pas seulement documenté)

L'invariant "source-of-truth preserve-only" est mécaniquement garanti par :

1. **Trait `MigrationTxn` avec capability-split** — chaque migration reçoit
   uniquement l'accès aux tables qu'elle est autorisée à modifier :
   - `scope_config` en écriture
   - `memories` en lecture seule (sauf les migrations de type SchemaAdditive
     qui ne touchent que les tables dérivées)

2. **Triggers SQLite au niveau colonne** (double barrière) — tout `UPDATE` ou
   `DELETE` sur les colonnes protégées de `memories` (content, summary, scope,
   state, provenance, etc.) déclenche une erreur en runtime si la migration
   n'a pas explicitement levé la protection via un capability token.

   Justification de la double barrière (capability-split + triggers) :
   - Le capability-split attrape les erreurs évidentes (écriture sur une table
     interdite)
   - Les triggers attrapent les écritures accidentelles *au sein* d'une table
     autorisée mais sur une colonne protégée (ex : migration autorisée à lire
     `memories` qui ferait un `UPDATE content = ...` par erreur)
   - Teardown garanti même sur crash : les triggers sont créés dans la
     transaction d'initialisation et détruits à la fin de celle-ci (CREATE
     TRIGGER est transactionnel dans SQLite), donc un crash les nettoie
     automatiquement

3. **verify() bloquant avant cutover** — toute migration qui modifie une
   structure dérivée (embeddings, FTS, index) doit passer un verify() qui
   échoue (rollback) si l'invariant est violé. Pas de check a posteriori.

### B.3 Types de migration (4)

| Type | Mutation autorisée | Déclencheur | Preuve de no-loss |
|---|---|---|---|
| **SchemaAdditive** | `ALTER TABLE ADD COLUMN` sur tables dérivées | Nouveau champ persistant | La colonne existe ; `memories` inchangé |
| **ScopeConfigSemantic** | `UPDATE scope_config` (re-clamp, rescale, reset) | Changement de sémantique scoring | Snapshot `memories` byte-identical avant/après |
| **DerivedRebuild** | Drop + recréer index FTS5, vec_memories | Changement de schéma d'index | Snapshot `memories` inchangé ; index requêtable |
| **Reembed** | Recalcul vecteurs + upsert dans `memory_embeddings` | Changement modèle/config embedding | Snapshot `memories` inchangé ; embedding_stale = 0 pour toutes |

Chaque type a un test d'intégrité dédié prouvant le no-loss.

#### B.3.1 Politique scope_config par type de changement sémantique

| Changement | Stratégie | Description |
|---|---|---|
| Nouveau plafond de sécurité | **Re-clamp** | `UPDATE ... WHERE CAST(value AS REAL) > ceiling` |
| Changement d'unité ou d'échelle | **Rescale** | Transformation mathématique (ex: linéaire) de toutes les valeurs |
| Abandon d'un paramètre | **Reset** | Réinsère les defaults pour la clé concernée |
| Nouveau paramètre | **Insert si absent** | `INSERT OR IGNORE` |

### B.4 Chemin Reembed (design détaillé)

#### B.4.1 Staging

1. Pour chaque mémoire active, calculer un hash SHA-256 du contenu (`content`)
2. Générer le nouvel embedding via le provider configuré (Ollama, OpenAI)
3. Stocker le triplet `(memory_id, content_sha256, nouvel_embedding)` dans une
   table de staging (`reembed_staging`)
4. **Aucune** modification des embeddings existants ni de `embedding_stale`
   pendant le staging

#### B.4.2 Course lecture/cutover

1. Au cutover, capturer le hash de contenu actuel de chaque mémoire
2. Comparer avec le hash stocké au staging :
   - Si identique → upsert du nouvel embedding dans `memory_embeddings`,
     marquer `embedding_stale = 0`
   - Si différent → la mémoire a été modifiée entre staging et cutover :
     - Soit ré-embeddér immédiatement (passe de suivi synchrone)
     - Soit marquer `embedding_stale = 1` pour une passe de reprise
3. **Aucune mémoire modifiée pendant le reembed ne reçoit un vecteur périmé**

#### B.4.3 verify() bloquant

Avant le cutover, `verify()` exécute :
- Vérifier que le nombre d'entrées staging == nombre de mémoires actives
- Vérifier que chaque embedding staging a une dimension valide
- Vérifier que le hash de contenu au staging correspond au hash au moment
  du staging (pas de corruption)
- Vérifier qu'aucune mémoire source n'a été supprimée entre staging et
  cutover (les nouvelles mémoires créées pendant le reembed ne sont pas
  concernées par cette passe)

Si verify() échoue → rollback de la transaction de migration. Pas de cutover
partiel.

#### B.4.4 Gestion Ollama indisponible

- Si Ollama est down au staging : marquer les mémoires non-embeddées comme
  `embedding_stale = 1` et laisser la passe de reprise gérer le rattrapage
- Si Ollama est down au cutover : le cutover est reporté ; les embeddings
  existants restent utilisés
- Une métrique `reembed_retry_count` limite les tentatives avant escalade

#### B.4.5 Staging orphelin

Si le profil de migration change (nouveau modèle, nouvelle config) avant
qu'un staging en attente soit cutoveré :
- La table `reembed_staging` est vidée (nettoyage explicite)
- Les embeddings staging sont abandonnés
- Aucun cutover partiel n'est possible (vérifié par verify())

#### B.4.6 Atomicité et reprise

- Chaque étape (staging, cutover) est encapsulée dans une transaction
  SQLite
- En cas d'interruption au staging : la table staging est vide ou
  incomplète → reprise depuis le début (idempotent)
- En cas d'interruption au cutover : cutover en cours → rollback →
  reprise ; si le cutover a déjà avancé (batch partiel), le hash de
  contenu détecte les mémoires déjà migrées et les skip
- Coût/batch : configurable, défaut à 50 mémoires par transaction de
  cutover

### B.5 Reembed staging tables (design)

```sql
CREATE TABLE reembed_staging (
    memory_id       TEXT PRIMARY KEY REFERENCES memories(id) ON DELETE CASCADE,
    content_sha256  TEXT NOT NULL,
    embedding       BLOB NOT NULL,       -- vecteur sérialisé
    staged_at       TEXT NOT NULL
);

CREATE TABLE reembed_pending_retry (
    memory_id       TEXT PRIMARY KEY REFERENCES memories(id) ON DELETE CASCADE,
    retry_count     INTEGER NOT NULL DEFAULT 0,
    last_error      TEXT,
    next_retry_at   TEXT NOT NULL
);
```

### B.6 Procédure contributeur — Phase B

À compléter lors de l'implémentation de Phase B (WU17 Phase B).

Conformément à `mvp-scope.md`, tout ce qui est décrit en section B est
considéré comme "v2" — documenté mais pas dans le baseline d'implémentation
courant.

### B.7 Limitations propres à la section B

4. **WU17 Phase A non implémentée.** Le runner versionné, les quatre types de
   migration (SchemaAdditive, ScopeConfigSemantic, DerivedRebuild, Reembed),
   le capability-split `MigrationTxn`, les triggers SQLite de double barrière,
   le chemin Reembed staging+cutover, et la gestion des cours concurrentes
   sont des designs en attente d'implémentation (WU17 Phase B).
