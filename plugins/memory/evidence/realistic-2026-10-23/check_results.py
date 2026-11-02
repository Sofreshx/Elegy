"""Finite campaign acceptance gate: summarize measured anomalies, never monitor users."""
import argparse
import json
from pathlib import Path
from run_replay import ROOT, save, sha


def evaluate(assessment):
    errors = assessment['inventoryErrors'] + [x for r in assessment['records'] for x in r['executionErrors']]
    critical = [{'caseId': r['caseId'], 'forbiddenMentions': r['forbiddenMentions'],
                 'actual': r['actual'], 'rawPath': r['rawPath']}
                for r in assessment['records'] if r['arm'] == 'with' and r['forbiddenMentions']]
    review = [{'caseId': r['caseId'], 'missingGroups': r['missingGroups'], 'formatErrors': r['formatErrors']}
              for r in assessment['records'] if r['arm'] == 'with' and r['score'] != 1]
    return {'schemaVersion': 'memory-realistic-acceptance/v1',
            'status': 'inconclusive' if errors else 'blocked' if critical else 'review-required' if review else 'passed-on-this-sample',
            'executionErrors': errors, 'criticalAnswerErrors': critical, 'answersToReview': review,
            'limits': 'Rule-bound one-shot test gate, not a continuous monitor or general semantic/hallucination detector. Frozen lexical metrics retained separately.'}


if __name__ == '__main__':
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument('--assessment', default='assessment.json')
    ap.add_argument('--output', default='verdicts.json')
    args = ap.parse_args()
    path = ROOT / args.assessment
    a = json.loads(path.read_text(encoding='utf-8'))
    verdict = evaluate(a)
    verdict['assessmentSha256'] = sha(path)
    save(ROOT / args.output, verdict)
    print(json.dumps({'status': verdict['status'], 'criticalCases': [x['caseId'] for x in verdict['criticalAnswerErrors']]}))
    raise SystemExit(0 if verdict['status'] == 'passed-on-this-sample' else 1)
