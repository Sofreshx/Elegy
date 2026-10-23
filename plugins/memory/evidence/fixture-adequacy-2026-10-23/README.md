# Adéquation du corpus Vesper

**Fixture clarifiée : les12 réponses passent**,6/6 dans chaque condition.
q20v2 restitue la région et le tag stockés ; q25 reconnaît explicitement que
leur approbation actuelle n'est pas documentée. Le témoin passe aussi : cette
étude vérifie l'adéquation des deux questions, sans mesurer un gain du préfixe.

La version2 conserve les24 autres questions originales, toutes leurs actions
et leurs attentes, ainsi que les12 cas de transfert. Elle clarifie seulement
la question q20 et ajoute q25, sans ajouter de fait stocké. Les attentes connues
de q20 restent identiques ; le nouveau contrôle exige l'abstention sur
l'approbation. Les anciens Q3/Q4 et leurs échecs restent intacts.

Le [protocole](protocol.json), les [entrées exactes](inputs.json), les
[sorties brutes](diagnostic-01/run.json), la [revue indépendante](independent-review.json)
et l'[évaluation](assessment.json) sont conservés. Scores et conditions étaient
masqués à la revue, mais les préfixes restent visibles ; les reviewers sont des
agents. Le parent a relu les12 paragraphes. Tous les threads sont nouveaux par
rapport à Q3/Q4. Le code source reste b828223 : il s'agit encore de préfixes
simulés sur les paquets exacts Q3, pas d'une qualification de nouveaux binaires.

Le signal Ares vient séparément de Q4 :3/3 contre1/3 sur cette petite comparaison,
avec un rejet global20/21 conservé. La décision suivante est un correctif minimal
du libellé, un test d'enveloppe observé rouge puis vert, de nouveaux binaires
liés au code et84 contextes réels sur le corpus révisé. Aucune réussite ancienne
n'est réécrite et aucun gain global ou économique n'est promis.

Recalcul hors ligne depuis la racine, vers un fichier nouveau :

```powershell
python plugins/memory/evidence/fixture-adequacy-2026-10-23/assess_probe.py --output plugins/memory/evidence/fixture-adequacy-2026-10-23/runtime/recheck.json
```

Code attendu0, `evidenceConsistent=true`, `candidateAccepted=true`. Cette
acceptation porte seulement sur le diagnostic de fixture. Authentification
ChatGPT existante, modèle gpt-6.1-sol/max, sans API payante, installation,
mémoire réelle, configuration permanente ou nouvel accès.
