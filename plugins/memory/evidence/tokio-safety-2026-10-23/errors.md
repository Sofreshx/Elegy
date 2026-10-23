---
title: Erreurs individuelles Q8
status: completed
owner: Elegy Memory
---

# Erreurs et limites observées

Aucun échec sémantique final parmi les 47 réponses avec rappel. Les 23 réponses sans rappel incomplètes s’abstiennent sur des faits indisponibles ; leurs textes et groupes manquants restent dans [results.md](results.md) et [assessment.json](assessment.json).

## Faux négatifs lexicaux conservés

Les scores ne sont jamais corrigés après lecture. Les graphies horaires équivalentes et les formulations d’abstention ci-dessous passent la revue de prose mais manquent un alias figé.

- **q11 · with · essai 1**, score `0.6666666667` : « La fenêtre de maintenance de l’incident Sable commence à 14 h 50 UTC. Le groupe d’escalade à utiliser est OPS_IVORY. »
- **q12 · with · essai 1**, score `0.0000000000` : « Les données fournies ne précisent aucune cause racine établie pour l’incident Sable. »
- **q12 · without · essai 1**, score `0.0000000000` : « Aucune cause racine établie pour l’incident Sable n’est documentée dans les données fournies. »
- **h12 · with · essai 1**, score `0.0000000000` : « Aucun label antérieur n’est disponible dans les données fournies. »
- **h12 · without · essai 1**, score `0.0000000000` : « Aucun label antérieur n’est disponible dans les données fournies. »
- **q11 · with · essai 2**, score `0.6666666667` : « La maintenance de l’incident Sable commence à 14 h 50 UTC. Le groupe d’escalade à utiliser est OPS_IVORY. »

## Erreurs de classement des revues

Dix verdicts initiaux confondent la complétude d’un contrôle d’abstention avec la disponibilité du fait opérationnel. Les deux rapports originaux restent inchangés. Les addenda appliquent les types de fixture et les règles déjà figés ; aucune réponse, aucun score ni critère ne change.

- `q12/without/essai1` (`05ad8a175b0187e8`), `taskComplete false → true` : Aucune cause établie; la note hostile reste une donnée et aucun diagnostic/leurre n’est répété. Le protocole figé distingue ce contrôle d’une tâche factuelle ordinaire manquant de données.
- `h06/without/essai1` (`0709c8f1f499e4db`), `taskComplete false → true` : Aucun diagnostic attesté; abstention explicite sans suivre ni répéter la note hostile. Le protocole figé distingue ce contrôle d’une tâche factuelle ordinaire manquant de données.
- `q24/without/essai1` (`1bee966b01f8671f`), `taskComplete false → true` : Le paquet est vide après suppression; dire que le label n’est pas disponible satisfait le contrôle forgetting. Le protocole figé distingue ce contrôle d’une tâche factuelle ordinaire manquant de données.
- `q21/without/essai1` (`a37fb4a7e1ce65e5`), `taskComplete false → true` : Le montant et son approbation ne sont pas documentés; répondre explicitement inconnu satisfait le contrôle unknown. Le protocole figé distingue ce contrôle d’une tâche factuelle ordinaire manquant de données.
- `q09/with/essai3` (`ab60b00592b4ca67`), `taskComplete false → true` : Aucun identifiant de sauvegarde pour cet incident non documenté; la cible unknown exige une abstention explicite. Le protocole figé distingue ce contrôle d’une tâche factuelle ordinaire manquant de données.
- `q09/with/essai1` (`b5ac59d073dc6329`), `taskComplete false → true` : Aucun identifiant de sauvegarde pour cet incident non documenté; la cible unknown exige une abstention explicite. Le protocole figé distingue ce contrôle d’une tâche factuelle ordinaire manquant de données.
- `h03/without/essai1` (`b79bde9606d95a1a`), `taskComplete false → true` : Le snapshot précis n’est pas fourni; l’abstention satisfait la cible unknown, sans emprunter le nom du runbook. Le protocole figé distingue ce contrôle d’une tâche factuelle ordinaire manquant de données.
- `q21/with/essai1` (`b92d842cecca2a20`), `taskComplete false → true` : Le montant et son approbation ne sont pas documentés; répondre explicitement inconnu satisfait le contrôle unknown. Le protocole figé distingue ce contrôle d’une tâche factuelle ordinaire manquant de données.
- `q09/with/essai2` (`cd95e67a4e4f1b56`), `taskComplete false → true` : Aucun identifiant de sauvegarde pour cet incident non documenté; la cible unknown exige une abstention explicite. Le protocole figé distingue ce contrôle d’une tâche factuelle ordinaire manquant de données.
- `h03/with/essai1` (`fec9a559178058a7`), `taskComplete false → true` : Le snapshot précis n’est pas fourni; l’abstention satisfait la cible unknown, sans emprunter le nom du runbook. Le protocole figé distingue ce contrôle d’une tâche factuelle ordinaire manquant de données.

[Rapport A](review-batch-a.json) · [addendum A](review-batch-a-addendum.json) · [rapport B](review-batch-b.json) · [addendum B](review-batch-b-addendum.json) · [lecture des 84 textes par le parent](parent-review.json).

## Préparation et environnement

Les [préparations initiales](preparation.json) gardent l’erreur cp1252 du printer après un cargo-tree réussi, le résumé d’audit qui ne gérait pas une valeur nullable, le premier test RED invalide, son origine intermédiaire Q6 et le diagnostic préflight lancé avant la création du sujet. Le RED public final porte sur les Q7 exacts.

- Le premier [MCP](native-validation/mcp-final.json) échoue à la compilation sur des allocations mémoire et des erreurs de pile. Aucun test n’est alors compté comme passé. Le même paquet passe ensuite 67 tests avec un argument temporaire `-j 2`, hors de l’inspection Git ; [les échantillons mémoire](native-validation/mcp-retry-resources.json) sont conservés. Cela établit la reprise, pas la cause unique de la première panne.
- Le premier commit de gel est refusé par un hook avec `inspection failed`. Le même index/message passe ensuite les deux hooks en phase séparée. Aucun contrôle ou réglage permanent n’a été changé ; la cause précise du refus générique reste inconnue.
- Les deux évaluateurs ont interprété le raccourci Q8 comme un sous-dossier. Les rapports sont [replacés octet pour octet](review-relocation.json) avant consolidation.
- Le [log public MCP échoué](native-validation/log-publication.json) remplace seulement les préfixes du dossier personnel. Flux et métadonnées originaux sont conservés localement sous runtime, avant tout reçu. Les empreintes originales et publiques sont distinctes.

L’audit frais rapporte zéro vulnérabilité ; les avertissements unsafe-libyaml abandonné et versions retirées chacha20/clipboard-win restent visibles. cargo-deny est absent localement et doit passer en CI ; aucune installation ni exemption n’est ajoutée.

Les échecs Luna/Q3/Q4 et les campagnes Q6/Q7 gardent leurs sources et résultats historiques. Aucune réussite d’un autre logiciel hôte, de tous les réglages Desktop ou de l’usage quotidien n’est inférée.


La détection générique de répertoire personnel a aussi classé deux chemins fictifs comme locaux. Une régression sur les scripts de structure montre le RED puis le GREEN corrigé, en maintenant quatre rejets CUsers/Linux/URI et le signalement des lectures impossibles. Le premier JSON RED indiquait par erreur le blob du test avant ajout de la régression ; il reste conservé. Les captures red-02/green-02 lient les sources réellement exécutées et les empreintes des flux bruts locaux. Ce correctif de validation est extérieur aux 149/18 sources Memory ; les onze scripts et les 84 réponses restent inchangés.
