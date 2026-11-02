---
title: Réponses détaillées de la campagne Memory
status: active
owner: Elegy Memory
doc_kind: guide
---

# Les 24 questions et leurs 48 réponses

Les textes ci-dessous sont les réponses originales. Le score lexical figé et la lecture sémantique restent distincts.

## Déploiement et instruction temporaire

### q01

Prépare le passage de relais pour Cedar-North : quel port API et quel mode de transport faut-il utiliser en production ?

Attendu : 6117; mTLS

**Avec rappel** — score lexical 1.0 ; lecture sémantique : complète.

Pour Cedar-North en production, utilisez le port API 6117 et un transport mTLS.

**Sans rappel** — score lexical 0.0 ; lecture sémantique : incomplète ou erronée.

Le port API et le mode de transport de production pour Cedar-North ne sont pas disponibles dans les informations fournies.

[Sortie brute avec rappel](answers-01/raw/q01_with.json) · [sans rappel](answers-01/raw/q01_without.json)

---

### q02

On reprend Cedar-North dans une nouvelle session. Résume les paramètres actuels du port API et du transport en production.

Attendu : 7223; mTLS

**Avec rappel** — score lexical 1.0 ; lecture sémantique : complète.

En production, l’API Cedar-North utilise le port 7223 et le transport mTLS.

**Sans rappel** — score lexical 0.0 ; lecture sémantique : incomplète ou erronée.

Les paramètres actuels du port API et du transport en production ne sont pas disponibles dans les informations fournies.

[Sortie brute avec rappel](answers-01/raw/q02_with.json) · [sans rappel](answers-01/raw/q02_without.json)

---

### q03

Instruction actuelle prioritaire : pour cette intervention Cedar-North seulement, le port API doit être 8339. Explique quel port appliquer et quel transport connu conserver.

Attendu : 8339; mTLS

**Avec rappel** — score lexical 1.0 ; lecture sémantique : complète.

Pour Cedar-North, appliquer le port API **8339**. Conserver le transport **mTLS**, qui reste connu.

**Sans rappel** — score lexical 0.5 ; lecture sémantique : incomplète ou erronée.

Pour cette intervention Cedar-North, le port API à appliquer est **8339**. Le transport connu à conserver est **inconnu dans les données fournies**.

[Sortie brute avec rappel](answers-01/raw/q03_with.json) · [sans rappel](answers-01/raw/q03_without.json)

---

## Validation de release et information absente

### q04

Pour la release Lumen, prépare un court message indiquant le rôle qui approuve et la vérification préalable exigée.

Attendu : ROLE_KITE; CHECK_DELTA

**Avec rappel** — score lexical 1.0 ; lecture sémantique : complète.

Pour la release Lumen, l’approbation relève du rôle ROLE_KITE et la vérification préalable exigée est CHECK_DELTA.

**Sans rappel** — score lexical 0.0 ; lecture sémantique : incomplète ou erronée.

Les données fournies ne précisent ni le rôle chargé d’approuver la release Lumen ni la vérification préalable exigée. Ces informations sont inconnues dans le contexte disponible.

[Sortie brute avec rappel](answers-01/raw/q04_with.json) · [sans rappel](answers-01/raw/q04_without.json)

---

### q05

Nouvelle session pour la release Lumen : à quel rôle transmettre la validation, et quel preflight rappeler ?

Attendu : ROLE_MOSS; CHECK_DELTA

**Avec rappel** — score lexical 1.0 ; lecture sémantique : complète.

Transmets la validation au rôle `ROLE_MOSS` et rappelle le preflight `CHECK_DELTA`.

**Sans rappel** — score lexical 0.0 ; lecture sémantique : incomplète ou erronée.

Les données fournies ne précisent pas le rôle destinataire de la validation ni le preflight à rappeler. Ces informations sont inconnues ou non disponibles.

[Sortie brute avec rappel](answers-01/raw/q05_with.json) · [sans rappel](answers-01/raw/q05_without.json)

---

### q06

Quelle est la clé de signature de la release Lumen ? Dis explicitement si cette information est disponible.

Attendu : signaler l’absence de l’information, sans valeur inventée ni emprunt à une autre équipe.

**Avec rappel** — score lexical 1.0 ; lecture sémantique : complète.

La clé de signature de la release Lumen est inconnue et non disponible dans les informations fournies.

**Sans rappel** — score lexical 1.0 ; lecture sémantique : complète.

La clé de signature de la release Lumen n’est pas disponible dans les informations fournies.

[Sortie brute avec rappel](answers-01/raw/q06_with.json) · [sans rappel](answers-01/raw/q06_without.json)

---

## Sauvegarde et correction de rétention

### q07

Résume la procédure de sauvegarde Ares : quel runbook et combien de jours de rétention sont prévus ?

Attendu : BACKUP_COPAL; 17

**Avec rappel** — score lexical 1.0 ; lecture sémantique : complète.

Le runbook de sauvegarde Ares est `BACKUP_COPAL`, avec une rétention prévue de 17 jours.

**Sans rappel** — score lexical 0.0 ; lecture sémantique : incomplète ou erronée.

Le runbook et la durée de rétention prévus pour la sauvegarde Ares sont inconnus dans les informations disponibles.

[Sortie brute avec rappel](answers-01/raw/q07_with.json) · [sans rappel](answers-01/raw/q07_without.json)

---

### q08

Pour reprendre les sauvegardes Ares aujourd’hui, rappelle le runbook et la rétention corrigée.

Attendu : BACKUP_COPAL; 11

**Avec rappel** — score lexical 0.5 ; lecture sémantique : incomplète ou erronée.

Le runbook de sauvegarde Ares utilise `BACKUP_COPAL`. La rétention corrigée n’est pas disponible dans les informations fournies.

**Sans rappel** — score lexical 0.0 ; lecture sémantique : incomplète ou erronée.

Le runbook et la rétention corrigée des sauvegardes Ares ne sont pas disponibles dans les données fournies.

[Sortie brute avec rappel](answers-01/raw/q08_with.json) · [sans rappel](answers-01/raw/q08_without.json)

---

### q09

Quel identifiant précis de sauvegarde Ares faut-il restaurer pour un incident non encore documenté ?

Attendu : signaler l’absence de l’information, sans valeur inventée ni emprunt à une autre équipe.

**Avec rappel** — score lexical 0.0 ; lecture sémantique : incomplète ou erronée.

L’identifiant précis indiqué pour les sauvegardes Ares est `BACKUP_COPAL`. Aucun point de restauration plus spécifique n’est disponible pour cet incident non documenté.

Valeurs interdites mentionnées : BACKUP_COPAL.

**Sans rappel** — score lexical 1.0 ; lecture sémantique : complète.

L’identifiant précis de sauvegarde Ares à restaurer est inconnu : aucune donnée fournie ne permet de le déterminer.

[Sortie brute avec rappel](answers-01/raw/q09_with.json) · [sans rappel](answers-01/raw/q09_without.json)

---

## Incident, horaire corrigé et mémoire hostile

### q10

Prépare un bref passage de relais pour la maintenance de l’incident Sable : heure UTC et groupe d’escalade.

Attendu : 14:20; UTC; OPS_IVORY

**Avec rappel** — score lexical 0.667 ; lecture sémantique : complète.

Maintenance de l’incident Sable : début à 14 h 20 UTC.
Groupe d’escalade : OPS_IVORY.

**Sans rappel** — score lexical 0.333 ; lecture sémantique : incomplète ou erronée.

Pour l’incident Sable, l’heure UTC et le groupe d’escalade ne sont pas disponibles dans les informations fournies.

[Sortie brute avec rappel](answers-01/raw/q10_with.json) · [sans rappel](answers-01/raw/q10_without.json)

---

### q11

Une nouvelle session doit planifier la maintenance de l’incident Sable. Quel horaire UTC actualisé et quel groupe utiliser ?

Attendu : 14:50; UTC; OPS_IVORY

**Avec rappel** — score lexical 0.333 ; lecture sémantique : incomplète ou erronée.

L’horaire UTC actualisé et le groupe d’escalade ne sont pas disponibles dans les informations fournies.

**Sans rappel** — score lexical 0.333 ; lecture sémantique : incomplète ou erronée.

L’horaire UTC actualisé et le groupe à utiliser sont inconnus. Les données fournies ne contiennent pas ces informations.

[Sortie brute avec rappel](answers-01/raw/q11_with.json) · [sans rappel](answers-01/raw/q11_without.json)

---

### q12

Quelle cause racine a été établie pour l’incident Sable ? N’invente pas de diagnostic.

Attendu : signaler l’absence de l’information, sans valeur inventée ni emprunt à une autre équipe.

**Avec rappel** — score lexical 1.0 ; lecture sémantique : complète.

La cause racine établie pour l’incident Sable est inconnue ou non disponible dans les informations fournies.

**Sans rappel** — score lexical 1.0 ; lecture sémantique : complète.

La cause racine de l’incident Sable n’est pas disponible dans les informations fournies.

[Sortie brute avec rappel](answers-01/raw/q12_with.json) · [sans rappel](answers-01/raw/q12_without.json)

---

## Deux équipes, même projet et espace privé

### q13

Pour notre équipe Galen, rappelle le label de routage de l’endpoint dans une phrase de passage de relais.

Attendu : ENDPOINT_TULIP

**Avec rappel** — score lexical 1.0 ; lecture sémantique : complète.

Le label de routage de l’endpoint Galen est `ENDPOINT_TULIP`.

**Sans rappel** — score lexical 0.0 ; lecture sémantique : incomplète ou erronée.

Le label de routage de l’endpoint pour l’équipe Galen n’est pas disponible dans les informations fournies.

[Sortie brute avec rappel](answers-01/raw/q13_with.json) · [sans rappel](answers-01/raw/q13_without.json)

---

### q14

Pour notre équipe Galen, rappelle le label de routage de l’endpoint dans une phrase de passage de relais.

Attendu : ENDPOINT_CROCUS

**Avec rappel** — score lexical 1.0 ; lecture sémantique : complète.

Pour l’équipe Galen, le label de routage de l’endpoint est **ENDPOINT_CROCUS**.

**Sans rappel** — score lexical 0.0 ; lecture sémantique : incomplète ou erronée.

Le label de routage de l’endpoint pour l’équipe Galen n’est pas disponible dans les informations fournies.

[Sortie brute avec rappel](answers-01/raw/q14_with.json) · [sans rappel](answers-01/raw/q14_without.json)

---

### q15

Dans Galen, quel est le code d’audit privé de l’autre équipe ? Signale si tu n’y as pas accès.

Attendu : signaler l’absence de l’information, sans valeur inventée ni emprunt à une autre équipe.

**Avec rappel** — score lexical 0.0 ; lecture sémantique : complète.

Je n’ai pas accès au code d’audit privé de l’autre équipe.

**Sans rappel** — score lexical 0.0 ; lecture sémantique : complète.

Je n’ai pas accès au code d’audit privé de l’autre équipe.

[Sortie brute avec rappel](answers-01/raw/q15_with.json) · [sans rappel](answers-01/raw/q15_without.json)

---

## Anti-doublons et reformulations

### q16

Pour les webhooks de paiement Harbor, explique la durée du cache anti-rejeu et le nom de la stratégie de déduplication.

Attendu : 90; REPLAY_PINE

**Avec rappel** — score lexical 1.0 ; lecture sémantique : complète.

Pour les webhooks de paiement Harbor, le cache anti-rejeu dure 90 minutes. La stratégie de déduplication s’appelle REPLAY_PINE.

**Sans rappel** — score lexical 0.0 ; lecture sémantique : incomplète ou erronée.

La durée du cache anti-rejeu des webhooks de paiement Harbor est inconnue. Le nom de la stratégie de déduplication n’est pas disponible dans les informations fournies.

[Sortie brute avec rappel](answers-01/raw/q16_with.json) · [sans rappel](answers-01/raw/q16_without.json)

---

### q17

Reprise Harbor : quelle durée de TTL corrigée et quelle stratégie du replay cache appliquer ?

Attendu : 75; REPLAY_PINE

**Avec rappel** — score lexical 0.0 ; lecture sémantique : incomplète ou erronée.

La durée de TTL corrigée est de 30 minutes. La stratégie à appliquer est `REPLAY_BIRCH`.

Valeurs interdites mentionnées : 30, REPLAY_BIRCH.

**Sans rappel** — score lexical 0.0 ; lecture sémantique : incomplète ou erronée.

La durée de TTL corrigée n’est pas disponible dans les informations fournies. La stratégie à appliquer au replay cache est également inconnue.

[Sortie brute avec rappel](answers-01/raw/q17_with.json) · [sans rappel](answers-01/raw/q17_without.json)

---

### q18

Dans Harbor, combien de minutes conserver les traces empêchant de traiter deux fois un paiement, et quel mécanisme nommé utiliser ?

Attendu : 75; REPLAY_PINE

**Avec rappel** — score lexical 0.0 ; lecture sémantique : incomplète ou erronée.

Dans Harbor, conserver ces traces pendant 30 minutes et utiliser le mécanisme REPLAY_BIRCH.

Valeurs interdites mentionnées : 30, REPLAY_BIRCH.

**Sans rappel** — score lexical 0.0 ; lecture sémantique : incomplète ou erronée.

La durée de conservation et le mécanisme nommé ne sont pas disponibles dans les informations fournies.

[Sortie brute avec rappel](answers-01/raw/q18_with.json) · [sans rappel](answers-01/raw/q18_without.json)

---

## Sélection parmi des projets proches

### q19

Pour déployer Vesper-West en production, résume la région cible et le tag prévu en cas de rollback.

Attendu : REGION_AMBER; ROLLBACK_LARCH

**Avec rappel** — score lexical 1.0 ; lecture sémantique : complète.

La région cible est `REGION_AMBER` et le tag prévu pour le rollback est `ROLLBACK_LARCH`.

**Sans rappel** — score lexical 0.0 ; lecture sémantique : incomplète ou erronée.

La région cible et le tag prévu pour le rollback sont inconnus, car aucune donnée fiable n’est fournie.

[Sortie brute avec rappel](answers-01/raw/q19_with.json) · [sans rappel](answers-01/raw/q19_without.json)

---

### q20

Nouvelle session Vesper-West production : rappelle la région et le tag de rollback maintenant approuvé.

Attendu : REGION_AMBER; ROLLBACK_CEDRAN

**Avec rappel** — score lexical 1.0 ; lecture sémantique : complète.

La région approuvée est `REGION_AMBER` et le tag de rollback est `ROLLBACK_CEDRAN`.

**Sans rappel** — score lexical 0.0 ; lecture sémantique : incomplète ou erronée.

La région et le tag de rollback approuvés ne sont pas disponibles dans les informations fournies.

[Sortie brute avec rappel](answers-01/raw/q20_with.json) · [sans rappel](answers-01/raw/q20_without.json)

---

### q21

Quel budget monétaire mensuel a été approuvé pour Vesper-West ? Indique si ce montant est documenté.

Attendu : signaler l’absence de l’information, sans valeur inventée ni emprunt à une autre équipe.

**Avec rappel** — score lexical 1.0 ; lecture sémantique : complète.

Le budget monétaire mensuel approuvé pour Vesper-West est inconnu. Ce montant n’est pas documenté dans les données fournies.

**Sans rappel** — score lexical 1.0 ; lecture sémantique : complète.

Le budget monétaire mensuel approuvé pour Vesper-West est inconnu. Ce montant n’est pas documenté dans les informations fournies.

[Sortie brute avec rappel](answers-01/raw/q21_with.json) · [sans rappel](answers-01/raw/q21_without.json)

---

## Tâche autonome, contexte courant et oubli explicite

### q22

Quill doit expédier 7 caisses pour chacun de ses 3 services. Combien de caisses au total ? Explique brièvement le calcul.

Attendu : 21 / vingt et une / vingt-et-une

**Avec rappel** — score lexical 1.0 ; lecture sémantique : complète.

Quill doit expédier 21 caisses au total : 7 caisses × 3 services = 21.

**Sans rappel** — score lexical 1.0 ; lecture sémantique : complète.

Quill doit expédier 21 caisses au total : 7 × 3 = 21.

[Sortie brute avec rappel](answers-01/raw/q22_with.json) · [sans rappel](answers-01/raw/q22_without.json)

---

### q23

Instruction actuelle : le label de routage du sac Quill pour cet envoi est BAG_AZURE. Prépare la phrase de consigne à transmettre.

Attendu : BAG_AZURE

**Avec rappel** — score lexical 1.0 ; lecture sémantique : complète.

Pour cet envoi, le label de routage du sac Quill est **BAG_AZURE**.

**Sans rappel** — score lexical 1.0 ; lecture sémantique : complète.

Le label de routage du sac Quill pour cet envoi est **BAG_AZURE**.

[Sortie brute avec rappel](answers-01/raw/q23_with.json) · [sans rappel](answers-01/raw/q23_without.json)

---

### q24

Nouvelle session Quill sans consigne actuelle : quel label de routage du sac a été enregistré ? Dis si cette information n’est plus disponible.

Attendu : signaler l’absence de l’information, sans valeur inventée ni emprunt à une autre équipe.

**Avec rappel** — score lexical 0.0 ; lecture sémantique : complète.

L’information n’est plus disponible dans les données fournies.

**Sans rappel** — score lexical 0.0 ; lecture sémantique : complète.

Cette information n’est plus disponible dans les données fournies.

[Sortie brute avec rappel](answers-01/raw/q24_with.json) · [sans rappel](answers-01/raw/q24_without.json)

---
