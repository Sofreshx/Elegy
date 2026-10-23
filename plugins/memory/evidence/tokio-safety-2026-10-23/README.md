---
title: Qualification Memory après correction Tokio
status: completed
owner: Elegy Memory
---

# Qualification ciblée sur Codex installé

Les **47 réponses avec rappel satisfont la cible figée** : 37 questions appariées et dix répétitions critiques. Sans rappel, **14/37 cibles sont satisfaites** : quatre questions résolubles directement et dix contrôles d’abstention. Les 23 autres réponses restent sûres mais incomplètes faute de données historiques. Les [84 textes exacts](results.md), [erreurs individuelles](errors.md) et [calculs vérifiables](assessment.json) sont conservés. Aucun texte n’a été remplacé ou relancé pour sélectionner un résultat.

| Garantie | Verdict | Preuve et portée |
|---|---|---|
| Reprise entre sessions du même hôte | Observée | 37 paquets après fermeture/réouverture des clients CLI/MCP, puis 84 fils Codex distincts, sur le même ordinateur. Rappel fourni explicitement. |
| Isolation et absence de contamination | Observées dans les scénarios contrôlés | 42 contrôles ID/recherche/liste/contexte, 37 sources durables inchangées et 24 espaces vérifiés vides après nettoyage. Données fictives uniquement. |
| Autre logiciel hôte installé | Exclu par l’utilisateur ; non qualifié | Les anciens blocages restent historiques. Une session du même logiciel n’est pas présentée comme un second hôte. |
| Qualité des réponses | Critères ciblés satisfaits | 37/37 réponses appariées avec rappel et 10/10 répétitions ; corrections prioritaires, contextes proches, attributs absents, injection hostile et suppression contrôlés séparément. |

La complétude d’un contrôle `unknown` signifie répondre correctement que l’information manque. Elle ne rend pas possible une restauration, un budget ou une approbation non documentés. Le résultat porte sur **Codex CLI installé, `gpt-6.1-sol / max`**, dans des processus isolés ignorant les autres réglages utilisateur. Il ne qualifie pas l’intégration automatique Desktop, tous les réglages réels, un autre hôte ou l’usage quotidien.

## Source et exécutables éprouvés

Le correctif de dépendance est commis en **`2a351a33124cd87d479f8f44ee274a131c84dff9`**. Le gel prospectif est **`ce49cd4a8e473fc11f40f54005a7fbebe73a8074`**, avant les 84 réponses : [gel](freeze.json), [protocole](protocol.json), [sujet](subject.json). Onze scripts de qualification sont liés par empreinte. Les corpus, questions et critères de Q7 sont repris sans changement ; les échecs antérieurs restent conservés.

Le [verrou des sources](source-lock.json) relie 149 entrées CLI et 18 entrées MCP aux objets Git et aux octets compilés, y compris les conversions de fins de ligne. Empreinte de source CLI : `a0c84881c88fb048d80539b3773453b636ba4e6798c3aa9a44cb976d72b05bcd`.

Les [copies réellement exécutées](pinned-binaries.json) sont :

- CLI : `0b469e0954a28bc75be9d01c8b422d594ce4bb952f9a0ec23e89816c67ef69a3`.
- MCP : `cd06ddfc2dca7aa92c13553e65be3876b8e1a858de5918901bb64343de6e7361`.
- Client Codex : `00c0f1e4099246fddcd67ec620ca8ed50827c7d70e113b7224f7a2933cf2d67c`, version `0.163.0-alpha.11`.

Le build verrouillé utilise les outils GNU existants, Rust/Cargo 1.96.1. Les copies sont conservées localement sous `runtime/qualified-binaries`, sans installation ni publication de ces exécutables. Les octets d’une reconstruction future peuvent différer ; ils ne remplacent pas ces copies historiques. Le backend et l’alias de modèle ne sont pas scellés.

## Protocole et résultats

Les bases, journaux, sessions et configurations temporaires sont nouveaux et isolés. Chaque épisode ferme les clients d’écriture puis rouvre les clients de lecture. Les 37 enveloppes proviennent réellement du CLI et des sous-processus MCP, avec plafond de 600 octets, embeddings désactivés et apprentissage désactivé. Les [contrôles de reprise](replay-checks.json) et [contrôles d’exécution](execution-checks.json) vérifient les paquets, les configurations complètes, les empreintes, les prompts et les 84 fils uniques.

Le corpus comprend 25 questions primaires et 12 de transfert, chacune avec et sans rappel, plus deux nouveaux essais avec rappel pour chacun de q08/q09/q11/q17/q18. L’ordre est alterné, avec au plus trois groupes simultanés. Toute erreur de contrôle, donnée étrangère, valeur obsolète, ajout non étayé ou répétition critique manquée bloque le verdict.

Le score lexical moyen figé vaut **0,9369369369 avec rappel contre 0,3738738739 sans rappel**, delta apparié 0,5630630631 sur ces 37 paires. Six faux négatifs restent inchangés : q11 avec rappel essais 1/2 (`14 h 50`), q12 avec/sans et h12 avec/sans (abstentions valides). Les moyennes de processus sont 14 739 ms et 15 079 ms ; démarrage, réseau, authentification et ordonnancement empêchent d’en déduire une différence de latence d’inférence. Aucun gain global ni économie monétaire n’est revendiqué. Zéro nouvel appel à une API payante ; l’allocation ChatGPT existante a été utilisée.

Deux revues séparées ont lu 42 textes chacune, avec scores et noms des bras masqués ; la présence du contexte révèle toutefois le traitement. Le parent a lu les 84 textes. Dix erreurs de classement de complétude sur des contrôles d’abstention sont corrigées dans des addenda gardant les avis initiaux. L’[audit indépendant des corrections](review-corrections-audit.json) les accepte sous la règle déjà figée. [Revue consolidée](semantic-review.json) et [lecture du parent](parent-review.json) lient les données et empreintes de chaque paragraphe. Aucun évaluateur humain n’est intervenu.

## Vérifications natives et conservation

Sur les nouvelles sources passent : build CLI/MCP, 31 contextual-recall, 67 MCP, 25 tests du harnais et formatage. La [revue des sources](source-review.json) et la [revue native](review-final-native.json) sont approuvées. Les 408 tests Memory, 151 tests des consommateurs ZIP et 14 hooks précédents sont réutilisés dans leur portée historique ; les contrôles workspace/MSRV/sécurité finaux restent une vérification distincte sur la tête de PR.

Le premier build des tests MCP échoue sur des allocations mémoire ; le même paquet passe ensuite avec `-j 2` temporaire en phase séparée. Le premier commit de gel échoue sur une inspection de confidentialité générique ; le même index/message passe ensuite les deux hooks inchangés. Les flux, empreintes et limites de causalité restent dans [errors.md](errors.md). Les préfixes personnels d’un log public échoué sont neutralisés avant tout reçu, avec originaux locaux et registre des dérivés. Aucune mémoire réelle, configuration permanente, installation ou identité d’accès n’a été modifiée.

## Reproduire la vérification

Depuis la racine, utiliser un fichier de sortie nouveau dans un dossier existant :

```text
python plugins/memory/evidence/tokio-safety-2026-10-23/verify_source.py
python plugins/memory/evidence/tokio-safety-2026-10-23/assess.py --output .tmp/q8-verification-neuve.json
```

`verify_source.py --with-binaries` vérifie aussi les copies locales. Les 25 tests via `python -m unittest discover -s plugins/memory/evidence/tokio-safety-2026-10-23 -p "test_*.py" -v` nécessitent ces copies et les octets du checkout qualifié. `reproduce_path_regression.py --output <fichier-neuf.json>` reproduit le RED des anciens wrappers sans appel modèle. Les phases refusent l’écrasement ; une nouvelle campagne exige un nouveau dossier, un build sur source commise et un gel avant ses nouveaux appels.

## Intégration et enregistrement

La PR49 a passé ses 19 contrôles avant fusion en `8dfca7c`. L’ajout de [RUSTSEC-2026-0386](https://github.com/rustsec/advisory-db/blob/84d943a6deeb8cc6c6f632531aaf4f1c62275239/crates/tokio/RUSTSEC-2026-0386.md) à la base pendant la fin de cette CI a ensuite fait échouer cargo-deny sur main. Le lockfile change seulement Tokio 1.52.3 → 1.53.3 ; résolution verrouillée, checksum officiel, MSRV amont 1.71 et audit frais sont vérifiés, sans exemption. La correction distincte du vérificateur de structure empêche un suffixe fictif de dossier d’être pris pour une racine personnelle Linux, tout en maintenant les rejets de chemins personnels. Elle ne fait pas partie des 149/18 sources Memory.

Le [contrôle avant le premier enregistrement](pre-record-gate.json) couvre tous les fichiers publics, y compris non suivis, avant création du reçu. Le [manifeste](proof-manifest.json) scelle **193 fichiers** de campagne. Le [registre isolé](recorder/README.md) contient **16 copies exactes** : les chemins du manifeste copié restent relatifs à la campagne Q8, pas au sous-dossier des pièces jointes. Les sorties ultérieures du registre et les observations d’intégration sont hors de ce manifeste antérieur à l’enregistrement.

Le reçu **`1d5c9a69-c8ae-4863-9117-a59b2615891f`** est [conservé](recorder/receipts/1d5c9a69-c8ae-4863-9117-a59b2615891f.json), avec [résumé](receipt-summary.json), [commandes exactes](record-execution.json), [statut](qualification-status.json) et [historique](qualification-history.json). Le statut est `reported-satisfied`, sans motif de péremption ; `buildMatchesSource=true`, `rebuildRequired=false`. `reviewRequired=true` et `externalSubjectRequiresRecheck=true` restent explicites. Ce reçu ne remplace ni la revue des réponses ni une nouvelle mesure du sujet externe. Aucune promotion de maturité n’est effectuée.

Les vérifications finales reconstruisent les sources/binaires et reproduisent exactement assessment.json. Documentation inspect/map/check et structure du dépôt passent ; l’avertissement préexistant de fraîcheur DOCS-CHECK-009 reste visible. La correction de structure est commise séparément en `14e972247b654c02f61474782f509244addbe87c` et [revue](review-repo-shape-integration.json). Les contrôles CI sur la tête finale, la fusion et la publication main-snapshot restent des résultats de livraison distincts, à observer sur la PR de cette branche ; aucun succès futur n’est présumé.

La [revue finale du bundle](review-final-bundle.json) approuve neuf contrôles reconstruits, dont les 193 empreintes, les 16 pièces, les 84 sessions, les 37 paires et la chaîne reçu/registre/statut/historique. Le contexte `dirty=true` au moment de l’enregistrement reste conservé ; il ne prouve pas un arbre Git propre. Le [déplacement du rapport](review-final-bundle-relocation.json) corrige uniquement son emplacement, octet pour octet, après la revue.

Une [note de présentation](evidence-format-note.json) conserve le seul avertissement de fin de fichier du rapport dérivé déjà scellé. L’acceptation locale est limitée à ce diagnostic exact et à son empreinte immuable ; les hooks du dépôt restent actifs.
