"""Prepare reviewed qualification observations from saved synthetic evidence.

Does not execute models, MCP tools or the qualification recorder. Existing
observations are never overwritten. CLI record commands are documented in README.
"""
import hashlib
import json
from datetime import datetime, timezone
from pathlib import Path

ROOT = Path(__file__).resolve().parent


def load(name):
    return json.loads((ROOT / name).read_text(encoding='utf-8'))


def digest(value):
    return hashlib.sha256(json.dumps(value, sort_keys=True, separators=(',', ':')).encode()).hexdigest()


def evidence(*names):
    return [{'path': name, 'sha256': hashlib.sha256((ROOT / name).read_bytes()).hexdigest()} for name in names]


def events(packet):
    return [json.loads(line) for line in packet['stdout'].splitlines() if line.startswith('{')]


def tools(packet):
    return {e['item']['tool']: e['item'] for e in events(packet)
            if e.get('type') == 'item.completed' and e.get('item', {}).get('type') == 'mcp_tool_call'}


def result(tool):
    assert tool['status'] == 'completed' and not tool['error']
    return tool['result']['structured_content']


def check(name, passed, detail):
    return {'name': name, 'passed': passed, 'detail': detail}


def base(claim, artifact, host, version, config, sessions, started, checks, files):
    return {'schemaVersion': 'memory-observation/v1', 'claimId': claim, 'protocolVersion': 1,
            'subject': {'artifactSha256': artifact, 'host': host, 'hostVersion': version,
                        'configurationSha256': digest(config)},
            'sessions': sessions, 'startedAt': started,
            'completedAt': datetime.now(timezone.utc).isoformat(),
            'checks': checks, 'evidence': evidence(*files)}


def prepare():
    audit = load('packet-audit.json')
    assert audit['acceptance']['status'] == 'accepted_with_preserved_historical_failure'
    assert audit['auditScriptSha256'] == hashlib.sha256((ROOT / 'audit_packets.py').read_bytes()).hexdigest()
    assert all(hashlib.sha256((ROOT / name).read_bytes()).hexdigest() == expected
               for name, expected in audit['inputs'].items())
    assert all(value for key, value in audit['acceptance'].items()
               if isinstance(value, bool) and key != 'sqliteSnapshotIndependentSqlProof')
    destination = ROOT / 'observations'
    if destination.exists():
        raise ValueError('Preserve existing observations; use a separate evidence copy for another preparation')
    destination.mkdir()
    bindings = {}
    observations = {}
    for run_name in ('host-run-02', 'host-run-03'):
        run = load(f'{run_name}/run.json')
        first, second = [load(f'{run_name}/session-{i:02}.json') for i in (1, 2)]
        config = {'scope': 'Recorded scoped Codex argv excluding synthetic question; shared host defaults are not a sealed environment.',
                  'first': first['argv'][:-1], 'second': second['argv'][:-1]}
        bindings[run_name] = {'sha256': digest(config), 'configuration': config}
        written = result(tools(first)['memory_store'])
        recalled = result(tools(second)['memory_recall'])
        searched = result(tools(second)['memory_search'])
        identifier = written['memory']['id']
        assert written['action'] == 'added' and written['gateResult'] == 'accepted'
        assert recalled['found'] and recalled['memory']['id'] == identifier
        assert all(x['exitCode'] == 0 for x in (first, second))
        ids = [next(e['thread_id'] for e in events(p) if e['type'] == 'thread.started') for p in (first, second)]
        assert len(set(ids)) == 2
        assert written['memory']['content'] not in second['prompt']
        assert written['memory']['content'].split('recall code ')[1].rstrip('.') not in second['prompt']
        assert all(x['namespace'] == 'qual-a' for x in (written, recalled, searched))
        assert run['cleanup']['verifiedEmpty'] == {'qual-a': True, 'qual-b': True} and not run['cleanup']['errors']
        search_ok = identifier in [x['id'] for x in searched['results']]
        if run_name == 'host-run-03':
            assert search_ok
        else:
            assert not search_ok and searched['query'].startswith('label ')
        obs = base('host.cross-session', run['executables']['memoryMcp']['sha256'],
            'Codex CLI; temporary binding to source-built MCP', '0.160.0', config,
            [f'{run_name}-A1', f'{run_name}-A2'], run['createdAt'], [
                check('installed-client', True, 'Actual installed Codex executable; candidate MCP explicitly bound for this process only. Does not qualify the permanent default binding.'),
                check('stored-via-mcp', True, 'Completed memory_store event with accepted provenance and returned id.'),
                check('fresh-session-recall', search_ok, f'Distinct fresh thread ids; recall by id found=true, search count={searched["count"]}. Session 2 receives only id and topic label, never the stored marker.'),
                check('same-namespace', True, 'Both completed MCP results name qual-a, using the same dedicated scratch database.'),
                check('cleanup', True, 'Synthetic record deleted; fresh MCP lists report both namespaces empty.')],
            ['protocol.json', 'reused-evidence.json', f'{run_name}/run.json', f'{run_name}/preflight.json',
             f'{run_name}/session-01.json', f'{run_name}/session-02.json', 'host_probe.py'])
        if not search_ok:
            obs['error'] = 'Host added an unplanned label prefix to the search query. ID recall succeeded, but search returned zero. Preserve this incomplete protocol attempt; it is not a demonstrated persistence defect.'
        observations[f'{run_name}-cross-session.json'] = obs

    run = load('host-run-02/run.json')
    other = load('host-run-02/session-03.json')
    failures = [e for e in events(other) if e.get('type') == 'error']
    assert other['exitCode'] == 1 and failures[0]['error']['data']['statusCode'] == 403
    assert not tools(other)
    config = {'scope': 'Authored temporary shared-database binding, not a verified effective OpenCode tool session.',
              'codex': bindings['host-run-02']['configuration'],
              'opencodeArgv': other['argv'][:-1], 'database': '<same-scratch-database>',
              'agents': ['qual-a', 'qual-b'], 'readScope': 'agent', 'embeddings': 'off',
              'providers': ['opencode'], 'model': 'opencode/big-pickle', 'plugins': [], 'installDisabled': True}
    bindings['second-host'] = {'sha256': digest(config), 'configuration': config}
    obs = base('host.agent-isolation', run['executables']['memoryMcp']['sha256'],
        'Codex CLI + OpenCode; second software host blocked', 'Codex 0.160.0; OpenCode 1.18.21', config,
        ['host-run-02-A2', 'host-run-02-B1'], run['createdAt'], [
            check('same-database', False, 'Same scratch path authored for both clients; effective second-host tool binding unverified because provider rejected the session.'),
            check('owner-can-recall', True, 'Codex A completed memory_recall by id and received its marker.'),
            check('other-agent-cannot-recall', False, 'Unexecuted: no OpenCode memory ID/search/list call or B positive control. Absence of a response is not isolation proof.'),
            check('cleanup', True, 'A marker deleted and qual-a/qual-b lists empty after the provider error.')],
        ['protocol.json', 'host_probe.py', 'host-run-02/preflight.json', 'host-run-02/run.json',
         'host-run-02/session-02.json', 'host-run-02/session-03.json', 'source-run-02/run.json', 'installed-run-02/run.json'])
    obs['error'] = 'BLOCKED: installed OpenCode free provider returned HTTP 403 before any memory tool call. No paid fallback, installation, access grant or header spoofing attempted.'
    observations['second-host-isolation-blocked.json'] = obs

    assessment, run = load('timed-assessment.json'), load('timed-run-01/run.json')
    assert assessment['status'] == 'valid' and assessment['inventory']['arms'] == 24
    assert assessment['scores']['forbiddenAnswerCount'] == 0
    config = {'requestedModel': run['model'], 'reasoningEffort': run['reasoningEffort'],
              'configArgHash': run['configArgHash'], 'corpusSha256': run['corpusSha256'],
              'protocolSha256': run['protocolSha256'], 'packetSha256': run['sourcePacketsSha256'],
              'hostSha256': run['codex']['sha256'], 'runtime': 'one fresh process per arm; no MCP or tools',
              'limits': 'Same requested alias and authored options; hosted backend version, host defaults and temperature are not sealed.'}
    bindings['answer-quality'] = {'sha256': digest(config), 'configuration': config}
    subject = load('reused-evidence.json')['build']['cliSha256']
    obs = base('recall.answer-quality', subject, 'Codex CLI with saved source-built contextual recall packets', '0.160.0', config,
        [f'timed-{row["id"]}-{arm}' for row in assessment['cases'] for arm in ('with', 'without')],
        run['createdAt'], [
            check('paired-inputs', True, '24 complete arms; same frozen question and instruction; exact uncurated CLI envelope only in with arm. Input digests reconstructed offline.'),
            check('fixed-model-config', True, 'Same requested gpt-5.6-luna/medium and command shape; 24 distinct fresh host sessions. Hosted alias is not an immutable backend guarantee.'),
            check('predeclared-rubric', True, 'Frozen exact-match rubric, expectations and negative controls unchanged. Timed repeat declared before its execution after initial uninstrumented trial.'),
            check('independent-scoring', True, 'Deterministic offline scorer compares saved final messages with frozen expectations; no subjective model judge.'),
            check('negative-controls', True, 'Current instruction overrides old port; unknown signing key/private marker/contact abstain; hostile recalled instruction ignored; irrelevant arithmetic unaffected.')],
        ['protocol.json', 'corpus.json', 'timed-repeat-protocol.json', 'source-run-01/packets.json', 'source-run-02/run.json',
         'timed_answers.py', 'timed-run-01/run.json', 'timed-run-01/answers.json', 'assess_timed.py', 'timed-assessment.json',
         'answers.json', 'assessment.json', 'reused-evidence.json', 'packet-audit.json'])
    obs['completedAt'] = run['completedAt']
    obs['costUnit'] = 'paid-api-calls'
    obs['pairedCases'] = [{'id': row['id'], 'scoreWith': row['arms']['with']['score'],
        'scoreWithout': row['arms']['without']['score'], 'latencyWithMs': row['arms']['with']['elapsedMs'],
        'latencyWithoutMs': row['arms']['without']['elapsedMs'], 'costWith': 0, 'costWithout': 0}
        for row in assessment['cases']]
    observations['timed-answer-quality.json'] = obs
    (ROOT / 'host-config-bindings.json').write_text(json.dumps(bindings, indent=2) + '\n', encoding='utf-8')
    for name, observation in observations.items():
        observation['evidence'] += evidence('host-config-bindings.json', 'mcp-source-binding.json')
        assert len(observation['evidence']) <= 16
        (destination / name).write_text(json.dumps(observation, indent=2) + '\n', encoding='utf-8')
    print(json.dumps({'observations': list(observations)}))


if __name__ == '__main__':
    prepare()
