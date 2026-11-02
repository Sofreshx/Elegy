"""Reviewed local install plan; dry-run by default, never starts MCP/tunnel."""
import argparse
from contextlib import closing
import hashlib
import json
import os
from pathlib import Path
import re
import shutil
import sqlite3
import sys
import time
import tomllib
import uuid


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def safe_path(value):
    path = Path(value)
    if not path.is_absolute():
        raise ValueError('Absolute paths required')
    for item in [path, *path.parents]:
        if item.is_symlink() or item.is_junction():
            raise ValueError('Reparse paths rejected')
    return path


def entry_hash(entry):
    return hashlib.sha256(json.dumps(entry, sort_keys=True).encode()).hexdigest()


def fragment(entry):
    if set(entry) != {'command', 'env'} or not all(isinstance(v, str) for v in entry['env'].values()):
        raise ValueError('Unsupported memory configuration shape')
    allowed = {'ELEGY_DB_PATH', 'ELEGY_MCP_AGENT_ID', 'ELEGY_MCP_READ_SCOPE',
               'ELEGY_MCP_SHARED_SPACE', 'ELEGY_MCP_SHARED_AGENTS',
               'ELEGY_EMBEDDING_BOOT_POLICY', 'ELEGY_ALLOW_NO_EMBEDDINGS',
               'ELEGY_EMBEDDING_MODEL', 'OLLAMA_URL'}
    if not set(entry['env']).issubset(allowed):
        raise ValueError('Only non-credential Memory environment settings are permitted')
    if any(re.search(r'(?:sk-[A-Za-z0-9_-]{8,}|ghp_[A-Za-z0-9]+|Bearer\s+)', value) for value in entry['env'].values()):
        raise ValueError('Credential-shaped configuration value rejected')
    lines = ['[mcp_servers.elegy-memory]', 'command = '+json.dumps(entry['command']), '', '[mcp_servers.elegy-memory.env]']
    lines += [key+' = '+json.dumps(value) for key, value in entry['env'].items()]
    return '\n'.join(lines)+'\n'


def replace_entry(text, entry):
    before = tomllib.loads(text)
    parts = re.split(r'(?m)(?=^\[)', text)
    pattern = re.compile(r'^\[mcp_servers\.elegy-memory(?:\.env)?\]\s*(?:\n|$)')
    result = ''.join(part for part in parts if not pattern.match(part)).rstrip()+'\n\n'+fragment(entry)
    after = tomllib.loads(result)
    expected = json.loads(json.dumps(before))
    expected['mcp_servers']['elegy-memory'] = entry
    if after != expected:
        raise ValueError('Configuration preservation check failed')
    return result


def publish_new(temp, path):
    # Windows rename refuses an existing destination. Other platforms need an
    # exclusive hard-link instead, because their rename replaces the target.
    if os.name == 'nt':
        os.rename(temp, path)
    else:
        os.link(temp, path)


def atomic_text(path, text, *, exclusive=False, expected_text=None):
    temp = path.with_name(path.name+'.'+uuid.uuid4().hex+'.tmp')
    try:
        with temp.open('x', encoding='utf-8', newline='') as stream:
            stream.write(text)
            stream.flush()
            os.fsync(stream.fileno())
        # Recheck the complete input after temporary-file preparation. This is
        # not an indivisible compare-and-swap with external editors; all other
        # config writers must remain closed until installation has finished.
        if expected_text is not None and safe_path(str(path)).read_bytes() != expected_text.encode('utf-8'):
            raise ValueError('Configuration changed during replacement preparation; no configuration written')
        if exclusive:
            publish_new(temp, path)
        else:
            os.replace(temp, path)
    finally:
        if temp.exists():
            temp.unlink()


def save_receipt(path, receipt, phase):
    receipt = dict(receipt, phase=phase)
    atomic_text(safe_path(str(path)), json.dumps(receipt, indent=2))
    return receipt


def verify_snapshot(receipt, backup):
    snapshot = safe_path(receipt['backupPath'])
    if snapshot.parent != backup or not snapshot.name.endswith('.sqlite3'):
        raise ValueError('Unexpected snapshot boundary')
    if digest(snapshot) != receipt['backupSha256']:
        raise ValueError('Snapshot digest mismatch; preserve and review')
    with closing(sqlite3.connect(snapshot.as_uri()+'?mode=ro', uri=True)) as connection:
        if connection.execute('pragma integrity_check').fetchone()[0] != 'ok':
            raise ValueError('Snapshot integrity failed')
    return snapshot


def install_artifact(source, target, expected_hash):
    target = safe_path(str(target))
    if not target.exists():
        temp = target.with_name(target.name+'.'+uuid.uuid4().hex+'.tmp')
        try:
            with safe_path(source).open('rb') as input_file, temp.open('xb') as output:
                shutil.copyfileobj(input_file, output)
                output.flush()
                os.fsync(output.fileno())
            if digest(temp) != expected_hash:
                raise ValueError('Copied artifact digest mismatch')
            # Exclusive publication never replaces a differing file, including
            # one created by another installer during the copy.
            try:
                publish_new(temp, safe_path(str(target)))
            except FileExistsError:
                pass
        finally:
            if temp.exists():
                temp.unlink()
    if digest(safe_path(str(target))) != expected_hash:
        raise ValueError('Existing version directory differs; preserve and review')


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--plan', required=True)
    parser.add_argument('--plan-sha256', required=True)
    mode = parser.add_mutually_exclusive_group()
    mode.add_argument('--apply-approved', action='store_true')
    mode.add_argument('--rollback-config-approved', action='store_true')
    args = parser.parse_args()
    plan_file = safe_path(args.plan)
    if digest(plan_file) != args.plan_sha256.lower():
        raise ValueError('Plan digest mismatch')
    plan = json.loads(plan_file.read_text(encoding='utf-8'))
    commit = plan['sourceCommit']
    if not re.fullmatch('[0-9a-f]{40}', commit):
        raise ValueError('Exact committed source required')
    config = safe_path(plan['codexConfig'])
    database = safe_path(plan['database'])
    destination = safe_path(plan['installDirectory'])
    backup = safe_path(plan['backupDirectory'])
    if destination != config.parent/'bin'/'elegy-memory'/commit or database != config.parent.parent/'.elegy'/'memory.db':
        raise ValueError('Unexpected installation/data boundary')
    if backup != config.parent/'private-data'/'elegy-memory-dot'/'backups'/('before-shared-'+commit[:7]):
        raise ValueError('Unexpected backup boundary')
    text = config.read_bytes().decode('utf-8')
    current = tomllib.loads(text)['mcp_servers']['elegy-memory']
    desired = plan['codexMemoryEntry']
    if safe_path(desired['command']) != destination/'elegy-memory-mcp-stdio.exe' or safe_path(desired['env']['ELEGY_DB_PATH']) != database:
        raise ValueError('The Memory binding must use the pinned installed executable and database')
    fragment(desired)
    receipt_path = safe_path(str(backup/'install-receipt.json'))
    original_path = safe_path(str(backup/'codex-memory-before.toml'))
    receipt = None
    original = None
    if receipt_path.exists():
        receipt = json.loads(receipt_path.read_text(encoding='utf-8'))
        if (receipt.get('planSha256') != args.plan_sha256.lower()
                or receipt.get('sourceCommit') != commit
                or receipt.get('phase') not in {'prepared', 'snapshot-ready', 'configured', 'rolled-back'}):
            raise ValueError('Existing receipt differs from the reviewed plan; preserve and review')
    if original_path.exists():
        original = tomllib.loads(original_path.read_text(encoding='utf-8'))['mcp_servers']['elegy-memory']
        if entry_hash(original) != plan['expectedCurrentMemoryEntrySha256']:
            raise ValueError('Original binding backup differs from the reviewed plan')
    if receipt is not None and original is None:
        raise ValueError('Receipt requires the preserved original binding')
    if args.rollback_config_approved:
        if receipt is None or receipt['phase'] == 'prepared':
            raise ValueError('Rollback requires a verified installation receipt')
        verify_snapshot(receipt, backup)
        if entry_hash(current) not in {entry_hash(desired), entry_hash(original)}:
            raise ValueError('Current memory binding changed; review before rollback')
        fresh = safe_path(str(config)).read_bytes().decode('utf-8')
        if entry_hash(tomllib.loads(fresh)['mcp_servers']['elegy-memory']) != entry_hash(current):
            raise ValueError('Memory binding changed during rollback; no configuration written')
        atomic_text(config, replace_entry(fresh, original), expected_text=fresh)
        save_receipt(receipt_path, receipt, 'rolled-back')
        print(json.dumps({'configRestored': True, 'databaseRestoredOrDeleted': False, 'runtimeStarted': False}))
        return
    already_configured = entry_hash(current) == entry_hash(desired)
    if entry_hash(current) != plan['expectedCurrentMemoryEntrySha256'] and not (
            already_configured and receipt is not None and receipt['phase'] in {'snapshot-ready', 'configured'}):
        raise ValueError('Current memory binding changed; refresh the reviewed plan')
    for name, item in plan['artifacts'].items():
        if name not in {'elegy-memory.exe', 'elegy-memory-mcp-stdio.exe'} or digest(safe_path(item['path'])) != item['sha256']:
            raise ValueError('Artifact digest mismatch')
    if len(plan['artifacts']) != 2:
        raise ValueError('Both committed Memory executables required')
    profile = safe_path(plan['tunnelProfile'])
    if digest(profile) != plan['tunnelProfileSha256'] or not database.is_file():
        raise ValueError('Profile/database prerequisite failed')
    replace_entry(text, desired)
    if not args.apply_approved:
        print(json.dumps({'dryRun': True, 'configCompatible': True, 'databaseOpened': False, 'runtimeStarted': False}))
        return
    backup.mkdir(parents=True, exist_ok=True)
    if original is None:
        # Never replace an existing original binding. If interrupted before its
        # receipt, the reviewed entry hash is sufficient to resume this step.
        atomic_text(original_path, fragment(current), exclusive=True)
    if receipt is None:
        receipt = save_receipt(receipt_path, {
            'sourceCommit': commit, 'planSha256': args.plan_sha256.lower(),
            'runtimeStarted': False}, 'prepared')
    if receipt['phase'] == 'prepared':
        # A candidate without an integrity/hash receipt is never trusted on
        # retry. Keep it intact and create a fresh candidate with another name.
        snapshot = backup/'memory.sqlite3'
        if snapshot.exists():
            snapshot = backup/('memory-'+uuid.uuid4().hex+'.sqlite3')
        with safe_path(str(snapshot)).open('xb'):
            pass
        deadline = time.monotonic()+60
        def progress(status, remaining, total):
            if time.monotonic() > deadline:
                raise TimeoutError('Snapshot deadline exceeded')
        with closing(sqlite3.connect(database.as_uri()+'?mode=ro', uri=True)) as source, closing(sqlite3.connect(snapshot)) as target:
            source.execute('pragma query_only=on')
            source.backup(target, pages=256, progress=progress)
            if target.execute('pragma integrity_check').fetchone()[0] != 'ok':
                raise ValueError('Snapshot integrity failed')
        with snapshot.open('r+b') as stream:
            os.fsync(stream.fileno())
        receipt = save_receipt(receipt_path, dict(receipt, backupPath=str(snapshot),
                                                backupSha256=digest(snapshot)), 'snapshot-ready')
    else:
        verify_snapshot(receipt, backup)
    destination.mkdir(parents=True, exist_ok=True)
    for name, item in plan['artifacts'].items():
        install_artifact(item['path'], destination/name, item['sha256'])
    # Re-read after the snapshot/copy work to retain unrelated preference edits.
    # Close Codex before approval/apply; there is no cross-application file lock.
    fresh = safe_path(str(config)).read_bytes().decode('utf-8')
    if entry_hash(tomllib.loads(fresh)['mcp_servers']['elegy-memory']) != entry_hash(current):
        raise ValueError('Memory binding changed during installation; no configuration written')
    if not already_configured:
        atomic_text(config, replace_entry(fresh, desired), expected_text=fresh)
    save_receipt(receipt_path, receipt, 'configured')
    print(json.dumps({'installed': True, 'configurationUpdated': True, 'runtimeStarted': False, 'receipt': str(backup/'install-receipt.json')}))


if __name__ == '__main__':
    try:
        main()
    except Exception as error:
        print(json.dumps({'status': 'BLOCKED', 'errorType': type(error).__name__}))
        sys.exit(1)
