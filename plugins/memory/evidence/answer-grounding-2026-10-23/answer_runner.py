"""Answer frozen replay questions in isolated ephemeral Codex CLI processes."""
from __future__ import annotations
import argparse
from concurrent.futures import ThreadPoolExecutor, as_completed
from datetime import datetime, timezone
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import time

from run_replay import clean, save, sha

ROOT = Path(__file__).resolve().parent


def now():
    return datetime.now(timezone.utc).isoformat()


def environment():
    return {k: v for k, v in os.environ.items()
            if not k.endswith(('_API_KEY', '_ACCESS_TOKEN'))
            and not k.startswith(('ELEGY_', 'OPENAI_', 'ANTHROPIC_', 'OLLAMA_', 'OPENCODE_'))
            and k not in {'CODEX_API_BASE_URL', 'CODEX_API_ENDPOINT', 'CODEX_THREAD_ID', 'CODEX_SESSION_ID'}}


def command(exe, cwd, protocol):
    return [str(exe), 'exec', '--ignore-user-config', '--ephemeral', '--sandbox', 'read-only',
            '--skip-git-repo-check', '--json', '-C', str(cwd), '-m', protocol['model'],
            '-c', f'model_reasoning_effort="{protocol["reasoningEffort"]}"',
            '-c', 'project_doc_max_bytes=0', '-']


def configuration(exe, protocol):
    args = command('<codex>', '<scratch-case>', protocol)
    payload = {'argv': args, 'executableSha256': sha(exe), 'environment': 'API keys/endpoints/Memory bindings removed; existing ChatGPT login only'}
    return payload, hashlib.sha256(json.dumps(payload, sort_keys=True, separators=(',', ':')).encode()).hexdigest()


def parse(stdout):
    events = [json.loads(x) for x in stdout.splitlines() if x.strip()]
    issues, messages, threads, usage = [], [], [], {}
    for event in events:
        kind = event.get('type', '')
        if kind in {'error', 'turn.failed'} or event.get('error'):
            issues.append({'type': 'model-or-provider-error', 'event': event})
        if kind == 'thread.started':
            threads.append(event['thread_id'])
        if kind.startswith('item.'):
            item = event.get('item', {})
            if item.get('type') not in {'agent_message', 'assistant_message', 'reasoning'}:
                issues.append({'type': 'unexpected-tool-or-item', 'event': event})
            if kind == 'item.completed' and item.get('type') in {'agent_message', 'assistant_message'}:
                messages.append(item['text'])
        if kind == 'turn.completed':
            usage = event.get('usage', {})
    if len(messages) != 1 or len(threads) != 1:
        issues.append({'type': 'missing-or-duplicate-final/session', 'finalCount': len(messages), 'sessionCount': len(threads)})
    return messages[0] if len(messages) == 1 else '', threads[0] if len(threads) == 1 else None, usage, issues


def invoke(exe, protocol, out, case_id, arm, question, context, config_sha):
    runtime = out / 'runtime' / f'{case_id}_{arm}'
    runtime.mkdir(parents=True)
    prompt = protocol['instruction'] + '\n\nDonnées historiques :\n' + context + '\n\nQuestion actuelle :\n' + question
    argv = command(exe, runtime.resolve(), protocol)
    started, tick = now(), time.perf_counter()
    try:
        proc = subprocess.run(argv, input=prompt, capture_output=True, encoding='utf-8', env=environment(), timeout=240)
        answer, thread, usage, issues = parse(proc.stdout)
        if proc.returncode:
            issues.append({'type': 'process-exit', 'exitCode': proc.returncode})
        packet = {'answer': answer, 'threadId': thread, 'usage': usage, 'exitCode': proc.returncode,
                  'stdout': proc.stdout, 'stderr': proc.stderr, 'issues': issues, 'valid': not issues and bool(answer)}
    except Exception as exc:
        packet = {'answer': '', 'threadId': None, 'usage': {}, 'exitCode': None,
                  'stdout': '', 'stderr': '', 'valid': False, 'issues': [{'type': type(exc).__name__, 'message': str(exc)}]}
    public_argv = command('<codex>', '<scratch-case>', protocol)
    packet.update({'schemaVersion': 'memory-realistic-answer/v1', 'caseId': case_id, 'arm': arm,
                   'startedAt': started, 'completedAt': now(), 'elapsedMs': round((time.perf_counter() - tick) * 1000, 3),
                   'promptSha256': hashlib.sha256(prompt.encode()).hexdigest(), 'configurationSha256': config_sha,
                   'command': public_argv})
    (out / 'raw').mkdir(exist_ok=True)
    save(out / 'raw' / f'{case_id}_{arm}.json', clean(packet))
    print(json.dumps({'caseId': case_id, 'arm': arm, 'valid': packet['valid'], 'elapsedMs': packet['elapsedMs']}), flush=True)
    return {key: packet[key] for key in ('caseId', 'arm', 'valid', 'elapsedMs', 'issues')}


def main():
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument('action', choices=['preflight', 'run'])
    ap.add_argument('--replay', type=Path)
    ap.add_argument('--output', type=Path, required=True)
    args = ap.parse_args()
    out = args.output.resolve()
    assert not out.exists(), 'Use a new directory; preserve prior attempts'
    out.mkdir()
    protocol = json.loads((ROOT / 'protocol.json').read_text(encoding='utf-8'))
    exe = Path(shutil.which('codex') or '')
    assert exe.is_file(), 'Installed Codex CLI unavailable'
    login = subprocess.run([str(exe), 'login', 'status'], capture_output=True, encoding='utf-8', env=environment(), timeout=30)
    assert login.returncode == 0 and 'chatgpt' in (login.stdout + login.stderr).lower(), 'Existing ChatGPT authentication required; never use API-key fallback'
    config, config_sha = configuration(exe, protocol)
    report = {'schemaVersion': 'memory-realistic-answers/v1', 'startedAt': now(), 'status': 'running',
              'model': protocol['model'], 'reasoningEffort': protocol['reasoningEffort'],
              'configuration': config, 'configurationSha256': config_sha, 'codexSha256': sha(exe),
              'codexVersion': subprocess.run([str(exe), '--version'], capture_output=True, encoding='utf-8', timeout=30).stdout.strip(),
              'corpusSha256': sha(ROOT / 'corpus.json'), 'protocolSha256': sha(ROOT / 'protocol.json'),
              'runnerSha256': sha(__file__), 'ordinaryChatGPTAuth': True, 'paidApiCalls': 0, 'answers': []}
    if args.action == 'preflight':
        packet = invoke(exe, protocol, out, 'preflight', 'without', 'Pour tester uniquement la disponibilité du client, réponds exactement READY.', '', config_sha)
        raw = json.loads((out / 'raw/preflight_without.json').read_text(encoding='utf-8'))
        report['answers'].append(packet)
        report['status'] = 'completed' if raw['valid'] and raw['answer'].strip() == 'READY' else 'failed'
    else:
        assert args.replay and args.replay.exists()
        replay = args.replay.resolve()
        base = json.loads((replay / 'run.json').read_text(encoding='utf-8'))
        assert base['status'] == 'completed', 'Do not mask an incomplete replay as a model failure'
        assert report['corpusSha256'] == base['corpusSha256'] and report['protocolSha256'] == base['protocolSha256']
        report['replayRunSha256'] = sha(replay / 'run.json')
        cases = []
        corpus = json.loads((ROOT / 'corpus.json').read_text(encoding='utf-8'))
        for episode in corpus['episodes']:
            data = json.loads((replay / episode['id'] / 'run.json').read_text(encoding='utf-8'))
            packets = {s['caseId']: s['packet'] for s in data['stages']}
            for case in episode['cases']:
                cases.append((case, packets[case['id']]['response']['data']['additionalContext']))
        def pair(item):
            case, context = item
            arms = ('without', 'with') if int(case['id'][1:]) % 2 else ('with', 'without')
            return [invoke(exe, protocol, out, case['id'], arm, case['question'], context if arm == 'with' else '', config_sha) for arm in arms]
        with ThreadPoolExecutor(max_workers=3) as pool:
            for future in as_completed([pool.submit(pair, item) for item in cases]):
                report['answers'].extend(future.result())
                save(out / 'run.json', report)
        report['answers'].sort(key=lambda x: (x['caseId'], x['arm']))
        report['status'] = 'completed' if len(report['answers']) == 48 and all(p['valid'] for p in report['answers']) else 'inconclusive'
    report['completedAt'] = now()
    save(out / 'run.json', report)
    return int(report['status'] != 'completed')


if __name__ == '__main__':
    raise SystemExit(main())
