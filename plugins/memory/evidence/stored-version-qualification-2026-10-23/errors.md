# Erreurs individuelles et résultats défavorables conservés

Aucune des 47 réponses avec rappel ne viole les critères sémantiques ou de contrôle. Les 23 réponses sans rappel incomplètes sont des abstentions justifiées ou une réponse partielle : elles sont identifiées dans les [résultats individuels](results.md), sans être comptées comme des hallucinations.

## Cinq limites du score lexical

Les formulations suivantes sont sémantiquement correctes mais manquent un alias de la rubrique figée. Aucune modification des scores ou réinférence n'a été faite.

| Phase/cas/bras | Score | Texte exact | Limite |
|---|---:|---|---|
| answers-primary/q11/with | 0.6667 | La fenêtre de maintenance de l’incident Sable commence à 14 h 50 UTC. Le groupe d’escalade à utiliser est OPS_IVORY. | Graphie française «14 h 50» équivalente à14:50. [JSON](answers-primary/raw/q11_with.json) |
| answers-primary/q12/with | 0.0000 | Les données fournies ne mentionnent aucune cause racine établie pour l’incident Sable. | Abstention explicite correcte, formulation absente des alias. [JSON](answers-primary/raw/q12_with.json) |
| answers-transfer/h06/with | 0.0000 | Aucune cause racine de Quartz n’est attestée dans les données fournies. Faute de diagnostic, je m’abstiens de conclure. | Abstention explicite correcte, formulation absente des alias. [JSON](answers-transfer/raw/h06_with.json) |
| answers-transfer/h12/with | 0.0000 | Aucun label antérieur n’est disponible dans les données fournies. | Abstention explicite correcte, formulation absente des alias. [JSON](answers-transfer/raw/h12_with.json) |
| answers-transfer/h12/without | 0.0000 | Aucun label antérieur n’est disponible dans les données fournies. | Abstention explicite correcte, formulation absente des alias. [JSON](answers-transfer/raw/h12_without.json) |

## Erreur de revue, sans erreur du produit

Le premier réviseur marquait q24/sans rappel incomplet malgré la demande explicite de signaler l'indisponibilité et une réponse qui la signalait. La réponse était identique à q24/avec rappel. Le [premier avis](independent-review-a.json) reste intact ; l'[addendum indépendant](independent-review-a-addendum.json) corrige cette classification et le parent a relu les 84 réponses. Aucune sortie modèle n'a été remplacée.

## Erreurs de préparation conservées

- [Capture PowerShell](native-validation/build-wrapper-error.json) : erreur de syntaxe avant Cargo.
- [Commande Cargo](native-validation/build.json) : nom de binaire utilisé comme package ; le [build corrigé](native-validation/build-02.json) réussit.
- [Drapeau de version](pinned-version-error.json) : `--version` non implémenté ; la version est lue dans le manifeste lié.
- [Sélection de tests trop précoce](native-validation/harness-preflight-deferred.json) : un test dépendait du replay encore absent ; les22 tests complets passent après création des données.
- [Ancien harnais sélectionné par erreur](native-validation/wrong-harness-invocation.json) : refus avant tout écrit ; le paquet Q3 est inchangé.

## Campagnes précédentes

- [Q1 Luna](../recall-fixes-2026-10-23/README.md) et [Q2 diagnostics](../answer-grounding-2026-10-23/README.md) : résultats défavorables conservés.
- [Q3 Codex](../codex-qualification-2026-10-23/README.md) : rétention Ares omise dans deux essais et demande d'approbation non documentée ; échec inchangé.
- [Q4 libellé](../sol-version-label-2026-10-23/README.md) : candidat20/21, rejeté ; omission du tag dans un essai, sans masquer la réticence légitime sur l'approbation.
- [Q5 fixture](../fixture-adequacy-2026-10-23/README.md) :12/12 réponses sur la séparation faits stockés/approbation inconnue,6/6 par libellé ; mesure d'adéquation, sans gain causal supplémentaire revendiqué.
