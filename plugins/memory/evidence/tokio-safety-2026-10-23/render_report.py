"""Render individual frozen answers and errors; never change scores or verdicts."""
from pathlib import Path
import json

ROOT = Path(__file__).resolve().parent


def load(name):
    return json.loads((ROOT / name).read_text(encoding='utf-8'))


def write_new(name, text):
    path = ROOT / name
    if path.exists():
        raise ValueError('preserve the previous report')
    path.write_text(text, encoding='utf-8')


if __name__ == '__main__':
    assessment = load('assessment.json')
    if assessment['status'] != 'qualified-selected-configuration':
        raise ValueError('report requires accepted assessment')
    inputs = {r['id']: r for r in load('review-inputs.json')['records']}
    mapped = {(r['phase'], r['caseId'], r['arm'], r['trial']): r['id']
              for r in load('review-map.json')['records']}
    lines = ['---', 'title: Les 84 réponses individuelles Q8', 'status: completed',
             'owner: Elegy Memory', '---', '', '# Réponses individuelles', '',
             'Chaque texte reste inchangé. La complétude suit la cible du protocole : '
             'un contrôle d’information inconnue est satisfait par une abstention correcte. '
             'Cela ne rend pas une restauration ou une approbation non documentée possible.', '',
             'Les scores lexicaux originaux sont séparés des verdicts de prose. '
             'Les données exactes, empreintes et groupes d’alias restent dans les JSON liés.', '']
    false_negatives = []
    incomplete = []
    rows = sorted(assessment['records'], key=lambda r: (r['phase'], r['caseId'], r['trial'], r['arm']))
    for record in rows:
        key = (record['phase'], record['caseId'], record['arm'], record['trial'])
        rid = mapped[key]
        item, semantic = inputs[rid], record['semantic']
        label = f"{record['caseId']} · {record['arm']} · essai {record['trial']}"
        raw = f"{record['phase']}/{record['rawPath']}"
        lines += [f'## {label}', '', f"Question : {item['question']}", '',
                  'Contexte réellement fourni :', '', '```text', item['context'] or '(vide)',
                  '```', '', 'Réponse exacte :', '', '```text', item['answer'], '```', '',
                  f"Score lexical : {record['score']:.10f}. Complétude de la cible : "
                  f"{'oui' if semantic['taskComplete'] else 'non, données historiques absentes'}. "
                  f"Étaiement/contrôle/format : {semantic['grounded']}/{semantic['controlPass']}/{semantic['formatPass']}.", '',
                  f"Revue : {semantic['reason']}", '', f'[Sortie brute]({raw}) · identifiant de revue `{rid}`.', '']
        if semantic['taskComplete'] and record['score'] < 1:
            false_negatives.append((record, label, item['answer']))
        if not semantic['taskComplete']:
            incomplete.append(label)
    write_new('results.md', '\n'.join(lines) + '\n')
    errors = ['---', 'title: Erreurs individuelles Q8', 'status: completed',
              'owner: Elegy Memory', '---', '', '# Erreurs et limites observées', '',
              f"Aucun échec sémantique final parmi les 47 réponses avec rappel. "
              f"Les {len(incomplete)} réponses sans rappel incomplètes s’abstiennent sur des faits indisponibles ; "
              'leurs textes et groupes manquants restent dans [results.md](results.md) et [assessment.json](assessment.json).', '',
              '## Faux négatifs lexicaux conservés', '',
              'Les scores ne sont jamais corrigés après lecture. Les graphies horaires équivalentes '
              'et les formulations d’abstention ci-dessous passent la revue de prose mais manquent un alias figé.', '']
    for record, label, answer in false_negatives:
        errors += [f"- **{label}**, score `{record['score']:.10f}` : « {answer} »"]
    errors += ['', '## Erreurs de classement des revues', '',
               'Dix verdicts initiaux confondent la complétude d’un contrôle d’abstention avec '
               'la disponibilité du fait opérationnel. Les deux rapports originaux restent inchangés. '
               'Les addenda appliquent les types de fixture et les règles déjà figés ; '
               'aucune réponse, aucun score ni critère ne change.', '']
    for letter in ('a', 'b'):
        for item in load(f'review-batch-{letter}-addendum.json')['records']:
            errors += [f"- `{item['caseId']}/{item['arm']}/essai{item['trial']}` "
                       f"(`{item['id']}`), `taskComplete false → true` : {item['reason']}"]
    errors += ['', '[Rapport A](review-batch-a.json) · [addendum A](review-batch-a-addendum.json) · '
               '[rapport B](review-batch-b.json) · [addendum B](review-batch-b-addendum.json) · '
               '[lecture des 84 textes par le parent](parent-review.json).', '',
               '## Préparation et environnement', '',
               'Les [préparations initiales](preparation.json) gardent l’erreur cp1252 du printer après '
               'un cargo-tree réussi, le résumé d’audit qui ne gérait pas une valeur nullable, '
               'le premier test RED invalide, son origine intermédiaire Q6 et le diagnostic '
               'préflight lancé avant la création du sujet. Le RED public final porte sur les Q7 exacts.', '',
               '- Le premier [MCP](native-validation/mcp-final.json) échoue à la compilation sur des '
               'allocations mémoire et des erreurs de pile. Aucun test n’est alors compté comme passé. '
               'Le même paquet passe ensuite 67 tests avec un argument temporaire `-j 2`, hors de '
               'l’inspection Git ; [les échantillons mémoire](native-validation/mcp-retry-resources.json) '
               'sont conservés. Cela établit la reprise, pas la cause unique de la première panne.',
               '- Le premier commit de gel est refusé par un hook avec `inspection failed`. '
               'Le même index/message passe ensuite les deux hooks en phase séparée. '
               'Aucun contrôle ou réglage permanent n’a été changé ; la cause précise du refus générique reste inconnue.',
               '- Les deux évaluateurs ont interprété le raccourci Q8 comme un sous-dossier. '
               'Les rapports sont [replacés octet pour octet](review-relocation.json) avant consolidation.',
               '- Le [log public MCP échoué](native-validation/log-publication.json) remplace seulement '
               'les préfixes du dossier personnel. Flux et métadonnées originaux sont conservés localement '
               'sous runtime, avant tout reçu. Les empreintes originales et publiques sont distinctes.', '',
               'L’audit frais rapporte zéro vulnérabilité ; les avertissements unsafe-libyaml abandonné '
               'et versions retirées chacha20/clipboard-win restent visibles. cargo-deny est absent '
               'localement et doit passer en CI ; aucune installation ni exemption n’est ajoutée.', '',
               'Les échecs Luna/Q3/Q4 et les campagnes Q6/Q7 gardent leurs sources et résultats historiques. '
               'Aucune réussite d’un autre logiciel hôte, de tous les réglages Desktop ou de l’usage quotidien n’est inférée.', '']
    errors.append('\nLa détection générique de répertoire personnel a aussi classé deux chemins fictifs comme locaux. Une régression sur les scripts de structure montre le RED puis le GREEN corrigé, en maintenant quatre rejets CUsers/Linux/URI et le signalement des lectures impossibles. Le premier JSON RED indiquait par erreur le blob du test avant ajout de la régression ; il reste conservé. Les captures red-02/green-02 lient les sources réellement exécutées et les empreintes des flux bruts locaux. Ce correctif de validation est extérieur aux 149/18 sources Memory ; les onze scripts et les 84 réponses restent inchangés.\n')
    write_new('errors.md', '\n'.join(errors) + '\n')
    print(json.dumps({'answers': len(rows), 'lexicalFalseNegatives': len(false_negatives),
                      'withoutRecallIncomplete': len(incomplete)}))
