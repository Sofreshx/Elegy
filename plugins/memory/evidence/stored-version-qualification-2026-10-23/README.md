---
title: Qualification Codex des versions stockées
status: in-progress
owner: memory
---

# Qualification prospective des binaires corrigés

Cette campagne vise le Codex installé `gpt-6.1-sol / max`, choisi par l'utilisateur.
Elle reste en cours : aucun verdict de qualité ni autorisation de fusion ne se
déduit des tests natifs. Le [protocole](protocol.json), les
[25 questions primaires](corpus.json), les [12 questions de transfert](transfer-corpus.json)
et le [sujet exécuté](subject.json) sont fixés avant les nouveaux appels modèle.

Le code produit est `95bf15ba2afa8e7655c45a8127b6a060893b695a`. Le
[verrou des sources](source-lock.json) relie 149 entrées CLI et 18 entrées MCP aux
objets Git et aux octets compilés, avec conversions de fins de ligne explicites.
Les [copies exécutables](pinned-binaries.json) sont isolées du répertoire Cargo.
Le [statut du CLI](pinned-status.json) confirme `buildMatchesSource=true`.

Le seul changement produit depuis la réparation de fixture est le libellé
`Latest stored versions` à la place de `Historical memory data` : même longueur,
même suffixe et même budget. Il désigne les versions des lignes de la base source,
sans certifier une fraîcheur ou une approbation extérieure. Le correctif antérieur
de classement des noms proches reste inclus dans le code testé.

Les 408 tests Memory passent sur cette source, sans échec ni test ignoré ; les
31 tests contextual-recall en font partie. Formatage et Clippy passent. La
[validation native](native-validation/memory.json) et les logs conservent aussi
le rouge/vert ciblé, les erreurs de capture et de commande de build, puis le
[build corrigé](native-validation/build-02.json). Les suites MCP/hook inchangées
peuvent réutiliser la [preuve précédente](../recall-fixes-2026-10-23/native-validation/run.json).

Le harnais Q2 scellé est réutilisé. La revue statique exige aussi les empreintes
des huit fichiers de qualification, la reconstruction exacte des inputs de
revue et les liens question/contexte/réponse, ainsi que la configuration normalisée
complète du rappel. Dix-neuf tests du harnais passent avant le replay ; trois
contrôles nécessitant les nouveaux fichiers de replay doivent encore passer
avant le premier appel modèle. L'erreur de dépendance du premier lancement
est [conservée et expliquée](native-validation/harness-preflight-deferred.json).

La campagne prévue comprend 84 sessions éphémères : 37 paires avec/sans rappel,
puis dix répétitions critiques supplémentaires. Les 47 réponses avec rappel
doivent toutes être complètes et étayées ; chaque erreur bloque le verdict.
Les abstentions sûres sans rappel restent distinctes des réponses complètes.

Les [échecs Luna](../recall-fixes-2026-10-23/README.md),
[échecs Codex initiaux](../codex-qualification-2026-10-23/README.md),
[diagnostic de libellé rejeté](../sol-version-label-2026-10-23/README.md) et
[réparation de fixture](../fixture-adequacy-2026-10-23/README.md) restent intacts.
q20 demande désormais région/tag enregistrés ; q25 demande l'approbation absente
et impose l'abstention. Aucun fait d'approbation n'a été ajouté.

Pour reproduire les contrôles locaux depuis ce dossier :

```text
python verify_source.py --with-binaries
python campaign.py --phase preflight
python campaign.py --phase replay-primary
python campaign.py --phase replay-transfer
python -m unittest test_campaign test_assess test_verify_source test_evidence_binding -v
```

Les exécutables sont reconstruits hors ligne depuis le commit produit avec
`cargo build --offline --locked -p elegy-memory -p elegy-memory-mcp --bin elegy-memory --bin elegy-memory-mcp-stdio`.
Un autre build peut avoir une empreinte binaire différente ; ne pas substituer
ses fichiers aux copies historiquement qualifiées. Les phases refusent les
sorties existantes ; utiliser un nouveau dossier pour une nouvelle campagne.

Les bases, journaux et configurations sont jetables. Ni mémoire réelle, ni
configuration permanente, ni API payante, ni installation ou accès persistant.
Il s'agit de nouvelles sessions sur un même logiciel hôte installé, avec rappel
explicite. Le hook automatique Desktop, un autre hôte logiciel (exclu à la demande
de l'utilisateur), l'usage quotidien et une amélioration globale ne sont pas établis.
