---
title: Qualification finale Memory sur Codex installé
status: completed
owner: Elegy Memory
---

# Qualification du produit corrigé

Les **47 réponses avec rappel sont complètes et étayées** : 37 questions appariées et dix répétitions critiques. Sans rappel, **14/37 réponses sont complètes et 23/37 s'abstiennent ou restent partielles sans inventer de faits**. Les [84 réponses individuelles](results.md), les [erreurs lexicales et de revue](errors.md) et le [calcul vérifiable](assessment.json) sont conservés. Aucune réponse n'a été remplacée ou relancée pour sélectionner un meilleur résultat.

| Garantie | Verdict | Preuve et portée |
|---|---|---|
| Reprise entre sessions sur le même hôte | Observée | 37 paquets après fermeture/réouverture des clients CLI/MCP, puis 84 fils Codex distincts. Même ordinateur et logiciel hôte, rappel fourni explicitement. |
| Isolation des agents | Observée dans les scénarios contrôlés | [42 contrôles](replay-checks.json) ID/recherche/liste/contexte, 37 sources inchangées et 24 espaces vérifiés vides après nettoyage. Bases fictives uniquement. |
| Autre logiciel hôte installé | Non qualifié ; exclu par l'utilisateur | L'ancien blocage OpenCode reste dans les preuves historiques. Une nouvelle session du même hôte n'est pas présentée comme un second hôte. |
| Qualité des réponses collectées | Critères ciblés satisfaits | 37/37 paires avec rappel et 10/10 répétitions critiques passent séparément. Codex installé `gpt-6.1-sol / max`, corpus fictif fini. |

Le score lexical figé moyen est **0,9459459459 avec rappel contre 0,3738738739 sans rappel**, delta apparié 0,5720720721 sur ces 37 paires. Cinq faux négatifs lexicaux restent inchangés : abstentions valides et `14 h 50` pour `14:50`. Les moyennes de processus sont 13 151 ms et 15 195 ms ; démarrage, réseau, authentification et ordonnancement empêchent d'en déduire un gain de latence d'inférence. Zéro nouvel appel à une API payante ; l'allocation ChatGPT existante a été utilisée. Aucune économie monétaire ni amélioration globale n'est revendiquée.

## Sujet, méthode et conservation

Le produit est **`5eb34e0b32034a4bd254c6d775e5179dd2e4a1e1`**, descendant du `main` `619f5dd` incluant la PR48. Le gel prospectif du protocole, du sujet et des huit scripts de qualification est **`90094e9147824fe9a70c9b45ee29e3d92fb74b25`**, avant les 84 réponses. Le [protocole](protocol.json), le [sujet](subject.json) et le [delta du harnais](harness-delta.json) reprennent les questions, actions fictives et critères de Q6. La [préparation initiale](qualification-preparation.json) reste un historique provisoire antérieur à ces révisions ; le protocole et le sujet figés gouvernent cette campagne.

Le [verrou des sources](source-lock.json) relie les 149 entrées CLI et 18 entrées MCP aux objets Git et aux octets compilés, avec conversions de fins de ligne explicites. Les [copies exécutées](pinned-binaries.json) sont CLI `55b76918b44dc1e556a027f444d0c47c8e1175bae6d3d7a59df1fc274917afd6` et MCP `ad5e338e9733a0dd1516e9fec28b99f625fe63246c0b52cf929ddc9b1cf83c7b`. Le client Codex reste `00c0f1e4099246fddcd67ec620ca8ed50827c7d70e113b7224f7a2933cf2d67c`, version `0.163.0-alpha.11`. Le build utilise les outils GNU présents, Rust/Cargo 1.96.1 ; le MSRV 1.88 relève aussi de la CI finale. Ces copies sont conservées localement, sans installation ni publication des exécutables.

Les [reprises contrôlées](replay-checks.json) vérifient corrections, propriétaires, suppressions, texte délivré, budget de 600 octets et configuration isolée complète. Les notes hostiles sont réellement présentes pour q12/h06 ; le secret fictif étranger est absent pour q15 ; les contextes supprimés q24/h12 sont vides. Les [contrôles d'exécution](execution-checks.json) vérifient 84 fils uniques, même client et configuration, commandes, prompts, sorties brutes et contextes exacts. Chaque processus utilise une configuration temporaire, sans chargement des autres réglages utilisateur ou instructions de projet.

Deux revues indépendantes de 42 réponses ont masqué scores et noms des bras ; la présence du contexte révèle toutefois le traitement. Le parent a lu les 84 réponses. La [revue consolidée](semantic-review.json) lie le fichier d'entrée entier et chaque question/contexte/réponse/sortie brute. Cinq erreurs de classement de complétude ont été corrigées selon les contrôles d'abstention déjà figés, dans des addenda conservant les avis initiaux. Évaluation par des agents, sans évaluateur humain ni changement rétroactif de critère.

Le [reçu public](public-record-output.json) est `3b103385-f0a9-4961-a12a-103a8318a2ff`, statut relu `reported-satisfied`, `reviewRequired=true`, sujet externe à revérifier. Le [résumé vérifié](public-receipt-summary.json) confirme 16 pièces intactes, `buildMatchesSource=true`, aucun motif d'obsolescence et aucune promotion de readiness. Le [magasin public](recorder-public/README.md) contient seulement ces 16 copies régulières ; le [bundle public de 198 fichiers](public-proof-manifest.json) se vérifie séparément depuis la racine Q7, comme son champ `pathBase` l'indique.

Le [reçu original](record-output-02.json) `488792f5-fcb6-426b-b151-8b23716ae0f6` avait scellé une métadonnée contenant le chemin local de Python. Le gate de publication l'a détecté. Les [deux générations](publication-derivatives.json) restent distinctes : original exact et 195 entrées validés dans deux copies locales privées, ledger et reçu originaux [archivés ensemble](recorder-original/README.md), puis dérivé public ne remplaçant que `argv[0]` par `<python>`. Tous les autres octets de cette métadonnée, les 84 réponses et leurs scores restent identiques. L'ancien [manifeste](proof-manifest.json), les sorties et les revues de ce reçu décrivent le snapshot original avant neutralisation, pas le bundle public courant. Les originaux portant le chemin local sont conservés localement, sans sauvegarde distante ; sources et preuves publiques sont destinées à la branche autorisée. Aucun nouvel appel modèle n'a été nécessaire.

Le premier enregistrement avait été refusé par le sandbox avant tout état persistant ; sa [sortie](record-output.json) est conservée. L'[enregistrement original avec accès normal](record-execution-02.json) et l'[enregistrement public distinct](public-record-execution.json) utilisent le même binaire, les mêmes sessions, contrôles et mesures ; seules les références des pièces publiques changent.

## Validation des sources finales

Le comportement, la spec et les tests Memory restent identiques à Q6 sur `95bf15b` ; la [revue des sources](source-review.json) vérifie ce périmètre. Les [408 tests Memory de Q6](../stored-version-qualification-2026-10-23/native-validation/memory.json), incluant 31 contextual-recall, et les suites MCP 67/hook 14 de Q1 sont réutilisés dans leur portée historique. Ils ne remplacent pas les nouveaux paquets et réponses sur les binaires corrigés.

Sur la source finale passent le [build CLI/MCP](native-validation/build-final-02.json), les [151 tests des consommateurs corrigés](native-validation/zip-tests-final-02.json), les [31 contextual-recall](native-validation/contextual-final.json), les [27 tests du harnais](native-validation/harness-final.json) et le [formatage](native-validation/format-final.json). La [revue native](review-final-native.json) vérifie empreintes et sources inchangées. Le premier essai natif a échoué sur six tests Accounts faute d'espace temporaire sur C: ; la même commande passe avec des variables temporaires enfant sur D:. La [clarification du chemin](native-validation/temp-space-clarification.json) indique la base réellement utilisée et la conservation des fixtures après exécution, sans changement d'environnement permanent.

Les [échecs Luna](../recall-fixes-2026-10-23/README.md), Q3/Q4 et la réparation de fixture Q5 ne sont pas réécrits. [Q6](../stored-version-qualification-2026-10-23/README.md) reste la preuve réussie du sujet historique `95bf15b`. Q20 demande les faits stockés, q25 l'approbation absente ; cette séparation était figée avant Q6 et Q7. Les trois nouveaux [logs natifs neutralisés](native-validation/log-publication.json) conservent empreintes brutes/publiques et originaux exacts locaux ; leurs verdicts, scores et erreurs ne changent pas.

Depuis la racine, vérifier hors ligne avec une sortie nouvelle :

```text
python plugins/memory/evidence/ci-integration-2026-10-23/verify_source.py
python plugins/memory/evidence/ci-integration-2026-10-23/assess.py --output .tmp/verification-memory-neuve.json
python -m unittest discover -s plugins/memory/evidence/ci-integration-2026-10-23 -p "test_*.py" -v
python plugins/memory/evidence/ci-integration-2026-10-23/sanitize_evidence.py
```

`verify_source.py --with-binaries` vérifie aussi les copies locales. Une reconstruction future ne remplace pas leurs empreintes historiques. Une nouvelle campagne exige un nouveau dossier ; les phases refusent l'écrasement. Les contrôles workspace, MSRV, sécurité et packaging doivent passer sur la tête exacte de la [PR49](https://github.com/Sofreshx/Elegy/pull/49) avant fusion ; son état de livraison est séparé de cette preuve.

Aucune mémoire réelle, configuration permanente ou identité d'accès n'a été modifiée. Aucun nouvel outil, API payante ou accès persistant n'a été ajouté. L'intégration automatique Desktop, tous les réglages utilisateur, un autre logiciel hôte et l'usage quotidien restent hors preuve. Les noms de modèles et leur backend ne sont pas scellés.

## Historique des prérequis d'intégration

La PR49 a qualifié le produit `95bf15b` sur Codex installé `gpt-6.1-sol / max`. Sa CI sur `a753b4b` révèle des prérequis d'intégration. Aucune erreur de cette CI n'est assimilée à une erreur des réponses modèle.

Autorité : `AGENTS.md`, `plugins/memory/AGENTS.md`, `plugins/memory/CONTRIBUTING.md`, spec contextual-recall. L'utilisateur autorise push et merge après réussite des contrôles et exclut l'autre logiciel hôte.

1. Corriger le traitement des fichiers suivis vides par le vérificateur de structure ; prouver l'ancien échec puis l'acceptation du vide et le rejet d'un chemin local fictif.
2. Neutraliser trois chemins locaux dans des copies publiques de logs. Conserver les originaux dans le commit publié `a753b4b`, leurs empreintes et une transformation reproductible ; les anciens manifestes et reçus ne sont pas réécrits.
3. Exempter seulement dix détections Gitleaks historiques dont la valeur est le SHA-256 vérifié de `resource_auth.rs`. Garder toutes les autres détections actives.
4. Mettre à jour uniquement les dépendances vulnérables : quinn-proto 0.11.18, rustls 0.23.46, rustls-webpki 0.103.16, webpki-roots 1.0.10 et zip 9.0.5 (contrainte minimale 9.0.0) ; garder le MSRV 1.88 et les fonctions de validation des archives.
5. Après commit des sources finales, reconstruire CLI/MCP et lier leurs octets et objets Git. Réutiliser les suites toujours valides, exécuter les vérifications des consommateurs zip et les contrôles de rappel nécessaires. Une nouvelle campagne prospective reprend les 84 contextes et les critères figés de Q6 sans réécrire Q6.
6. Relire le delta, les preuves et la destination publique ; pousser sans force, vérifier le SHA distant, attendre chaque contrôle CI sur la tête finale. Fusionner seulement après réussite, par commit de merge préservant les sources qualifiées. Observer l'automatisation main-snapshot existante.

Arrêt : données réelles, configuration permanente, outil supplémentaire à installer, API payante, accès persistant, ambiguïté de sécurité ou échec de preuve exigeraient réévaluation. Les échecs restent visibles ; aucune exception générale de sécurité, diminution de seuil ou réécriture d'historique n'est permise. Aucun gain global ou financier n'est inféré.

Les résultats finaux figurent ci-dessus ; les preuves Q6 restent celles de leur source historique.

## Contrôles déjà observés

Le [triage lié à la tête CI](ci-failures.json) attribue séparément chaque échec. Le [registre des trois dérivés publics](redactions.json) conserve la reconstruction des originaux Git ; `python sanitize_evidence.py` et cinq tests passent. Le lecteur de fichiers du vérificateur accepte le vide et bloque une lecture impossible sans publier le détail de l'erreur ; les [deux régressions RED/GREEN](native-validation/repo-shape-final.json) conservent la détection de chemin local.

L'[audit de dépendances](native-validation/cargo-audit-03.json) sur les octets exacts du lockfile corrigé rapporte zéro vulnérabilité, avec l'avis d'abandon unsafe-libyaml et deux avertissements de versions retirées conservés. Deux problèmes de capture préalables (champ d'avertissement nullable, exécution sandbox) sont consignés et ne sont pas présentés comme des audits réussis.

La [revérification hors ligne Q6](native-validation/q6-recheck-02.json) reste `qualified-selected-configuration`, 84 fils et aucune erreur. Elle confirme uniquement le sujet historique 95bf15b. Une erreur du runner a d'abord partagé un log entre deux checks, puis écrit un nouveau fichier de résultat dans le mauvais dossier. La première capture est conservée dans le runtime local ; la nouvelle capture a cinq logs distincts vérifiés, et le [déplacement sans changement d'octets](native-validation/relocated-rechecks.json) est explicite. Aucun fichier Q5 existant n'a été modifié.

La [structure complète](native-validation/repo-shape-full.json) passe sans issue. Inspect/map/check et l'export de l'index documentaire passent via l'exécutable existant dont l'empreinte est enregistrée ; l'avertissement de fraîcheur préexistant reste non bloquant. Le commit final devra encore passer les mêmes contrôles en CI. Les contrôles de capture des sources rejettent une révision incorrecte et des sources non commises même avec Python `-O`.

La [revue CI](review-ci.json) et la [revue ZIP](review-zip.json) sont acceptées. La migration ZIP propage les erreurs de nom et conserve les permissions Unix dans les archives produites sous Windows ; le test v1 reste présent et le test v3 vérifie binaire 0755 et manifeste 0644. La fixture préparée pour UTF-8 endommait aussi la longueur centrale : elle est corrigée et nommée comme contrôle d’archive malformée, sans inventer une politique de rejet UTF-8. Les premiers résultats natifs ZIP sont des résumés observés explicitement signalés, sans flux bruts reconstruits. Les captures finales sur les sources commises figurent ci-dessus ; la CI finale reste une vérification distincte avant fusion.
