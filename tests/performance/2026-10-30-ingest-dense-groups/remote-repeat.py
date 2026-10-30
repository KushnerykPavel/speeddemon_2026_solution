
from pathlib import Path
import json,subprocess
w=Path('/tmp/ingest-dense-e_fg95z0')
command=['taskset','-c','2','python3',str(w/'runner.py'),'--repo',str(w),'--data-root','/home/student/speed-demon-2026/data/rust-validation/a','--before',str(w/'before/target/release/ingest-baseline'),'--after',str(w/'track-a/target/release/ingest-baseline'),'--work',str(w/'batch2-retry'),'--summary',str(w/'summary2.json'),'--rustc-version-file',str(w/'rustc-version.txt'),'--reference-work','/tmp/ingest-byte-linux-mjj_tld6/batch1','--reference-summary',str(w/'summary1.json')]
p=subprocess.Popen(command,stdout=subprocess.PIPE,stderr=subprocess.STDOUT,text=True)
with (w/'raw2-retry.txt').open('w') as log:
 for line in p.stdout:
  log.write(line);print(line,end='',flush=True)
if p.wait(): raise RuntimeError('repeat failed')
(w/'repeat-command.json').write_text(json.dumps(command,indent=2)+'\n')
files={p.name:p.read_text() for p in w.iterdir() if p.is_file() and (p.name.startswith(('raw','summary')) or p.name in ('cpu.txt','rustc-version.txt','regression-tests.txt','repeat-command.json'))}
print('__ARTIFACTS__'+json.dumps(files),flush=True)
