import json,pathlib,subprocess,shlex
r=pathlib.Path.cwd(); e=r/'tests/performance/2026-10-31-ingest-hour-cache'
files={
'before/src/main.rs':(e/'before-main.rs').read_text(),
 'track-a/src/main.rs':(e/'candidate-main.rs').read_text(),
 'tests/ingest-parser.rs':(e/'measured-tests.rs').read_text(),
 'tests/reference/a_ingest.py':(r/'tests/reference/a_ingest.py').read_text(),
}
for name in ['before','track-a']:
 for f in ['Cargo.toml','Cargo.lock']: files[f'{name}/{f}']=(e/(f+'.snapshot')).read_text()
bundle={'files':files,'commit':subprocess.check_output(['git','rev-parse','HEAD'],text=True).strip()}
p=subprocess.Popen(['ssh','-o','ConnectTimeout=10','-o','ServerAliveInterval=5','-o','ServerAliveCountMax=2','my-vps','python3 -c '+shlex.quote((e/'remote-experiment.py').read_text())],stdin=subprocess.PIPE,stdout=subprocess.PIPE,stderr=subprocess.STDOUT,text=True)
p.stdin.write(json.dumps(bundle));p.stdin.close()
with (e/'console.txt').open('w') as log:
 for line in p.stdout:
  if line.startswith('__ARTIFACTS__'):
   for n,s in json.loads(line[len('__ARTIFACTS__'):]).items(): (e/n).write_text(s)
  else:
   print(line,end='',flush=True);log.write(line);log.flush()
raise SystemExit(p.wait())
