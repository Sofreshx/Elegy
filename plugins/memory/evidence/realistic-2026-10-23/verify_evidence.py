"""Offline evidence verification; no model call, memory access, or configuration write."""
import argparse
from datetime import datetime, timezone
import hashlib
import json
from pathlib import Path
import py_compile
import re
import subprocess
import sys

ROOT = Path(__file__).resolve().parent
REPO = ROOT.parents[3]


def load(path):
    return json.loads(path.read_text(encoding='utf-8-sig'))


def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def verify(cli=None, mcp=None):
    scratch = ROOT / 'runtime'
    scratch.mkdir(exist_ok=True)
    results = {'schemaVersion': 'memory-realistic-validation/v1',
               'verifiedAt': datetime.now(timezone.utc).isoformat(), 'checks': []}

    def check(name, passed, detail):
        results['checks'].append({'name': name, 'passed': bool(passed), 'detail': detail})

    def run(args, log):
        p = subprocess.run([sys.executable, *args], cwd=REPO, capture_output=True,
                           encoding='utf-8', errors='replace', timeout=120)
        (scratch / log).write_text(p.stdout + p.stderr, encoding='utf-8')
        return p

    p = run(['-m', 'unittest', 'discover', '-s', str(ROOT), '-p', 'test_*.py', '-v'], 'unit-tests.log')
    results['unitTestOutput'] = p.stdout + p.stderr
    check('focused-regression-tests', p.returncode == 0 and 'Ran 14 tests' in p.stderr,
          {'exitCode': p.returncode, 'expectedTestCount': 14, 'log': 'runtime/unit-tests.log'})
    scripts = sorted(ROOT.glob('*.py'))
    for path in scripts:
        py_compile.compile(str(path), doraise=True)
    check('python-compile', True, {'fileCount': len(scripts)})

    recheck = scratch / 'assessment-recheck.json'
    p = run([str(ROOT / 'grade.py'), '--answers', str(ROOT / 'answers-01'),
             '--replay', str(ROOT / 'replay-03'), '--output', str(recheck)], 'grade-recheck.log')
    check('deterministic-assessment-reproduction', p.returncode == 0 and load(recheck) == load(ROOT / 'assessment.json'),
          {'exitCode': p.returncode, 'originalSha256': sha(ROOT / 'assessment.json')})
    p = run([str(ROOT / 'check_results.py'), '--output', 'runtime/verdicts-recheck.json'], 'gate-recheck.log')
    gate = load(scratch / 'verdicts-recheck.json')
    critical = [x['caseId'] for x in gate['criticalAnswerErrors']]
    check('adverse-results-gate', p.returncode == 1 and gate == load(ROOT / 'verdicts.json') and critical == ['q09', 'q17', 'q18'],
          {'expectedExitCode': 1, 'actualExitCode': p.returncode, 'status': gate['status'], 'criticalCases': critical})

    subject = load(ROOT / 'subject.json')
    changed = [x['path'] for x in subject['sourceFiles'] if sha(REPO / x['path']) != x['sha256']]
    check('source-bytes-unchanged', not changed, {'fileCount': len(subject['sourceFiles']), 'differentPaths': changed})
    for name, path in [('cli', cli), ('mcp', mcp)]:
        if path:
            actual = sha(Path(path).resolve())
            check(name + '-binary-binding', actual == subject[name + 'Sha256'], {'sha256': actual})
        else:
            results.setdefault('skipped', []).append(name + '-binary-binding: executable path not supplied')

    receipts = load(ROOT / 'ledger.json')
    bound = []
    for item in receipts:
        wrapper = load(ROOT / 'receipts' / (item['id'] + '.json'))
        check('receipt-ledger-index', wrapper['sha256'] == item['sha256'], {'receiptId': item['id']})
        receipt = wrapper['receipt']
        check('receipt-adverse-outcome', receipt['outcome'] == 'inconclusive', {'outcome': receipt['outcome']})
        for evidence in receipt['observation']['evidence']:
            bound.append(evidence['path'])
            check('receipt-file:' + evidence['path'], sha(ROOT / evidence['path']) == evidence['sha256'], 'SHA-256 verified')

    status = load(ROOT / 'final-status.json')['data']
    quality = next(x for x in status['claims'] if x['claim']['id'] == 'recall.answer-quality')
    check('cli-receipt-readback', status['buildMatchesSource'] and quality['status'] == 'reported-inconclusive'
          and quality['latestReceipt'] == receipts[-1]['id'],
          {'buildMatchesSource': status['buildMatchesSource'], 'answerQualityStatus': quality['status']})

    links = []
    broken = []
    for name in ['README.md', 'answers.md']:
        for target in re.findall(r'\]\(([^)]+)\)', (ROOT / name).read_text(encoding='utf-8')):
            target = target.split('#')[0]
            if not target or '://' in target:
                continue
            links.append(target)
            if not (ROOT / target).exists():
                broken.append({'document': name, 'target': target})
    check('local-document-links', not broken, {'linkCount': len(links), 'broken': broken})
    diff_codes = [subprocess.run(['git', 'diff', *flags, '--check'], cwd=REPO,
                                capture_output=True, timeout=30).returncode
                  for flags in [[], ['--cached']]]
    check('git-diff-check', diff_codes == [0, 0], {'workingTreeExitCode': diff_codes[0], 'stagedExitCode': diff_codes[1]})
    results['knownNonPassingChecks'] = [{'name': 'repository-docs-check', 'exitCode': 1,
        'evidence': 'docs-validation.json', 'detail': 'DOCS-CHECK-015 bundle drift; DOCS-CHECK-009 freshness warning. Canonical docs/config unchanged.'}]
    results['status'] = 'passed-evidence-checks' if all(x['passed'] for x in results['checks']) else 'failed'
    return results


if __name__ == '__main__':
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument('--cli')
    ap.add_argument('--mcp')
    ap.add_argument('--output', type=Path, default=ROOT / 'runtime/validation-recheck.json')
    args = ap.parse_args()
    result = verify(args.cli, args.mcp)
    args.output.write_text(json.dumps(result, ensure_ascii=False, indent=2) + '\n', encoding='utf-8')
    print(json.dumps({'status': result['status'], 'failed': [x['name'] for x in result['checks'] if not x['passed']]}))
    raise SystemExit(0 if result['status'] == 'passed-evidence-checks' else 1)
