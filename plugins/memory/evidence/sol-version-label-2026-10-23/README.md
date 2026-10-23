# Libellé de version stockée — diagnostic Sol

**Candidat rejeté selon le critère préannoncé : 20/21, témoin 19/21.** Les42
réponses et leurs42 threads distincts sont conservés. Le changement simulé
remplace seulement22octets du préfixe ; suffixe, LF et corps JSON Q3 sont
identiques. Aucun nouveau code Rust ou binaire n'est qualifié par cet essai.

| Cas | Témoin actuel | Libellé candidat |
|---|---|---|
| q08 rétention Ares11jours | Correct1/3 ; essais1/3 omettent11 | Correct3/3 |
| q09 identifiant inconnu | Abstention correcte3/3 | Abstention correcte3/3 |
| q11 horaire corrigé | Correct3/3, variantes14h50 admises | Correct3/3 |
| q17/q18 Harbor | Correct3/3 chacun | Correct3/3 chacun |
| q20 Vesper | Valeurs stockées citées3/3 ; approbation inconnue | Valeurs citées2/3 ; essai2 omet le tag stocké |
| h02/h05/h08 transfert | Correct1/1 chacun | Correct1/1 chacun |

Les six réponses Vesper refusent honnêtement de confirmer une approbation que
la source ne documente pas. Leur complétude sur cette approbation reste fausse.
Le critère prospectif exigeait également la citation du tag stocké : son omission
dans le candidat q20/2 rejette le candidat. Aucun résultat n'est remplacé et
aucune réussite générale n'est déduite du signal fini sur Ares.

[Protocole gelé](protocol.json), [entrées exactes](inputs.json),
[42 sorties](diagnostic-01/run.json), [revue indépendante](independent-review.json)
et [évaluation](assessment.json) permettent l'inspection individuelle. La revue
est faite par un agent et vérifiée par le parent ; scores et étiquettes de
conditions sont masqués, mais les préfixes restent visibles. Le premier fichier
de préparation de revue avait des réponses nulles : il est conservé, puis
[review-inputs-02.json](review-inputs-02.json) est reconstruit depuis les mêmes
sorties brutes. Aucun appel modèle n'est relancé pour cette réparation.

Recalcul hors ligne depuis la racine, vers une nouvelle destination :

```powershell
python plugins/memory/evidence/sol-version-label-2026-10-23/assess_probe.py --output plugins/memory/evidence/sol-version-label-2026-10-23/runtime/recheck.json
```

Le résultat attendu est un code1, `evidenceConsistent=true`,
`candidateAccepted=false`. Le sujet reste la source `b828223`, le client Codex
installé et le modèle/effort `gpt-6.1-sol / max`. Pas d'API payante, installation,
mémoire réelle, configuration permanente ou nouvel accès. Les échecs Q3/Luna
restent séparés. Le problème d'adéquation de la question Vesper doit être
clarifié prospectivement ; ce rejet n'autorise aucune promotion automatique.
