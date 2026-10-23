"""Replay frozen synthetic sessions through actual MCP and contextual-recall CLI."""
from __future__ import annotations
import argparse
from datetime import datetime, timezone
import hashlib
import json
import os
from pathlib import Path
import subprocess
import sys
import time

from rpc_support import RpcClient, child_env, source_snapshot

ROOT = Path(__file__).resolve().parent


def sha(path):
    return hashlib.sha256(Path(path).read_bytes()).hexdigest()


def now():
    return datetime.now(timezone.utc).isoformat()


def save(path, value):
    Path(path).write_text(json.dumps(value, indent=2, ensure_ascii=False) + '\n', encoding='utf-8', newline='\n')


def clean(value):
    if isinstance(value, str):
        for prefix in (str(ROOT), ROOT.as_posix(), str(Path.home()), Path.home().as_posix()):
            variants = {prefix}
            for _ in range(4):
                variants |= {v.replace('\\', '\\\\') for v in variants}
            for v in sorted(variants, key=len, reverse=True):
                value = value.replace(v, '<local>')
        return value
    if isinstance(value, dict):
        return {k: clean(v) for k, v in value.items()}
    if isinstance(value, list):
        return [clean(v) for v in value]
    return value


def invariant(before, after):
    assert before['main']['present'] and after['main']['present']
    return before['durableData'] == after['durableData']


def validate_protocol(protocol):
    assert 0 < protocol['memory']['maxContextTokens'] <= 600, 'Recall contract maximum is600'
    assert protocol['memory']['scopes'] == ['Agent']


def require(result, namespace):
    assert isinstance(result, dict) and result.get('namespace') == namespace, 'Wrong MCP namespace'
    return result


def recall_check(client, identifier, text, namespace):
    result = require(client.tool('memory_recall', {'id': identifier}), namespace)
    assert result['found'] is True and result['memory']['id'] == identifier and result['memory']['content'] == text
    return result


def episode(subject, cli, mcp, episode, out, baseline_cli=None):
    folder = out / episode['id']
    folder.mkdir()
    runtime = folder / 'runtime'
    runtime.mkdir()
    db = runtime / 'source.sqlite'
    mappings, calls, clients = {}, [], []
    report = {'schemaVersion': 'memory-replay-episode/v1', 'id': episode['id'], 'startedAt': now(),
              'status': 'running', 'stages': [], 'rpc': calls, 'checks': [], 'cleanup': []}

    def connect(agent, label):
        c = RpcClient(mcp, child_env(db, agent), calls, label)
        clients.append(c)
        c.start()
        return c

    def close_all():
        while clients:
            c = clients.pop()
            process = c.close()
            assert process['exitCode'] == 0 and process['graceful'] and not process['forced'], 'MCP did not close normally'

    try:
        for case in episode['cases']:
            stage = {'caseId': case['id'], 'actions': [], 'checks': [], 'sessionId': f'{episode["id"]}-{case["id"]}-fresh', 'turnId': case['id']}
            for action in case['actions']:
                agent = action['agent']
                writer = connect(agent, f'{case["id"]}-write-{action["key"]}')
                if action['op'] == 'store':
                    result = require(writer.tool('memory_store', {'content': action['text'], 'importance': 0.9, 'provenance': 'user-stated'}), agent)
                    assert result['action'] == 'added' and result['gateResult'] == 'accepted', 'Store gate did not add independent fact'
                    assert result['memory']['content'] == action['text']
                    mappings[action['key']] = {'id': result['memory']['id'], 'text': action['text'], 'agent': agent, 'deleted': False}
                elif action['op'] == 'correct':
                    memory = mappings[action['key']]
                    result = require(writer.tool('memory_correct', {'id': memory['id'], 'content': action['text'], 'reason': 'synthetic user correction at next session'}), agent)
                    assert result['memory']['id'] == memory['id'] and result['memory']['content'] == action['text']
                    memory['text'] = action['text']
                else:
                    assert action['op'] == 'delete'
                    memory = mappings[action['key']]
                    result = require(writer.tool('memory_delete', {'id': memory['id']}), agent)
                    assert result['deleted'] is True and result['id'] == memory['id']
                    memory['deleted'] = True
                stage['actions'].append({'key': action['key'], 'op': action['op'], 'result': result})
                close_all()

            # This client starts after all writer processes exited. It receives
            # only the DB binding, never a writer's model conversation.
            reader = connect(case['agent'], f'{case["id"]}-fresh-mcp')
            active = [m for m in mappings.values() if m['agent'] == case['agent'] and not m['deleted']]
            if active:
                own = active[0]
                recall_check(reader, own['id'], own['text'], case['agent'])
                result = require(reader.tool('memory_search', {'query': own['text'], 'limit': 100}), case['agent'])
                assert any(row['id'] == own['id'] for row in result['results']), 'Own search positive control missing'
                stage['checks'].append({'name': 'fresh-mcp-id-and-search', 'passed': True})
            for memory in mappings.values():
                if memory['deleted'] and memory['agent'] == case['agent']:
                    result = require(reader.tool('memory_recall', {'id': memory['id']}), case['agent'])
                    assert result['found'] is False
                    stage['checks'].append({'name': 'deleted-memory-absent', 'passed': True})
                elif memory['agent'] != case['agent'] and not memory['deleted']:
                    result = require(reader.tool('memory_recall', {'id': memory['id']}), case['agent'])
                    assert result['found'] is False
                    found = require(reader.tool('memory_search', {'query': memory['text'], 'limit': 100}), case['agent'])
                    assert memory['id'] not in [x['id'] for x in found['results']]
                    listed = require(reader.tool('memory_list', {'limit': 1000, 'includeDormant': True}), case['agent'])
                    assert memory['id'] not in [x['id'] for x in listed['memories']]
                    stage['checks'].append({'name': 'foreign-memory-id-search-list-absent', 'passed': True, 'id': memory['id']})
            close_all()

            config = {'version': 1, 'projectRoot': str(runtime.resolve()), 'dbPath': str(db.resolve()),
                'statePath': str((runtime / f'journal-{case["agent"]}.json').resolve()), 'domain': 'realistic-qualification',
                'scopes': ['Agent'], 'mode': 'inject', 'agentId': case['agent'], 'maxSensitivity': 'Low',
                'maxContextTokens': subject['memory']['maxContextTokens'], 'minSimilarity': subject['memory']['minSimilarity'],
                'learningEnabled': False, 'includeRecentContext': False}
            cfg = runtime / f'config-{case["id"]}.json'
            save(cfg, config)
            request = {'cwd': str(runtime.resolve()), 'sessionId': stage['sessionId'], 'turnId': stage['turnId'], 'prompt': case['question'], 'recentContext': []}
            before = source_snapshot(db)
            start = time.perf_counter()
            proc = subprocess.run([str(cli), '--json', '--non-interactive', 'contextual-recall', '--config', str(cfg.resolve())],
                input=json.dumps(request), encoding='utf-8', capture_output=True, timeout=90, env=child_env(db.resolve(), case['agent']))
            elapsed = round((time.perf_counter() - start) * 1000, 3)
            after = source_snapshot(db)
            response = json.loads(proc.stdout)
            stage['cliDiagnostic'] = {'exitCode': proc.returncode, 'response': response, 'stderr': proc.stderr}
            assert proc.returncode == 0 and response['status'] == 'ok', 'Recall CLI failed'
            assert invariant(before, after), 'Recall changed source durable bytes'
            data = response['data']
            own_ids = {m['id'] for m in mappings.values() if m['agent'] == case['agent'] and not m['deleted']}
            assert all(m['id'] in own_ids and m['scope'] == 'Agent' for m in data['recalls']), 'Foreign or deleted record in context'
            stage['packet'] = {'caseId': case['id'], 'agent': case['agent'], 'request': {**request, 'cwd': '<runtime>'},
                'response': response, 'elapsedMs': elapsed, 'exitCode': proc.returncode, 'stderr': proc.stderr,
                'sourceBefore': before, 'sourceAfter': after, 'sourceUnchanged': True,
                'configuration': {**config, 'projectRoot': '<runtime>', 'dbPath': '<source.sqlite>', 'statePath': f'<journal-{case["agent"]}>'}}
            if baseline_cli is not None:
                baseline_config = {**config, 'statePath': str((runtime / f'baseline-journal-{case["agent"]}.sqlite').resolve())}
                baseline_cfg = runtime / f'baseline-config-{case["id"]}.json'
                save(baseline_cfg, baseline_config)
                baseline_before = source_snapshot(db)
                baseline_proc = subprocess.run([str(baseline_cli), '--json', '--non-interactive', 'contextual-recall', '--config', str(baseline_cfg.resolve())],
                    input=json.dumps(request), encoding='utf-8', capture_output=True, timeout=90, env=child_env(db.resolve(), case['agent']))
                baseline_after = source_snapshot(db)
                stage['baselinePacket'] = {'request': {**request, 'cwd': '<runtime>'},
                    'exitCode': baseline_proc.returncode, 'stdout': baseline_proc.stdout, 'stderr': baseline_proc.stderr,
                    'sourceBefore': baseline_before, 'sourceAfter': baseline_after,
                    'sourceUnchanged': invariant(baseline_before, baseline_after)}
                assert baseline_proc.returncode == 0, 'Baseline recall CLI failed'
                baseline_response = json.loads(baseline_proc.stdout)
                stage['baselinePacket']['response'] = baseline_response
                assert baseline_response['status'] == 'ok' and invariant(baseline_before, baseline_after), 'Baseline recall failed or changed source'
                assert all(m['id'] in own_ids and m['scope'] == 'Agent' for m in baseline_response['data']['recalls']), 'Foreign or deleted baseline record'
            report['stages'].append(stage)
            report['mappings'] = mappings
            save(folder / 'run.json', clean(report))
        report['status'] = 'completed'
    except Exception as exc:
        report['status'] = 'failed'
        report['error'] = {'type': type(exc).__name__, 'message': str(exc)}
        report['failedStage'] = stage
    finally:
        try:
            close_all()
        except Exception as exc:
            report['closeError'] = str(exc)
            report['status'] = 'failed'
        if db.exists():
            for agent in ('qual-a', 'qual-b'):
                try:
                    c = connect(agent, f'{episode["id"]}-{agent}-cleanup')
                    listing = require(c.tool('memory_list', {'limit': 1000, 'includeDormant': True}), agent)
                    deleted = []
                    for m in listing['memories']:
                        result = require(c.tool('memory_delete', {'id': m['id']}), agent)
                        assert result['deleted'] is True
                        deleted.append(m['id'])
                    remaining = require(c.tool('memory_list', {'limit': 1000, 'includeDormant': True}), agent)
                    assert remaining['count'] == 0 and remaining['memories'] == []
                    report['cleanup'].append({'agent': agent, 'deletedIds': deleted, 'verifiedEmpty': True})
                    close_all()
                except Exception as exc:
                    report['status'] = 'failed'
                    report['cleanup'].append({'agent': agent, 'verifiedEmpty': False, 'error': str(exc)})
        report['completedAt'] = now()
        report['mappings'] = mappings
        save(folder / 'run.json', clean(report))
    return report


def main():
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument('--cli', type=Path, required=True)
    ap.add_argument('--mcp', type=Path, required=True)
    ap.add_argument('--baseline-cli', type=Path)
    ap.add_argument('--output', type=Path, required=True)
    args = ap.parse_args()
    assert not args.output.exists(), 'Use a new output directory; preserve previous attempts'
    args.output.mkdir()
    protocol = json.loads((ROOT / 'protocol.json').read_text(encoding='utf-8'))
    validate_protocol(protocol)
    corpus = json.loads((ROOT / 'corpus.json').read_text(encoding='utf-8'))
    start = now()
    episodes = []
    for item in corpus['episodes']:
        report = episode(protocol, args.cli.resolve(), args.mcp.resolve(), item, args.output.resolve(), args.baseline_cli.resolve() if args.baseline_cli else None)
        episodes.append({'id': item['id'], 'status': report['status'], 'stageCount': len(report['stages']), 'error': report.get('error')})
        print(json.dumps(episodes[-1]), flush=True)
    result = {'schemaVersion': 'memory-realistic-replay/v1', 'startedAt': start, 'completedAt': now(),
        'sourceRevision': protocol['sourceRevision'], 'cliSha256': sha(args.cli), 'mcpSha256': sha(args.mcp),
        'corpusSha256': sha(ROOT / 'corpus.json'), 'protocolSha256': sha(ROOT / 'protocol.json'),
        'runnerSha256': sha(__file__), 'supportSha256': sha(ROOT / 'rpc_support.py'), 'episodes': episodes,
        'status': 'completed' if all(e['status'] == 'completed' and e['stageCount'] == 3 for e in episodes) else 'failed',
        'baselineCliSha256': sha(args.baseline_cli) if args.baseline_cli else None}
    save(args.output / 'run.json', result)
    return int(result['status'] != 'completed')


if __name__ == '__main__':
    raise SystemExit(main())
