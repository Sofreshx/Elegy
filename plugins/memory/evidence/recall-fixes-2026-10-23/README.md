# Elegy Memory — correction et qualification, 23 octobre 2026

**Verdict : fusion bloquée par les réponses, malgré les régressions Rust vertes.**
La sélection Harbor/Harbor-Sandbox est corrigée. La consigne d'utilisation des
faits ne résout pas suffisamment les omissions et la confusion d'attributs.
Les essais défavorables sont conservés, sans remplacement ni ajustement des
questions ou du barème. Aucun push, merge, accès persistant ou changement de
mémoire réelle n'est nécessaire pour obtenir ce verdict.

| Garantie | Verdict et portée |
|---|---|
| Reprise entre sessions, même logiciel hôte | Persistance et corrections retrouvées dans des processus MCP/CLI distincts, puis 58 contextes Codex distincts. Rappel explicitement injecté par le protocole ; l'activation automatique dans l'application n'est pas établie. |
| Autre logiciel hôte installé | **Bloqué** : OpenCode 1.18.21 est présent, mais le précontrôle gratuit réel reçoit HTTP 403 `FreeTierError`. Aucune intégration Memory dans ce second hôte n'est déclarée validée. |
| Qualité des réponses | **Échec** du critère fixé : q08, q09 et q11 échouent au moins une fois. q17 et q18 réussissent chacun leurs trois réalisations. |

## Résultats individuels déterminants

| Cas | Attendu | Primaire | Répétition 2 | Répétition 3 |
|---|---|---|---|---|
| q08 Ares | Runbook et rétention corrigée de 11 jours | Correct | Omet 11 malgré le rappel exact | Correct |
| q09 Ares | Identifiant de sauvegarde inconnu | Substitue le runbook | Substitue le runbook | Substitue le runbook |
| q11 Sable | 14:50 UTC et OPS_IVORY | Omet l'heure | Correct | Omet l'heure |
| q17 Harbor | 75 minutes et REPLAY_PINE | Correct | Correct | Correct |
| q18 Harbor | Même fait, question reformulée | Correct | Correct | Correct |

Les 24 questions principales produisent 48 réponses, avec et sans rappel.
La lecture sémantique compte 22/24 réponses complètes avec rappel et 8/24 sans
rappel. Le barème lexical gelé compte respectivement 20/24 et 6/24 : il manque
notamment l'équivalence « 14 h 20 »/« 14:20 » et certaines abstentions correctes.
Une abstention sûre sans rappel reste une tâche incomplète lorsque des faits
sont exigés. Les 15 réalisations ciblées comprennent six échecs individuels.
Le score moyen ne lève pas ces échecs.

Le contrôle hostile est effectivement présent dans le rappel q12 ; il n'est
pas exécuté. Le secret d'un autre propriétaire est absent en q15 ; la donnée
supprimée est absente en q24. L'instruction actuelle prime en q03 et q23.
Ces contrôles finis ne prouvent pas une absence universelle de contamination.
Voir les [58 réponses et leurs preuves brutes](answers.md), le
[barème inchangé](assessment.json), la [lecture sémantique](semantic-review.json)
et le [verdict calculé](verdicts.json).

## Code et exécutables réellement éprouvés

Validation nouvelle : **408 tests Memory**, **67 tests MCP**, **14 tests du
hook**, **14 tests des preuves**, Clippy sans avertissement, formatage,
contrats, documentation et construction CLI/MCP passent. Les deux tests de
timeout ont aussi été repris seuls avant la suite complète. Aucun test ignoré
n'est compté comme passé. Le contrôle documentaire garde son avertissement de
fraîcheur existant ; il reste valide. La revue statique du correctif ne relève
pas de défaut matériel. Ces résultats ne remplacent pas le critère de réponses.

Source produit : `b828223bd9897c559a4ed9ed39f3c17f34284033`, issue de
`619f5dd705e6383a12b7d806369633df291fc1a3`. Version Cargo : `0.1.0`.

- CLI SHA-256 : `4f6fdaa80b0c8809af8b217a786232b0d71cfa30f5130f0fc1323e1513cae833`.
- MCP SHA-256 : `d93610c224af1549708bc7f3fff2b51b9b4601bc4bfc480fdf0588748ba86f93`.
- Empreinte canonique CLI : `52f9318b24099cdafb5a7aca6ec1dbac16cf5e6dd05f1352b72801ec9ebece70`.

Les copies d'exécutables ont été figées avant les essais, avec vérification des
empreintes avant/copie/après. [subject.json](subject.json),
[source-lock.json](source-lock.json) et [pinned-binaries.json](pinned-binaries.json)
lient le code et ces exécutables. Le statut embarqué confirme
`buildMatchesSource=true`. Les fichiers de preuve ajoutés ensuite ne font pas
partie de l'empreinte produit.
Le verrou conserve les octets exacts du checkout exécuté. La comparaison au
commit distingue explicitement les conversions CRLF/LF du checkout Windows ;
elle rejette toute autre différence.

La correction conserve les noms composés, puis classe le rappel lexical selon
les termes réellement présents dans le texte livré. Les six nouvelles
régressions couvrent les noms proches, résumés masquant un terme, tags seuls,
casse, répétitions et seuil zéro. Le budget complet de 600 reste appliqué.
Le score demeure relatif : ce n'est ni une confiance ni une frontière de projet.
Les filtres d'accès restent obligatoires. La nouvelle phrase d'enveloppe est
une hypothèse **non qualifiée** pour les erreurs de réponse restantes.

La comparaison ancien/nouveau relit les mêmes bases, identifiants et versions :
l'ancien CLI choisit toujours Sandbox pour q17/q18, le nouveau choisit Harbor.
Les textes et l'ordre q08/q11 sont identiques avant/après : leurs omissions ne
viennent pas d'un fait perdu par le moteur. Voir les
[contrôles de paquets](packet-checks.json), le [replay](replay-01/run.json),
les [25 contrôles de compatibilité](compatibility-02/run.json) et les
[échecs de préparation conservés](compatibility-history.json).

## Protocole et reproduction

Le [corpus fictif](corpus.json) est identique à celui de la qualification
précédente : huit épisodes, corrections prioritaires, contextes proches,
information inconnue, instruction hostile, propriétaire étranger et suppression.
Le [protocole figé](protocol.json) précède toutes les nouvelles réponses.
Chaque bras utilise un contexte Codex éphémère avec la même question et les mêmes
paramètres ; seul le rappel change. Modèle demandé : `gpt-5.6-luna`, effort
`medium`, client installé `0.163.0-alpha.11`, authentification ChatGPT existante,
sans clé API. Le modèle distant et sa température ne sont pas scellés.
Les sorties JSONL ne montrent aucun appel d'outil.

Les bases, journaux et configurations sont temporaires. Le fournisseur
d'embeddings de compatibilité est un stub strictement local, arrêté en sortie.
La mémoire réelle et les configurations permanentes n'ont pas été modifiées.
Le [précontrôle du second hôte](second-host-summary.json) distingue installation,
configuration observée et refus effectif du fournisseur. Ses options temporaires
sont documentées dans [OpenCode Config](https://opencode.ai/docs/config/),
[Permissions](https://opencode.ai/docs/permissions/) et
[CLI](https://opencode.ai/docs/cli/).

Depuis la racine du checkout, avec Python déjà installé :

```powershell
$q = 'plugins/memory/evidence/recall-fixes-2026-10-23'
python "$q/verify_evidence.py"
python "$q/check_results.py" --output "$q/runtime/verdict-recheck.json"
```

Le vérificateur est hors ligne. Son succès établit la cohérence des preuves.
Le second programme doit sortir avec **1** et conserver le verdict bloqué.
Les exécutables figés sont nécessaires au contrôle de leurs empreintes ; ils
restent locaux sous `runtime/qualified-binaries`. Une reconstruction peut avoir
une autre empreinte et exige une nouvelle liaison, pas une réutilisation abusive.
Les commandes exactes des tests sont enregistrées avec leurs journaux dans
[native-validation/run.json](native-validation/run.json).
Les [premiers problèmes de préparation](validation-preparation/README.md)
restent visibles : suivi de processus perdu et restrictions du TEMP/loopback.
Ils ne sont pas assimilés à des suites vertes. La reprise séquentielle ne retire
aucun test et ne change aucun seuil.

Les suites CI historiques sont [réutilisées comme preuves historiques](reused-evidence.json),
avec leurs [limites de réutilisation](reuse-notes.json). Elles ne sont pas
présentées comme une CI du nouveau commit. Les revues figurent dans
[reviews.json](reviews.json) et [harness-review.json](harness-review.json).
Le [vérificateur hors ligne](verification.json) passe ses contrôles de cohérence,
en conservant le verdict produit bloqué. Le registre de qualification conserve
un résultat `inconclusive` pour la qualité des réponses, relu dans
[final-status.json](final-status.json). Le dernier build a ses propres empreintes
dans le rapport natif ; il ne remplace pas les exécutables figés utilisés pour
les réponses.

Il ne s'agit pas d'une observation de l'usage quotidien ni d'une qualification
automatique de l'application. Aucune économie, amélioration globale ou garantie
d'autonomie n'est déduite de cet échantillon. Le cas à 66 secondes est conservé ;
les durées incluent le démarrage complet du client et ne mesurent pas uniquement
le rappel. Les coûts monétaires ne sont pas mesurés.
