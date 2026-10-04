"""Reproduce deterministic per-answer scoring without calling any model."""
import argparse
import hashlib
import json
from pathlib import Path


def normalize(value):
    value = value.strip().upper()
    return value[:-1] if value.endswith('.') else value


def assess(root):
    corpus = json.loads((root / 'corpus.json').read_text(encoding='utf-8'))
    answer_doc = json.loads((root / 'answers.json').read_text(encoding='utf-8'))
    answers = answer_doc['answers']
    packets = {p['caseId']: p for p in json.loads((root / answer_doc['recallPackets']).read_text(encoding='utf-8'))}
    expected_keys = {f"{c['id']}_{arm}" for c in corpus['cases'] for arm in ('with', 'without')}
    if set(answers) != expected_keys:
        raise ValueError('Answer inventory differs from the frozen paired corpus')
    rows = []
    for case in corpus['cases']:
        row = {'id': case['id'], 'question': case['question'], 'expected': case['expected']}
        for arm in ('with', 'without'):
            observed = answers[f"{case['id']}_{arm}"]
            row[arm] = observed['answer']
            row[f'score{arm.title()}'] = int(normalize(observed['answer']) == normalize(case['expected']))
            row[f'forbidden{arm.title()}'] = [t for t in case.get('forbidden', []) if t.upper() in observed['answer'].upper()]
            row[f'observerUpperBound{arm.title()}Ms'] = observed['observedCompletedAt'] - observed['startedAt']
        packet = packets[case['id']]
        row['retrievalLatencyMs'] = packet['elapsedMs']
        row['retrievedExpectedId'] = packet['checks']['expectedMemoryIdPresent'] if case.get('expectedMemoryKey') else None
        row['retrievalStatus'] = packet['response']['data']['status']
        row['recallText'] = [m['text'] for m in json.loads(packet['response']['data']['additionalContext'].split('\n', 1)[1])['records']] if packet['response']['data']['additionalContext'] else []
        row['errors'] = [f'{arm}: expected {case["expected"]}, observed {row[arm]}' for arm in ('with', 'without') if not row[f'score{arm.title()}']]
        rows.append(row)
    counts = {arm: sum(r[f'score{arm.title()}'] for r in rows) for arm in ('with', 'without')}
    return {
        'schemaVersion': 'memory-answer-assessment/v1',
        'inputs': {name: hashlib.sha256((root / name).read_bytes()).hexdigest() for name in ('corpus.json', 'protocol.json', 'answers.json', answer_doc['recallPackets'])},
        'sampleSize': len(rows), 'correctWith': counts['with'], 'correctWithout': counts['without'],
        'meanPairedDifference': (counts['with'] - counts['without']) / len(rows),
        'forbiddenAnswerCount': sum(bool(r[f'forbidden{arm.title()}']) for r in rows for arm in ('with', 'without')),
        'timingLimit': answer_doc['timingLimit'],
        'formalQualification': 'inconclusive: exact per-answer execution latency was not captured; observer intervals include unrelated orchestration gaps',
        'limits': 'One synthetic realization per case and requested model alias; no population-wide gain, statistical significance, cost saving, native hook automation or immutable backend-version claim.',
        'cases': rows,
    }


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--directory', type=Path, default=Path(__file__).parent)
    parser.add_argument('--output', type=Path)
    args = parser.parse_args()
    result = assess(args.directory)
    encoded = json.dumps(result, indent=2, ensure_ascii=False) + '\n'
    if args.output:
        args.output.write_text(encoded, encoding='utf-8')
    else:
        print(encoded)
