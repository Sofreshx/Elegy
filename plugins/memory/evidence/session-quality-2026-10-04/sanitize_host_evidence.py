"""Remove local path prefixes, including JSON-escaped paths, from host evidence.

Original bytes are preserved under ignored runtime/ directories. No experiment
result, synthetic record, executable digest or tool result is changed.
"""
import hashlib
import json
from pathlib import Path

ROOT = Path(__file__).resolve().parent


def sanitize(value):
    if isinstance(value, str):
        prefixes = (str(Path.home()), Path.home().as_posix(), str(ROOT), ROOT.as_posix())
        for prefix in sorted(prefixes, key=len, reverse=True):
            variants = {prefix}
            for _ in range(4):
                variants |= {v.replace('\\', '\\\\') for v in variants}
            for variant in sorted(variants, key=len, reverse=True):
                value = value.replace(variant, '<local-path>')
        return value
    if isinstance(value, dict):
        return {key: sanitize(child) for key, child in value.items()}
    if isinstance(value, list):
        return [sanitize(child) for child in value]
    return value


if __name__ == '__main__':
    entries = []
    for folder in sorted(ROOT.glob('host-*')):
        if not folder.is_dir():
            continue
        for path in sorted(folder.glob('*.json')):
            before = path.read_bytes()
            original = json.loads(before)
            clean = sanitize(original)
            if original == clean:
                continue
            backup = folder / 'runtime' / ('before-redaction-' + path.name)
            backup.parent.mkdir(exist_ok=True)
            if backup.exists():
                raise RuntimeError('Backup exists; preserve prior evidence and review manually')
            backup.write_bytes(before)
            path.write_text(json.dumps(clean, indent=2, ensure_ascii=False) + '\n', encoding='utf-8')
            entries.append({'path': path.relative_to(ROOT).as_posix(),
                            'beforeSha256': hashlib.sha256(before).hexdigest(),
                            'afterSha256': hashlib.sha256(path.read_bytes()).hexdigest()})
    if entries:
        (ROOT / 'evidence-redaction.json').write_text(json.dumps({
            'schemaVersion': 'memory-evidence-redaction/v1',
            'scope': 'Local path prefixes only; raw originals remain local and ignored.',
            'entries': entries}, indent=2) + '\n', encoding='utf-8')
    print(json.dumps({'changedFiles': len(entries)}))
