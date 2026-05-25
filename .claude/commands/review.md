Review les changements sur la branche actuelle vs `dev` :

1. `git diff dev --stat` puis `git diff dev` pour voir les changements
2. Pour chaque fichier modifié, vérifie la cohérence avec les docs d'architecture
3. Cherche : invariants structurels touchés, unwrap en production, edge cases non testés, fichiers modifiés hors scope
4. Liste les findings par sévérité (critique / important / mineur)

Focus : $ARGUMENTS
