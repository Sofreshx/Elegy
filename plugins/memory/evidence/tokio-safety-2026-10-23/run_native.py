"""Capture a native check once against the committed, locked qualification source."""
from pathlib import Path
import argparse,hashlib,json,re,subprocess,sys,time
from datetime import datetime,timezone
Q=Path(__file__).resolve().parent
REPO=Q.parents[3]
def sha(p):return hashlib.sha256(p.read_bytes()).hexdigest()
def now():return datetime.now(timezone.utc).isoformat()
def snapshot(lock):
 entries={e['path']:e['sha256'] for key in ('sourceFiles','mcpSourceFiles') for e in lock[key]}
 actual={p:sha(REPO/p) for p in entries}
 if entries!=actual:raise ValueError('qualification source drift')
 return hashlib.sha256(json.dumps(actual,sort_keys=True,separators=(',',':')).encode()).hexdigest()
def _windows_path(value):
 return bool(re.match(r"^[A-Za-z]:[\\/]",value))
def _path_key(value):
 return value.replace('\\','/').casefold() if _windows_path(value) else value
def _path_suffix(value,prefix):
 value_key,prefix_key=_path_key(value),_path_key(prefix)
 if value_key==prefix_key:return ''
 if not value_key.startswith(prefix_key) or value_key[len(prefix_key)]!='/':return None
 return value.replace('\\','/')[len(prefix_key):]
def _same_path(left,right):
 return _path_key(left)==_path_key(right)
def public(args):
 result=[]
 for arg in args:
  if _same_path(arg,sys.executable):
   result.append('<python>');continue
  suffix=_path_suffix(arg,str(REPO))
  if suffix is not None:
   result.append('<repo>'+suffix);continue
  suffix=_path_suffix(arg,str(Path.home()))
  if suffix is not None:
   result.append('<home>'+suffix);continue
  result.append(arg)
 return result
if __name__=='__main__':
 p=argparse.ArgumentParser();p.add_argument('--name',required=True);p.add_argument('--timeout',type=int,default=7200);p.add_argument('command',nargs=argparse.REMAINDER);a=p.parse_args()
 if not a.name or any(c not in 'abcdefghijklmnopqrstuvwxyz0123456789-' for c in a.name):raise ValueError('invalid check name')
 command=a.command[1:] if a.command[:1]==['--'] else a.command
 if not command:raise ValueError('command required')
 out=Q/'native-validation';out.mkdir(exist_ok=True)
 record=out/(a.name+'.json');log=out/(a.name+'.log')
 if record.exists() or log.exists():raise ValueError('check already recorded')
 lock=json.loads((Q/'source-lock.json').read_text(encoding='utf-8'))
 head=subprocess.check_output(['git','rev-parse','HEAD'],cwd=REPO).decode().strip()
 before=snapshot(lock);start=now();tick=time.monotonic()
 r=subprocess.run(command,cwd=REPO,capture_output=True,timeout=a.timeout)
 # Commands used here produce no credentials. Preserve their exact output; publication scans still apply.
 data=r.stdout+b'\n--- stderr ---\n'+r.stderr;log.write_bytes(data)
 after=snapshot(lock);endhead=subprocess.check_output(['git','rev-parse','HEAD'],cwd=REPO).decode().strip()
 data={'schemaVersion':'memory-final-native-check/v1','argv':public(command),'startedAt':start,'completedAt':now(),'durationSeconds':round(time.monotonic()-tick,3),'exitCode':r.returncode,'log':log.name,'logSha256':sha(log),'sourceRevision':lock['productSourceRevision'],'gitHeadBefore':head,'gitHeadAfter':endhead,'sourceLockSha256':sha(Q/'source-lock.json'),'sourceSnapshotBeforeSha256':before,'sourceSnapshotAfterSha256':after,'sourceUnchanged':before==after,'sourceFiles':149,'mcpSourceFiles':18}
 record.write_text(json.dumps(data,indent=2)+'\n',encoding='utf-8')
 print(json.dumps(data));raise SystemExit(r.returncode if before==after else 1)
