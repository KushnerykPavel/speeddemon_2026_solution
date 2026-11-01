"""Run on VPS after remote-experiment.py; repeat full/miss-heavy from lean process."""
import hashlib
import json
import os
from pathlib import Path
import statistics
import subprocess
import sys
import time

work = Path(sys.argv[1])
os.sched_setaffinity(0, {2})
binaries = {'before':work/'before/target/release/ingest-baseline','after':work/'track-a/target/release/ingest-baseline'}
cases = {'full-seed1': Path('/home/student/speed-demon-2026/data/rust-validation/a/full-seed1'),
         'alternating-seed3':work/'synthetic/alternating-seed3'}
prior=json.loads((work/'summary.json').read_text())
expected={r['case']:r['output_sha256'] for r in prior['correctness']}

def sha(path):
    h=hashlib.sha256()
    with path.open('rb') as f:
        for b in iter(lambda:f.read(1048576),b''):h.update(b)
    return h.hexdigest()

result={'runs':[],'summary':{},'memory':[],'load_before':os.getloadavg()}
for name,data in cases.items():
    for r in range(7):
        for v in (['before','after'] if r%2==0 else ['after','before']):
            out=work/'repeat-out'/name/v;out.mkdir(parents=True,exist_ok=True)
            # Aggregate miss-heavy executions so individual ~0.1s samples do not dominate noise.
            iterations=5 if name=='alternating-seed3' else 1
            total=0
            for _ in range(iterations):
                start=time.perf_counter()
                subprocess.run([str(binaries[v]),'--data',str(data),'--out',str(out)],check=True)
                total+=time.perf_counter()-start
                assert sha(out/'summary.txt')==expected[name]
            row={'case':name,'variant':v,'round':r,'warmup':r==0,'iterations':iterations,'seconds_per_process':total/iterations}
            result['runs'].append(row)
    stats={}
    for v in binaries:
        rows=[r['seconds_per_process'] for r in result['runs'] if r['case']==name and r['variant']==v and not r['warmup']]
        stats[v]={'trimmed_mean':statistics.mean(sorted(rows)[1:-1]),'median':statistics.median(rows),'spread':max(rows)/min(rows)-1}
    stats['time_ratio']=stats['after']['trimmed_mean']/stats['before']['trimmed_mean']
    result['summary'][name]=stats
    print(name,json.dumps(stats),flush=True)
for v,binary in binaries.items():
    for r in range(3):
        mem=work/f'memory-{v}-{r}.txt'
        out=work/'memory-out'/v;out.mkdir(parents=True,exist_ok=True)
        subprocess.run(['/usr/bin/time','-f','%M','-o',str(mem),str(binary),'--data',str(cases['full-seed1']),'--out',str(out)],check=True)
        assert sha(out/'summary.txt')==expected['full-seed1']
        result['memory'].append({'variant':v,'round':r,'max_rss_bytes':int(mem.read_text())*1024})
result['load_after']=os.getloadavg()
(work/'repeat-summary.json').write_text(json.dumps(result,indent=2)+'\n')
print('REPEAT_JSON',json.dumps(result),flush=True)
