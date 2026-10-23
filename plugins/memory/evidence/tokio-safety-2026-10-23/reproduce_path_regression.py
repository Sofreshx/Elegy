"""Run only the three path regressions against the unchanged Q7 wrappers.

Exit 1 is the expected RED result. No model, Memory or network call is made.
The JSON output neutralizes local traceback prefixes before publication.
"""
from pathlib import Path
import argparse
import hashlib
import importlib.util
import io
import json
import re
import sys
import unittest

ROOT = Path(__file__).resolve().parent
REPO = ROOT.parents[3]


def load(name, path):
    spec = importlib.util.spec_from_file_location(name, path)
    module = importlib.util.module_from_spec(spec)
    sys.modules[name] = module
    spec.loader.exec_module(module)
    return module


def public_text(value):
    for prefix, replacement in ((str(REPO), '<repo>'), (str(Path.home()), '<home>')):
        pattern = re.escape(prefix).replace(r'\\', r'[\\/]')
        value = re.sub(pattern, lambda _: replacement, value, flags=re.IGNORECASE)
    return value


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    if args.output.exists():
        raise ValueError('preserve the previous observation; select a new output')
    originals = {name: ROOT.parent / 'ci-integration-2026-10-23' / (name + '.py')
                 for name in ('campaign', 'run_native')}
    for name, path in originals.items():
        load(name, path)
    tests = load('red_test_campaign', ROOT / 'test_campaign.py')
    names = ('test_public_normalizes_windows_paths_recursively_with_boundaries',
             'test_run_native_public_normalizes_windows_paths_with_boundaries',
             'test_public_functions_preserve_posix_case_and_boundaries')
    stream = io.StringIO()
    result = unittest.TextTestRunner(stream=stream, verbosity=2).run(
        unittest.TestSuite(tests.CampaignTests(name) for name in names))
    raw = stream.getvalue()
    private_raw = ROOT / 'runtime' / 'path-normalizer' / (args.output.stem + '.raw.log')
    if private_raw.exists():
        raise ValueError('raw observation already exists')
    private_raw.parent.mkdir(parents=True, exist_ok=True)
    private_raw.write_text(raw, encoding='utf-8')
    record = {
        'schemaVersion': 'memory-public-path-regression/v1',
        'argv': ['<python>', str(Path(__file__).relative_to(REPO)).replace('\\', '/'),
                 '--output', public_text(str(args.output))],
        'originalSourceSha256': {name: hashlib.sha256(path.read_bytes()).hexdigest()
                                 for name, path in originals.items()},
        'testSourceSha256': hashlib.sha256((ROOT / 'test_campaign.py').read_bytes()).hexdigest(),
        'testsRun': result.testsRun, 'failures': len(result.failures), 'errors': len(result.errors),
        'exitCode': 0 if result.wasSuccessful() else 1,
        'rawOutputSha256': hashlib.sha256(raw.encode()).hexdigest(),
        'publishedOutput': public_text(raw),
        'transformation': 'Only repo and home traceback prefixes replaced; raw output stays local.'}
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(record, indent=2) + '\n', encoding='utf-8')
    print(json.dumps({key: record[key] for key in ('testsRun', 'failures', 'errors', 'exitCode')}))
    raise SystemExit(record['exitCode'])
