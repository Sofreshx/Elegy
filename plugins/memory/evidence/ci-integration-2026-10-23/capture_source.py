from pathlib import Path
import hashlib,json,struct,subprocess,sys
R=Path.cwd();Q=R/'plugins/memory/evidence/ci-integration-2026-10-23';OLD=R/'plugins/memory/evidence/stored-version-qualification-2026-10-23'
rev=sys.argv[1]
if subprocess.check_output(['git','rev-parse','HEAD']).decode().strip()!=rev:raise ValueError('HEAD differs from requested revision')
lock=json.loads((OLD/'source-lock.json').read_text())
paths=sorted({x['path'] for group in ('sourceFiles','mcpSourceFiles') for x in lock[group]})
if subprocess.check_output(['git','diff','--name-only','HEAD','--',*paths]):raise ValueError('Source is uncommitted')
batch=subprocess.check_output(['git','cat-file','--batch'],input=('\n'.join(rev+':'+p for p in paths)+'\n').encode())
items={};offset=0
for p in paths:
 end=batch.index(b'\n',offset);header=batch[offset:end].split();n=int(header[2]);data=batch[end+1:end+1+n];offset=end+n+2
 actual=(R/p).read_bytes();relation='exact'
 if actual!=data:
  if actual==data.replace(b'\r\n',b'\n').replace(b'\n',b'\r\n'):relation='LF-to-CRLF'
  elif actual==data.replace(b'\r\n',b'\n'):relation='CRLF-to-LF'
  else: raise AssertionError('Uncommitted source bytes: '+p)
 items[p]={'path':p,'sha256':hashlib.sha256(actual).hexdigest(),'gitBlobSha1':header[0].decode(),'gitContentSha256':hashlib.sha256(data).hexdigest(),'gitToCheckout':relation}
if offset!=len(batch):raise ValueError('Unexpected Git object output')
new={'schemaVersion':lock['schemaVersion'],'productSourceRevision':rev,'sourceFiles':[items[x['path']] for x in lock['sourceFiles']],'mcpSourceFiles':[items[x['path']] for x in lock['mcpSourceFiles']],'method':lock['method']}
h=hashlib.sha256()
for item in sorted(new['sourceFiles'],key=lambda x:x['path']):
 p=item['path'].encode();b=(R/item['path']).read_bytes();h.update(struct.pack('<Q',len(p)));h.update(p);h.update(struct.pack('<Q',len(b)));h.update(b)
new['cliSourceSha256']=h.hexdigest()
if (Q/'source-lock.json').exists():raise ValueError('Source lock already exists')
(Q/'source-lock.json').write_text(json.dumps(new,indent=2)+'\n',encoding='utf-8')
print(json.dumps({'sourceRevision':rev,'sourceFiles':len(new['sourceFiles']),'mcpSourceFiles':len(new['mcpSourceFiles']),'cliSourceSha256':h.hexdigest()}))
