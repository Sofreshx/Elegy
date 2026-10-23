---
title: Qualification Memory après correction Tokio
status: in-progress
owner: Elegy Memory
---

# Correction Tokio et nouvelle qualification

La PR49 a passé ses 19 contrôles avant fusion. L'ajout de
[RUSTSEC-2026-0386](https://github.com/rustsec/advisory-db/blob/main/crates/tokio/RUSTSEC-2026-0386.md)
à la base RustSec pendant la fin de cette CI a ensuite fait échouer cargo-deny
sur `main`. Le correctif conserve uniquement Tokio 1.52.3 → 1.53.3 dans le
lockfile, sans exception d'avis. La qualification précédente reste attachée
à sa source et à ses binaires ; aucune réponse antérieure n'est réécrite.

Le [plan prospectif](../../../../docs/plans/memory-recall-quality-fixes.md#tokio-advisory-follow-up-after-pr49)
fixe les critères et les arrêts. Les [préparations](preparation.json),
[source téléchargée](dependency-source.json), [avis](advisory.json),
[audit frais](cargo-audit-preparation.json) et
[réduction du diff du lockfile](lock-preparation.json) sont conservés.
La compilation sur source commise, le gel des empreintes, les 84 nouveaux
contextes, les revues, le reçu isolé et les contrôles finaux restent à exécuter.

Les trois régressions de chemins échouent sur les wrappers Q7 puis passent
sur Q8 : [RED](native-validation/path-normalizer-red.json),
[GREEN](native-validation/path-normalizer-green.json).
`reproduce_path_regression.py --output <nouveau-fichier.json>` reproduit le RED
sans appel modèle. Les traces publiques neutralisent seulement les préfixes
locaux ; leurs originaux restent hors de l'arbre public. Le harnais final
contient exactement 25 tests découverts, dont la réussite complète reste requise.

Aucun changement de mémoire réelle, réglage permanent, installation, API
payante ou accès persistant. Un autre logiciel hôte reste exclu par l'utilisateur.
