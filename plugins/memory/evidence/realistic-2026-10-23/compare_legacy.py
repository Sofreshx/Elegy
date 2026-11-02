"""Post-hoc diagnostic: replay only Harbor against the archived PR47 binaries.

No additional answer/model call. This cannot establish full version parity;
it checks whether the two observed retrieval failures also occur on that artifact.
"""
import argparse
import json
from pathlib import Path
from run_replay import ROOT, episode, save, sha


def main():
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument('--cli', type=Path, required=True)
    ap.add_argument('--mcp', type=Path, required=True)
    ap.add_argument('--output', type=Path, required=True)
    args = ap.parse_args()
    assert sha(args.cli) == '92692d62c558feba70a4b8df17f1575d1fbb42aa7db31be3abf855ee255af960'
    assert sha(args.mcp) == '901361065cedcd54091f828a96f77768283e596ee980f18372eac86d13246eaf'
    assert not args.output.exists()
    args.output.mkdir()
    corpus = json.loads((ROOT / 'corpus.json').read_text(encoding='utf-8'))
    protocol = json.loads((ROOT / 'protocol.json').read_text(encoding='utf-8'))
    harbor = next(item for item in corpus['episodes'] if item['id'] == 'harbor')
    result = episode(protocol, args.cli.resolve(), args.mcp.resolve(), harbor, args.output.resolve())
    save(args.output / 'run.json', {'schemaVersion': 'memory-legacy-diagnostic/v1',
        'sourceRevision': 'acc6b4e22625841ce0fdde55226472cd416728f1',
        'cliSha256': sha(args.cli), 'mcpSha256': sha(args.mcp), 'runnerSha256': sha(ROOT / 'run_replay.py'),
        'diagnosticSha256': sha(__file__), 'corpusSha256': sha(ROOT / 'corpus.json'),
        'status': result['status'], 'modelCalls': 0,
        'scope': 'Only Harbor; exploratory diagnostic after primary retrieval, not a preregistered full-version comparison.',
        'retrieval': [{'caseId': s['caseId'], 'texts': [r['content'] for r in s['packet']['response']['data']['recalls']]}
                      for s in result['stages']]})
    print(json.dumps({'status': result['status'], 'retrieval': json.loads((args.output / 'run.json').read_text())['retrieval']}))
    return int(result['status'] != 'completed')


if __name__ == '__main__':
    raise SystemExit(main())
