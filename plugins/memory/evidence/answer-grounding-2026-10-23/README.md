# Memory — diagnostic des réponses, 23 octobre 2026

**Les erreurs de réponse ne sont pas résolues. Aucun push ni merge.** Les quatre
diagnostics conservés ne justifient pas une nouvelle modification du moteur.
Ils permettent de rejeter des correctifs de formulation et de présentation sans
les intégrer au produit. Le correctif de sélection Harbor reste validé ; il ne
répare pas les omissions et substitutions qui surviennent pendant la rédaction.

Une décision de cible est en attente : le proxy de réponses utilise
`gpt-5.6-luna / medium`, alors que la configuration Codex normale observée utilise
`gpt-6.1-sol / max`. Aucun résultat Sol n'est déduit de cet écart. Une éventuelle
qualification sur la configuration réelle doit constituer une nouvelle cohorte,
avec critères fixés avant exécution et échecs Luna conservés. Changer de modèle
ne constitue pas une correction du moteur ni une réussite rétroactive de Luna.
Les contrôles locaux et leurs limites figurent dans [local-checks.json](local-checks.json).

## Résultats et erreurs individuelles

Les 129 contextes de diagnostic ont tous terminé sans erreur d'exécution ni
d'appel d'outil observé. Leur réussite d'exécution ne vaut pas réussite produit.
Les questions, valeurs attendues et faits sont conservés ; chaque essai possède
sa sortie JSONL brute, son contexte, ses empreintes et son identifiant de session.

| Diagnostic | Variable et résultat |
|---|---|
| [Formulation, 30 réponses](diagnostic-01/run.json) | Préfixe courant contre préfixe générique de 197 octets : 10/15 réponses complètes dans chaque condition. La rétention q08 est omise et le runbook est substitué à l'identifiant inconnu q09. [Revue](diagnostic-01/review.json). |
| [Présentation, 33 réponses](layout-01/run.json) | Texte placé avant les métadonnées, puis ablation des métadonnées : les erreurs persistent. L'ablation est inéligible au produit, car elle supprime les références. [Revue et exceptions lexicales](layout-01/review.json). |
| [Sens du préfixe, 44 réponses](semantics-01/run.json) | Plan factoriel : historique/dernière version stockée et règle générique/distinction méthode-instance. Le candidat combiné réussit 8/11 réponses en lecture stricte : q08/t3 omet 11 ; q09/t1 et t2 substituent le runbook. Candidat rejeté. [44 verdicts de prose](semantics-01/review.json). |
| [Placement, 22 réponses](boundary-01/run.json) | Le même préfixe courant est déplacé avant les données : 6/11 réponses satisfaisantes contre 5/11 dans le contrôle. q08/t1 et t3 omettent 11 ; q09/t1 substitue le runbook ; q09/t3 mélange les attributs et répète la valeur interdite ; q11/t1 fournit l'heure mais nie qu'un horaire actualisé soit connu. Candidat rejeté. [22 verdicts de prose](boundary-01/review.json). |

Les comptages de prose proviennent d'une revue indépendante par agent, puis
d'une lecture par l'orchestrateur. Ils ne sont pas présentés comme une revue
humaine. « 14 h 50 » équivaut à « 14:50 » : le score lexical gelé reste conservé,
avec ses faux négatifs explicités. À l'inverse, citer une heure puis dire qu'elle
est inconnue peut satisfaire les atomes lexicaux sans satisfaire la demande.
L'ambiguïté q11 du dernier diagnostic est comptée comme échec strict.

Les deux conditions de placement passent dans un même message utilisateur ;
aucun rôle technique `system` ou `developer` distinct n'a été testé. Ces essais
ne permettent pas d'attribuer causalement tous les échecs au modèle ou au hôte.
Ils ne justifient pas une amélioration globale, un gain de temps ou une économie.

## Garanties et portée

| Garantie | Verdict actuel |
|---|---|
| Reprise entre sessions du même logiciel | Les preuves de persistance et de correction dans des processus distincts restent [celles de la qualification précédente](../recall-fixes-2026-10-23/README.md). Ces diagnostics réutilisent ses paquets ; ils ne requalifient pas l'activation automatique de l'application. |
| Isolation entre propriétaires, correction, suppression, données hostiles | Contrôles précédents réutilisés sur le même code produit. Aucun accès à la mémoire réelle pour ces diagnostics ; aucun nouveau verdict universel d'absence de contamination. |
| Autre logiciel hôte | Exclu du présent critère par l'utilisateur. Le résultat historique bloqué est préservé ; aucune réussite sur un second hôte n'est annoncée. |
| Qualité des réponses Luna/medium | Non qualifiée. Les omissions, confusions d'attributs et abstentions incorrectes persistent malgré les faits fournis. |
| Qualité sur le Codex réellement configuré | Non testée ici. Décision de cible en attente ; ne pas transposer les résultats Luna ou déclarer Sol meilleur. |

## Code et exécutables concernés

Ces diagnostics ne construisent ni n'exécutent une nouvelle version de Memory.
Leurs données d'entrée sont les sorties réelles conservées du CLI et du MCP de
la [qualification source/binaire précédente](../recall-fixes-2026-10-23/subject.json),
source produit `b828223bd9897c559a4ed9ed39f3c17f34284033`, version `0.1.0` :

- CLI SHA-256 : `4f6fdaa80b0c8809af8b217a786232b0d71cfa30f5130f0fc1323e1513cae833`.
- MCP SHA-256 : `d93610c224af1549708bc7f3fff2b51b9b4601bc4bfc480fdf0588748ba86f93`.
- Empreinte canonique source CLI : `52f9318b24099cdafb5a7aca6ec1dbac16cf5e6dd05f1352b72801ec9ebece70`.

Les 408 tests Memory, 67 tests MCP, 14 tests du hook et les contrôles statiques
déjà passés sont réutilisés ; aucune nouvelle modification du code produit ne
les invalide. Le vérificateur précédent a de nouveau confirmé la cohérence des
preuves et du verrou de source. Après revue et durcissement du harness, les
[33 tests Python finaux](local-checks.json) passent ; les
[22 contrôles de préparation antérieurs](harness-checks.json) sont conservés
séparément. Aucune campagne complète de transfert n'a encore été exécutée.

Le modèle de diagnostic reste `gpt-5.6-luna`, effort `medium`, client Codex
installé et authentification ChatGPT existante. Les empreintes du client, de sa
configuration, des scripts et des protocoles sont liées aux manifestes. Aucun
appel d'API payante, installation, configuration permanente ni accès persistant
n'a été ajouté. Les modèles distants et leurs paramètres internes ne sont pas
scellés.

## Protocole et reproduction

Les plans expérimentaux précèdent les réponses : [protocole commun](protocol.json),
[formulation](diagnostic-protocol.json), [présentation](layout-protocol.json),
[sens](semantics-protocol.json), [placement](boundary-protocol.json). Seul le
facteur annoncé varie dans chaque essai. Les faits et questions du
[corpus initial](corpus.json) restent inchangés.

Le [corpus de transfert](transfer-corpus.json) contient douze nouvelles questions
fictives, de nouveaux noms et valeurs, avec corrections, voisins, abstention,
instruction hostile, priorité actuelle et suppression. Il est gelé mais **pas
encore exécuté**. Sa structure est proche du corpus initial ; ce n'est pas une
preuve indépendante d'usage quotidien. Le [runner générique](qualification_runner.py)
accepte les deux corpus sans remplacement de fichiers et distingue replay,
réponses appariées et répétitions explicitement numérotées.

Depuis la racine de ce checkout, contrôles hors ligne sans modèle ni base Memory :

```powershell
$q = 'plugins/memory/evidence/answer-grounding-2026-10-23'
python "$q/verify_diagnostics.py"
python 'plugins/memory/evidence/recall-fixes-2026-10-23/verify_evidence.py'
```

Le premier succès signifie `evidence-consistent`, avec `productQualified=false`.
Le second conserve lui aussi le verdict produit défavorable. Pour les tests du
harness, depuis le dossier `$q` ; huit tests du vérificateur
exercent notamment le refus d'altérations des réponses, contextes, hashes et
instructions, ainsi que le lien aux replays et au barème conservés :

```powershell
python -m unittest test_qualification_runner test_grade test_replay test_verify_diagnostics -v
```

Une reproduction des modèles doit utiliser un nouveau dossier de sortie et
conserver tous les essais, même défavorables. Elle consomme le quota ChatGPT
existant et ne reproduit pas forcément la même réponse distante. Le
[plan de correction](../../../../docs/plans/memory-recall-quality-fixes.md)
conserve les hypothèses rejetées et les conditions de reprise.
