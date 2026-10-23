---
title: Qualification Codex des versions stockées
status: completed
owner: Elegy Memory
---

# Résultat de la qualification ciblée

Les **47 réponses avec rappel sont complètes et étayées**, dont les dix répétitions critiques. Les 84 réponses sont conservées et relues individuellement par deux agents puis par le parent. Le [protocole figé](protocol.json) passe ses critères ; le [calcul vérifiable](assessment.json) ne contient aucune erreur de preuve ni de réponse bloquante.

| Garantie | Verdict et preuve | Portée |
|---|---|---|
| Reprise entre sessions | Observée : 37 paquets après fermeture/réouverture des clients, puis 84 sessions Codex distinctes. [Contrôles](replay-checks.json) | Même ordinateur et même logiciel hôte ; rappel fourni explicitement. |
| Isolation des agents | 42 contrôles positifs/négatifs, 37 sources inchangées ; 24 espaces vérifiés vides après nettoyage. | ID, recherche, liste et contexte sur les bases fictives partagées. |
| Autre logiciel hôte installé | Non qualifié ; volet exclu par l'utilisateur. | L'ancien blocage OpenCode reste dans les preuves historiques. |
| Qualité des réponses | Avec rappel : 37/37 paires complètes et 10/10 répétitions. Sans rappel : 14/37 complètes, 23 abstentions sûres ou réponses partielles. [84 réponses](results.md) | Codex installé `gpt-6.1-sol / max`, corpus fictif fini. |

Le score lexical figé moyen est **0,9099 avec rappel contre 0,4009 sans rappel** sur 37 paires. Il ne mesure pas à lui seul la justesse : [cinq faux négatifs lexicaux](errors.md) et une erreur de classification du réviseur sont documentés, sans modifier les réponses ni les scores. Les durées enregistrées portent sur des processus complets ; elles ne prouvent pas un gain de latence d'inférence. Zéro nouvel appel à une API payante ; l'allocation ChatGPT existante a été utilisée. Aucune économie monétaire ou amélioration globale n'est revendiquée.

Le code produit est `95bf15ba2afa8e7655c45a8127b6a060893b695a`, descendant du `main` `619f5dd` incluant la PR48. Le [verrou](source-lock.json) relie les 149 entrées CLI et 18 entrées MCP aux objets Git et aux octets compilés, avec conversions de fins de ligne explicites. Le [sujet](subject.json) et les [copies exécutées](pinned-binaries.json) donnent toutes les empreintes : CLI `b9c6e9de1175…`, MCP `9083f13f079e…`, client Codex `00c0f1e40992…`. Le gel prospectif du protocole et du harnais est `2d4f788285a3b42ae905e9d73bb2b28910bb8a84`.

Deux corrections produit sont incluses : le classement contextualisé distingue les noms proches et évalue le texte réellement livré ; le préfixe indique les dernières **versions stockées**, avec priorité aux instructions actuelles et abstention si l'attribut manque. Le delta final de libellé remplace 22 octets par 22 octets ; il ne certifie jamais une approbation ou une fraîcheur extérieure. La spec et six régressions couvrent le classement ; l'assertion du préfixe a été observée rouge puis verte.

Les [408 tests Rust Memory](native-validation/memory.json) passent sans échec ni test ignoré, dont 31 tests contextual-recall. Formatage, Clippy et build passent. Les suites MCP (67 tests) et hook (14 tests) sont [réutilisées depuis Q1](../recall-fixes-2026-10-23/native-validation/run.json), leurs fichiers restant inchangés ; les [22 tests du harnais](native-validation/harness-final.json) passent sur les nouveaux replays. Les checks de la PR et ses métadonnées de fusion portent l’état de livraison, séparé de cette campagne.

Le [reçu local](record-output.json) est `c784eaac-12fb-4de3-9268-ab004d36de9f`, résultat `reported-satisfied`, avec `reviewRequired=true` et sujet externe à revérifier. [Statut relu](qualification-status.json) et [historique relu](qualification-history.json) vérifient les empreintes ; aucune readiness n'est promue automatiquement.

Les [échecs Luna](../recall-fixes-2026-10-23/README.md), [diagnostics de réponses](../answer-grounding-2026-10-23/README.md), [Q3 Codex échoué](../codex-qualification-2026-10-23/README.md), [Q4 rejeté](../sol-version-label-2026-10-23/README.md) et [Q5 de réparation de fixture](../fixture-adequacy-2026-10-23/README.md) restent intacts. Q20 demande les faits enregistrés ; Q25 demande l'approbation absente et impose l'abstention. Q5 mesure l'adéquation de la question, pas un gain causal supplémentaire.

Pour vérifier les preuves hors ligne, depuis ce dossier :

```text
python verify_source.py
python assess.py --output verification-neuve.json
```

`verify_source.py --with-binaries` vérifie en plus les copies locales conservées. Une nouvelle campagne exige un nouveau dossier, les sources/outils/binaires liés et une allocation ChatGPT existante ; les phases refusent tout écrasement. La commande de build réellement réussie est dans [build-02.json](native-validation/build-02.json). Les réponses peuvent varier ; une reconstruction ne remplace pas les empreintes des exécutables historiques.

Ni mémoire réelle, ni configuration permanente, installation supplémentaire ou accès persistant n'ont été créés/modifiés. Le hook automatique Desktop, tous les réglages utilisateur, un autre hôte et l'usage quotidien restent hors preuve. Les noms de modèles et leur backend ne sont pas scellés.

## Copies publiques de journaux

Trois copies de logs Q1/Q6 contenant des chemins locaux sont neutralisées après la première CI de PR49. Les résultats, réponses, protocoles, reçus et leurs entrées scellées ne changent pas. Les anciens manifestes décrivent le snapshot original du commit `a753b4b5f4bddf553a5b11255b3532957291d9c2`, et non ces trois dérivés publics. Le [registre de neutralisation](../ci-integration-2026-10-23/redactions.json) lie les objets Git originaux, leurs SHA-256, les nouveaux octets et la transformation reproductible. Les erreurs initiales restent visibles.
