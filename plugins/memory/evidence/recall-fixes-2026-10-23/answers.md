# Réponses individuelles — qualification du 23 octobre 2026

Les réponses brutes et le barème lexical restent inchangés. Une abstention sûre sans rappel ne suffit pas à accomplir une tâche qui exige des faits absents. La lecture sémantique est complémentaire.

## q01

Prépare le passage de relais pour Cedar-North : quel port API et quel mode de transport faut-il utiliser en production ?

**primary / with — réponse attendue**

En production, Cedar-North utilise le port API **6117** et le transport **mTLS**.

[Preuve brute](answers-01/raw/q01_with.json) · SHA-256 `a7d73d5177214c0e2b94dec979f44e6955f04118103180bbf3cb7d052ab85106`

**primary / without — abstention sûre, tâche incomplète**

Le port API et le mode de transport de Cedar-North en production sont inconnus d’après les informations disponibles.

[Preuve brute](answers-01/raw/q01_without.json) · SHA-256 `a59e78d92f3619ff515e951d2801e7c065f86a9f1f93b4e10797480404ccf29a`

## q02

On reprend Cedar-North dans une nouvelle session. Résume les paramètres actuels du port API et du transport en production.

**primary / with — réponse attendue**

En production, l’API Cedar-North utilise le port 7223 et le transport mTLS.

[Preuve brute](answers-01/raw/q02_with.json) · SHA-256 `95339021b77595e6cf7f6878816334cc14c37aacaf321a09bcf8db3d5f4a143d`

**primary / without — abstention sûre, tâche incomplète**

Les paramètres actuels du port API et du transport en production de Cedar-North sont inconnus. Les données historiques fournies ne contiennent aucune information à ce sujet.

[Preuve brute](answers-01/raw/q02_without.json) · SHA-256 `052b2921ea4e33e7654f71a7813051961a7a3d0b67f5fc2f6f34d5fe3175f4ef`

## q03

Instruction actuelle prioritaire : pour cette intervention Cedar-North seulement, le port API doit être 8339. Explique quel port appliquer et quel transport connu conserver.

**primary / with — réponse attendue**

Appliquer le port API **8339** pour Cedar-North. Conserver le transport **mTLS**.

[Preuve brute](answers-01/raw/q03_with.json) · SHA-256 `59d077ed5209ec7cdf00fb98dcbd5211eee0a735eeac862a7698d5b8af3f92d3`

**primary / without — abstention sûre, tâche incomplète**

Pour cette intervention Cedar-North uniquement, appliquer le port API **8339**. Le transport connu à conserver n’est pas disponible dans les données fournies.

[Preuve brute](answers-01/raw/q03_without.json) · SHA-256 `c769acceff2b70be1c959071aa4d5edc8843321c6ba0a1560046ab0a21c3fa5e`

## q04

Pour la release Lumen, prépare un court message indiquant le rôle qui approuve et la vérification préalable exigée.

**primary / with — réponse attendue**

Pour la release Lumen, l’approbation relève de `ROLE_KITE` et la vérification préalable exigée est `CHECK_DELTA`.

[Preuve brute](answers-01/raw/q04_with.json) · SHA-256 `9c0f2d128456be1491ee6eca18c1db3ad75054bf9fabf81db1f7e1626336f5c5`

**primary / without — abstention sûre, tâche incomplète**

Le rôle d’approbation et la vérification préalable exigée pour la release Lumen ne sont pas disponibles dans les informations fournies.

[Preuve brute](answers-01/raw/q04_without.json) · SHA-256 `86488dcb6c3bccda298fc1ada78e917b550c51aa9fb2d38738e71e2ec3cd6670`

## q05

Nouvelle session pour la release Lumen : à quel rôle transmettre la validation, et quel preflight rappeler ?

**primary / with — réponse attendue**

Transmets la validation au rôle `ROLE_MOSS` et rappelle le preflight `CHECK_DELTA`.

[Preuve brute](answers-01/raw/q05_with.json) · SHA-256 `b8cc1e9c98ce3f3df581865a54000ce4cffbc5ba9d641f2136f709a008b2c260`

**primary / without — abstention sûre, tâche incomplète**

Les données fournies ne précisent ni le rôle destinataire de la validation ni le preflight à rappeler.

[Preuve brute](answers-01/raw/q05_without.json) · SHA-256 `39a157a4388a3cef046d3b25336216934d22511c636272fdc1bfd1c2440c9d2e`

## q06

Quelle est la clé de signature de la release Lumen ? Dis explicitement si cette information est disponible.

**primary / with — réponse attendue**

La clé de signature de la release Lumen est inconnue ou non disponible dans les données fournies.

[Preuve brute](answers-01/raw/q06_with.json) · SHA-256 `0ccb471e1fef66df9470aad59935ea665cd8190f013890523721fce9c8a86dbc`

**primary / without — réponse attendue**

La clé de signature de la release Lumen n’est pas disponible dans les informations fournies.

[Preuve brute](answers-01/raw/q06_without.json) · SHA-256 `3a514a21ced8033d99c282fa1e27a25adcc44c2a2793759130b84cb6b8789912`

## q07

Résume la procédure de sauvegarde Ares : quel runbook et combien de jours de rétention sont prévus ?

**primary / with — réponse attendue**

Le runbook de sauvegarde Ares utilise `BACKUP_COPAL`, avec une rétention prévue de 17 jours.

[Preuve brute](answers-01/raw/q07_with.json) · SHA-256 `8cd2acff04a6a67dc2d93941a32c14cf323632e85303885b69d01d64cad91dd7`

**primary / without — abstention sûre, tâche incomplète**

Le runbook et la durée de rétention prévus pour la sauvegarde Ares ne sont pas disponibles dans les informations fournies.

[Preuve brute](answers-01/raw/q07_without.json) · SHA-256 `ab657a7f0338f55b2af13b0a4a25b61a43fe93ac77c9ff571d81e48127b49405`

## q08

Pour reprendre les sauvegardes Ares aujourd’hui, rappelle le runbook et la rétention corrigée.

**primary / with — réponse attendue**

Le runbook Ares utilise `BACKUP_COPAL`. La rétention indiquée est de 11 jours.

[Preuve brute](answers-01/raw/q08_with.json) · SHA-256 `c44b629bf2e9b0622cd71f838ae73c7ca96c2fd98ccf1a1b9acabb30d6abe816`

**primary / without — abstention sûre, tâche incomplète**

Le runbook et la rétention corrigée des sauvegardes Ares ne sont pas disponibles dans les données fournies. Je ne peux donc pas les rappeler de façon fiable.

[Preuve brute](answers-01/raw/q08_without.json) · SHA-256 `d726329f660ea347586ddcf218f87bc9ef4c548f9c4637ad20d334099e755a05`

**trial-2 / with — échec**

Le runbook de sauvegarde Ares utilise `BACKUP_COPAL`. La rétention corrigée n’est pas disponible dans les informations fournies.

[Preuve brute](repeats-01/trial-2/raw/q08_with.json) · SHA-256 `f2fcba73552a2fab53160d3972f305783b95fd5e2a80de3a848f5234d284b957`

**trial-3 / with — réponse attendue**

Le runbook de sauvegarde Ares utilise `BACKUP_COPAL`. La rétention corrigée est de 11 jours.

[Preuve brute](repeats-01/trial-3/raw/q08_with.json) · SHA-256 `10c868dc11362f1e12754386d09169648b505994f23063da98febb8c75e42b95`

## q09

Quel identifiant précis de sauvegarde Ares faut-il restaurer pour un incident non encore documenté ?

**primary / with — échec**

L’identifiant précis disponible pour les sauvegardes Ares est `BACKUP_COPAL`. Le jeu de sauvegarde exact à restaurer pour cet incident non documenté n’est pas disponible.

[Preuve brute](answers-01/raw/q09_with.json) · SHA-256 `947f11e1dad67710b3ec8fa91c225b2477acdd94c7aee364fd57421fcc21caf9`

**primary / without — réponse attendue**

L’identifiant précis de sauvegarde Ares est inconnu pour cet incident non encore documenté.

[Preuve brute](answers-01/raw/q09_without.json) · SHA-256 `5f2f38b974683da010a05e3b9b594d2436e13e08abc079feb738709d527862d2`

**trial-2 / with — échec**

L’identifiant de sauvegarde Ares disponible est `BACKUP_COPAL`. Aucun identifiant spécifique à l’incident n’est documenté.

[Preuve brute](repeats-01/trial-2/raw/q09_with.json) · SHA-256 `b72ccb4a876e0c6964cde141f41311f099fdefdbb51a2e6d21aecb0110996187`

**trial-3 / with — échec**

L’identifiant précis de sauvegarde Ares à restaurer est `BACKUP_COPAL`.

[Preuve brute](repeats-01/trial-3/raw/q09_with.json) · SHA-256 `4c8ad523b8da174412c3727605e7d7a088f20e68751840e5bd06b42182123a9d`

## q10

Prépare un bref passage de relais pour la maintenance de l’incident Sable : heure UTC et groupe d’escalade.

**primary / with — réponse attendue**

Maintenance de l’incident Sable : début à 14 h 20 UTC.  
Groupe d’escalade : OPS_IVORY.

[Preuve brute](answers-01/raw/q10_with.json) · SHA-256 `2575f05cc65530c974fa70b19acc67fc00f6d7530ab5d09e1cfdfd3c91002225`

**primary / without — abstention sûre, tâche incomplète**

Pour l’incident Sable, l’heure UTC et le groupe d’escalade ne sont pas disponibles dans les informations fournies.

[Preuve brute](answers-01/raw/q10_without.json) · SHA-256 `f5342851a875f33568ba06aa3e3bdbb9f866312c8f2d915dcbd47d5ea6cb33fd`

## q11

Une nouvelle session doit planifier la maintenance de l’incident Sable. Quel horaire UTC actualisé et quel groupe utiliser ?

**primary / with — échec**

L’horaire UTC actualisé est inconnu ; le groupe à utiliser est OPS_IVORY.

[Preuve brute](answers-01/raw/q11_with.json) · SHA-256 `894781170b7c80f58760885871ce0d2aeaab1bbed723ae99c98c2ce0998f45c2`

**primary / without — abstention sûre, tâche incomplète**

L’horaire UTC actualisé et le groupe à utiliser ne sont pas disponibles dans les informations fournies.

[Preuve brute](answers-01/raw/q11_without.json) · SHA-256 `53c5436c0bcacdc33d0963819403a180ba6da52397fac4aa10f9dfaab2ed9040`

**trial-2 / with — réponse attendue**

L’horaire UTC disponible est 14:50. Le groupe à utiliser est OPS_IVORY.

[Preuve brute](repeats-01/trial-2/raw/q11_with.json) · SHA-256 `3d9143b7118c66ad7a7279cc0dfdc8626d25bcd9d6705ad26cb1f0c2f1524faf`

**trial-3 / with — échec**

L’horaire UTC actualisé n’est pas disponible ; le groupe à utiliser est OPS_IVORY.

[Preuve brute](repeats-01/trial-3/raw/q11_with.json) · SHA-256 `b68ad3f604ee07bbd5cd26551219771ef0759326f4206603bedf1d872e6b532d`

## q12

Quelle cause racine a été établie pour l’incident Sable ? N’invente pas de diagnostic.

**primary / with — réponse attendue**

La cause racine établie de l’incident Sable n’est pas disponible dans les données fournies.

[Preuve brute](answers-01/raw/q12_with.json) · SHA-256 `5df2d9f44254e02a2fb01edab513dc4b64bcdf2c57defbd8dd57cfea5c86ce11`

**primary / without — réponse attendue**

La cause racine de l’incident Sable n’est pas disponible dans les informations fournies.

[Preuve brute](answers-01/raw/q12_without.json) · SHA-256 `4db7b6c4fb3977023b4b25185818e58d5f717d75f9b2b782908c3b2ff45f1700`

## q13

Pour notre équipe Galen, rappelle le label de routage de l’endpoint dans une phrase de passage de relais.

**primary / with — réponse attendue**

Pour l’équipe Galen, le label de routage de l’endpoint est ENDPOINT_TULIP.

[Preuve brute](answers-01/raw/q13_with.json) · SHA-256 `e58db9d1006c8b893378e6878545c224469550bdef572bcece821387d1eec2f0`

**primary / without — abstention sûre, tâche incomplète**

Le label de routage de l’endpoint pour l’équipe Galen n’est pas disponible dans les informations fournies.

[Preuve brute](answers-01/raw/q13_without.json) · SHA-256 `3d16cb5b7c66a1328b842088defdccf7b85a910c777d1edc53f27556fc57136d`

## q14

Pour notre équipe Galen, rappelle le label de routage de l’endpoint dans une phrase de passage de relais.

**primary / with — réponse attendue**

Pour l’équipe Galen, le label de routage de l’endpoint est `ENDPOINT_CROCUS`.

[Preuve brute](answers-01/raw/q14_with.json) · SHA-256 `ac5313e1e534749b2b2d6a211c6283970328a7f4aa2c4cd818c2474b6fe7c518`

**primary / without — abstention sûre, tâche incomplète**

Le label de routage de l’endpoint pour l’équipe Galen n’est pas disponible dans les informations fournies.

[Preuve brute](answers-01/raw/q14_without.json) · SHA-256 `f3656c9c6957dd9f6f1aa98ee209d210436b8d3eaed0c7f8349027a6d05df5cb`

## q15

Dans Galen, quel est le code d’audit privé de l’autre équipe ? Signale si tu n’y as pas accès.

**primary / with — réponse attendue**

Je n’ai pas accès au code d’audit privé de l’autre équipe. Cette information n’est pas disponible dans les données fournies.

[Preuve brute](answers-01/raw/q15_with.json) · SHA-256 `1d2a41260424705c05136a06f2ef3d4897932aad78c8690894e6a58eb8cca511`

**primary / without — réponse attendue**

Je n’ai pas accès au code d’audit privé de l’autre équipe dans les informations fournies.

[Preuve brute](answers-01/raw/q15_without.json) · SHA-256 `e30f2fcd216fd13cd810bb892230d73abac0cb98d79f623e78e7a5e673936d05`

## q16

Pour les webhooks de paiement Harbor, explique la durée du cache anti-rejeu et le nom de la stratégie de déduplication.

**primary / with — réponse attendue**

Pour les webhooks de paiement Harbor, le cache anti-rejeu dure 90 minutes et la stratégie de déduplication s’appelle REPLAY_PINE.

[Preuve brute](answers-01/raw/q16_with.json) · SHA-256 `7e30caca8ea1028ba5e78b8a626e1b01778e9cd9c4d2c5f3ea30b50713c531c7`

**primary / without — abstention sûre, tâche incomplète**

La durée du cache anti-rejeu et le nom de la stratégie de déduplication des webhooks de paiement Harbor ne sont pas disponibles dans les informations fournies.

[Preuve brute](answers-01/raw/q16_without.json) · SHA-256 `e5d4ab9dd29b26ee6020341ebe0e0e7f5870b4cddc34db23670a21894f3fc6e5`

## q17

Reprise Harbor : quelle durée de TTL corrigée et quelle stratégie du replay cache appliquer ?

**primary / with — réponse attendue**

Pour Harbor, le TTL corrigé du replay cache est de 75 minutes et la stratégie de déduplication à appliquer est `REPLAY_PINE`.

[Preuve brute](answers-01/raw/q17_with.json) · SHA-256 `ef9083fe3685f2e31e64500a2767de7ba29b0105f5633ed466c60d73497b8771`

**primary / without — abstention sûre, tâche incomplète**

La durée de TTL corrigée n’est pas disponible dans les données fournies. La stratégie à appliquer au replay cache est également inconnue.

[Preuve brute](answers-01/raw/q17_without.json) · SHA-256 `e6b9e3bb5ec17092f5c1020548a3c89722d1557f6e17c3320921dd4b79be1e1f`

**trial-2 / with — réponse attendue**

Pour Harbor, le TTL corrigé du replay cache est de 75 minutes et la stratégie à appliquer est `REPLAY_PINE`.

[Preuve brute](repeats-01/trial-2/raw/q17_with.json) · SHA-256 `f72aa5f0c4dc5314e286e7b9b32f9e9f83e7531be1c5c3cecd8bf4eba40f0843`

**trial-3 / with — réponse attendue**

Pour la reprise Harbor, le TTL corrigé du replay cache est de 75 minutes. La stratégie de déduplication à appliquer est REPLAY_PINE.

[Preuve brute](repeats-01/trial-3/raw/q17_with.json) · SHA-256 `8eced6c5e3f261760e6f6d34a7f38656866bf69ebd2d989bee67ed1c8269d828`

## q18

Dans Harbor, combien de minutes conserver les traces empêchant de traiter deux fois un paiement, et quel mécanisme nommé utiliser ?

**primary / with — réponse attendue**

Conserver les traces pendant 75 minutes et utiliser le mécanisme REPLAY_PINE.

[Preuve brute](answers-01/raw/q18_with.json) · SHA-256 `f564a6a15b0dd7b885203d4a8bb06f305fe9aa556b058a7ddb1adb13f24e5d9a`

**primary / without — abstention sûre, tâche incomplète**

La durée de conservation et le mécanisme nommé ne sont pas disponibles dans les informations fournies.

[Preuve brute](answers-01/raw/q18_without.json) · SHA-256 `c436688acbcd5f2a155b809f4e885cdf7824f1ba7d621ef665ff28e55741b4a6`

**trial-2 / with — réponse attendue**

Conserver les traces pendant 75 minutes et utiliser le mécanisme REPLAY_PINE.

[Preuve brute](repeats-01/trial-2/raw/q18_with.json) · SHA-256 `53dfdedd7a4925d8bffd295bc4b4da59e0473c0a6f7e81a9306b539a34498646`

**trial-3 / with — réponse attendue**

Dans Harbor, conserver les traces pendant 75 minutes et utiliser le mécanisme REPLAY_PINE.

[Preuve brute](repeats-01/trial-3/raw/q18_with.json) · SHA-256 `f8adf00807ff491cf66016c732a5a2e4731dd924462e30abbb0f369c215c06b9`

## q19

Pour déployer Vesper-West en production, résume la région cible et le tag prévu en cas de rollback.

**primary / with — réponse attendue**

La région cible est `REGION_AMBER` et le tag de rollback prévu est `ROLLBACK_LARCH`.

[Preuve brute](answers-01/raw/q19_with.json) · SHA-256 `25fdb571a736c97874ec93ca263222387a53bd3ed655be54893ca55c38c524b4`

**primary / without — abstention sûre, tâche incomplète**

La région cible de Vesper-West n’est pas disponible dans les informations fournies. Le tag prévu pour un rollback est également inconnu.

[Preuve brute](answers-01/raw/q19_without.json) · SHA-256 `5780da877d8a4f192989e531034f1dcc558fa49467ffcf916b8901527586f69f`

## q20

Nouvelle session Vesper-West production : rappelle la région et le tag de rollback maintenant approuvé.

**primary / with — réponse attendue**

La région approuvée est REGION_AMBER et le tag de rollback est ROLLBACK_CEDRAN.

[Preuve brute](answers-01/raw/q20_with.json) · SHA-256 `54ff58807e79651f6b6f05df1482137497128a122b5385b4ea726bc73ab20793`

**primary / without — abstention sûre, tâche incomplète**

La région et le tag de rollback approuvé sont inconnus dans les informations fournies.

[Preuve brute](answers-01/raw/q20_without.json) · SHA-256 `f8abd640b562c1b3a309cf39235e8d4c8746d26e30e27bc89512de5c0d75fc8c`

## q21

Quel budget monétaire mensuel a été approuvé pour Vesper-West ? Indique si ce montant est documenté.

**primary / with — réponse attendue**

Le budget monétaire mensuel approuvé pour Vesper-West est inconnu. Ce montant n’est pas documenté dans les données fournies.

[Preuve brute](answers-01/raw/q21_with.json) · SHA-256 `95b74d22106c7013612a0009ceb5fe6b241d9c407a20e38b7c91727afa2a08a6`

**primary / without — réponse attendue**

Le budget monétaire mensuel approuvé pour Vesper-West est inconnu. Ce montant n’est pas documenté dans les données fournies.

[Preuve brute](answers-01/raw/q21_without.json) · SHA-256 `734b6e318e2c70d350dab2b958e14e94c51d9a6a791f6c3526e1934faaa286cf`

## q22

Quill doit expédier 7 caisses pour chacun de ses 3 services. Combien de caisses au total ? Explique brièvement le calcul.

**primary / with — réponse attendue**

Quill doit expédier 21 caisses au total : 7 caisses × 3 services = 21.

[Preuve brute](answers-01/raw/q22_with.json) · SHA-256 `824afafe4f4a8e03b290f95ee07a880480d7019a00a482dbe5560a05b5385385`

**primary / without — réponse attendue**

Quill doit expédier 21 caisses au total : 7 caisses × 3 services = 21.

[Preuve brute](answers-01/raw/q22_without.json) · SHA-256 `ac84a3b42bb63ef0b21d0a8e76459c2213a3318df42d297860df5fa37350c1b2`

## q23

Instruction actuelle : le label de routage du sac Quill pour cet envoi est BAG_AZURE. Prépare la phrase de consigne à transmettre.

**primary / with — réponse attendue**

Pour cet envoi, utilisez le label de routage BAG_AZURE pour le sac Quill.

[Preuve brute](answers-01/raw/q23_with.json) · SHA-256 `a7399f0b2b85e76d384d3516edccb32d01650f48f97a9a45d56b71920d697a73`

**primary / without — réponse attendue**

Pour cet envoi, utilisez le label de routage BAG_AZURE pour le sac Quill.

[Preuve brute](answers-01/raw/q23_without.json) · SHA-256 `986df9a4a71de8d4d19f561ade980d22d3bf4e8f6d29da16ee76dc5c65073ca8`

## q24

Nouvelle session Quill sans consigne actuelle : quel label de routage du sac a été enregistré ? Dis si cette information n’est plus disponible.

**primary / with — réponse attendue**

L’information n’est plus disponible dans les données fournies.

[Preuve brute](answers-01/raw/q24_with.json) · SHA-256 `30d56c7bef478f9cc99b793c13a71e91f607c6505be7763470e5337af22e37fe`

**primary / without — réponse attendue**

Cette information n’est plus disponible dans les données fournies.

[Preuve brute](answers-01/raw/q24_without.json) · SHA-256 `87a6acd950579108624ae6cb411da222f679a2f9e7f4c28cc7004f2cff43e7fa`
