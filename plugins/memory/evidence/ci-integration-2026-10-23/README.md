---
title: Memory PR49 CI integration
status: active
owner: Elegy Memory
---

# Intégration de la qualification Memory

La PR49 a qualifié le produit `95bf15b` sur Codex installé `gpt-6.1-sol / max`. Sa CI sur `a753b4b` révèle des prérequis d'intégration. Aucune erreur de cette CI n'est assimilée à une erreur des réponses modèle.

Autorité : `AGENTS.md`, `plugins/memory/AGENTS.md`, `plugins/memory/CONTRIBUTING.md`, spec contextual-recall. L'utilisateur autorise push et merge après réussite des contrôles et exclut l'autre logiciel hôte.

1. Corriger le traitement des fichiers suivis vides par le vérificateur de structure ; prouver l'ancien échec puis l'acceptation du vide et le rejet d'un chemin local fictif.
2. Neutraliser trois chemins locaux dans des copies publiques de logs. Conserver les originaux dans le commit publié `a753b4b`, leurs empreintes et une transformation reproductible ; les anciens manifestes et reçus ne sont pas réécrits.
3. Exempter seulement dix détections Gitleaks historiques dont la valeur est le SHA-256 vérifié de `resource_auth.rs`. Garder toutes les autres détections actives.
4. Mettre à jour uniquement les dépendances vulnérables : quinn-proto 0.11.18, rustls 0.23.46, rustls-webpki 0.103.16, webpki-roots 1.0.10 et zip 9.0.5 (contrainte minimale 9.0.0) ; garder le MSRV 1.88 et les fonctions de validation des archives.
5. Après commit des sources finales, reconstruire CLI/MCP et lier leurs octets et objets Git. Réutiliser les suites toujours valides, exécuter les vérifications des consommateurs zip et les contrôles de rappel nécessaires. Une nouvelle campagne prospective reprend les 84 contextes et les critères figés de Q6 sans réécrire Q6.
6. Relire le delta, les preuves et la destination publique ; pousser sans force, vérifier le SHA distant, attendre chaque contrôle CI sur la tête finale. Fusionner seulement après réussite, par commit de merge préservant les sources qualifiées. Observer l'automatisation main-snapshot existante.

Arrêt : données réelles, configuration permanente, outil supplémentaire à installer, API payante, accès persistant, ambiguïté de sécurité ou échec de preuve exigeraient réévaluation. Les échecs restent visibles ; aucune exception générale de sécurité, diminution de seuil ou réécriture d'historique n'est permise. Aucun gain global ou financier n'est inféré.

Les nouveaux résultats seront consignés ici ; les preuves Q6 restent celles de leur source historique.

## Contrôles déjà observés

Le [triage lié à la tête CI](ci-failures.json) attribue séparément chaque échec. Le [registre des trois dérivés publics](redactions.json) conserve la reconstruction des originaux Git ; `python sanitize_evidence.py` et cinq tests passent. Le lecteur de fichiers du vérificateur accepte le vide et bloque une lecture impossible sans publier le détail de l'erreur ; les [deux régressions RED/GREEN](native-validation/repo-shape-final.json) conservent la détection de chemin local.

L'[audit de dépendances](native-validation/cargo-audit-03.json) sur les octets exacts du lockfile corrigé rapporte zéro vulnérabilité, avec l'avis d'abandon unsafe-libyaml et deux avertissements de versions retirées conservés. Deux problèmes de capture préalables (champ d'avertissement nullable, exécution sandbox) sont consignés et ne sont pas présentés comme des audits réussis.

La [revérification hors ligne Q6](native-validation/q6-recheck-02.json) reste `qualified-selected-configuration`, 84 fils et aucune erreur. Elle confirme uniquement le sujet historique 95bf15b. Une erreur du runner a d'abord partagé un log entre deux checks, puis écrit un nouveau fichier de résultat dans le mauvais dossier. La première capture est conservée dans le runtime local ; la nouvelle capture a cinq logs distincts vérifiés, et le [déplacement sans changement d'octets](native-validation/relocated-rechecks.json) est explicite. Aucun fichier Q5 existant n'a été modifié.

La [structure complète](native-validation/repo-shape-full.json) passe sans issue. Inspect/map/check et l'export de l'index documentaire passent via l'exécutable existant dont l'empreinte est enregistrée ; l'avertissement de fraîcheur préexistant reste non bloquant. Le commit final devra encore passer les mêmes contrôles en CI. Les contrôles de capture des sources rejettent une révision incorrecte et des sources non commises même avec Python `-O`.

La [revue CI](review-ci.json) et la [revue ZIP](review-zip.json) sont acceptées. La migration ZIP propage les erreurs de nom et conserve les permissions Unix dans les archives produites sous Windows ; le test v1 reste présent et le test v3 vérifie binaire 0755 et manifeste 0644. La fixture préparée pour UTF-8 endommait aussi la longueur centrale : elle est corrigée et nommée comme contrôle d’archive malformée, sans inventer une politique de rejet UTF-8. Les premiers résultats natifs ZIP sont des résumés observés explicitement signalés, sans flux bruts reconstruits. Une capture brute des suites sur les sources commises et la CI finale compléteront la preuve.
