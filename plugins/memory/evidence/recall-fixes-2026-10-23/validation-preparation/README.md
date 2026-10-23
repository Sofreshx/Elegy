# Préparation de la validation

Ces premiers lancements ne sont pas des preuves de passage. Le wrapper a perdu le suivi des enfants Cargo ; ses interruptions ne prouvaient pas leur arrêt. Les compilations ont été laissées finir. Un seul binaire de test, confirmé inactif pendant plus de 600 secondes avec attente loopback, a été arrêté après vérification de son chemin. Les tests Python ont aussi rencontré des refus d’accès dans le TEMP du bac restreint.

La reprise native séquentielle utilise uniquement un TEMP enfant isolé et les permissions de processus déjà autorisées. Les deux tests de timeout puis les suites complètes sont exécutés sans exclusion ni assouplissement. Les résultats conclusifs figurent dans ../native-validation/run.json. Les fichiers ci-présents sont conservés tels quels et ne sont pas requalifiés comme des résultats du nouveau pilote.
