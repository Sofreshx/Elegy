---
title: Magasin de qualification Memory publiable
status: completed
owner: Elegy Memory
---

# Magasin public courant

Ce magasin contient 16 copies régulières sous `inputs/`, son ledger et son reçu `3b103385-f0a9-4961-a12a-103a8318a2ff`. Le [statut relu](../public-qualification-status.json), l'[historique](../public-qualification-history.json) et le [résumé](../public-receipt-summary.json) vérifient les empreintes ; le sujet externe exige toujours une revue.

Le bundle complet des preuves se trouve séparément à la racine Q7, avec son [manifeste public](../public-proof-manifest.json). Les chemins internes du manifeste, y compris sa copie sous `inputs/`, se résolvent depuis cette racine Q7 ; ces 198 fichiers ne sont pas prétendus présents dans `inputs/`. Les copies de pièces jointes sont vérifiables individuellement depuis le présent `--evidence-dir`.

La [transformation de publication](../publication-derivatives.json) conserve l'original local et son [archive de métadonnées](../recorder-original/README.md). Ce reçu distinct rapporte exactement les mêmes sessions, réponses et scores ; seul le packaging de preuve change. Aucun nouveau test de réponse modèle n'a été exécuté.
