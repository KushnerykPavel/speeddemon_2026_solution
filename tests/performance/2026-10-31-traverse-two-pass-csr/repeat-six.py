#!/usr/bin/env python3
"""Repeat the default-six-worker A/B because initial baseline spread exceeded 5%."""
from pathlib import Path
import hashlib
import json
import statistics
import subprocess
import sys

root = Path(sys.argv[1]).resolve()
data = Path('/home/student/speed-demon-2026/data/rust-validation/d/full-seed1')
expected = Path('/home/student/speed-demon-2026/results/rust-baseline-full/python-d-full-seed1/distances.txt').read_bytes()
assert hashlib.sha256(expected).hexdigest() == 'a23dbdf0a3e49cc8901310a9ee1dc6a9a569a85297b6da75c2a7002cb2ed277e'
image = 'debian:13-slim@sha256:62bd423aeee912639ca8ee681c2b762cb3e25137e0a92b20be455898a9f034ad'
common = ['docker','run','--rm','--network=none','--read-only','--cpuset-cpus=0-5','--memory=8g',
          '--memory-swap=8g','--pids-limit=512','--cap-drop=ALL','--security-opt=no-new-privileges',
          '--user','65534:65534','--tmpfs','/tmp:rw,size=1g','-v',f'{root}:/work','-v',f'{data}:/data:ro']
rows = []
with (root/'repeat-six-runs.jsonl').open('w') as log:
    for round_number in range(6):
        for variant in (['before','after'] if round_number % 2 == 0 else ['after','before']):
            for path in data.iterdir():
                with path.open('rb') as stream:
                    while stream.read(1024*1024):
                        pass
            project = 'before' if variant == 'before' else 'track-d'
            command = common + [image,'/work/measure',f'/work/{project}/target/release/traverse-baseline',
                                '--threads','6','--data','/data','--out',f'/work/repeat-out-{variant}']
            print('COMMAND='+repr(command),flush=True)
            completed = subprocess.run(command,check=True,text=True,capture_output=True)
            assert (root/f'repeat-out-{variant}/distances.txt').read_bytes() == expected
            row = dict(json.loads(completed.stdout),variant=variant,round=round_number,python_byte_equal=True)
            rows.append(row)
            log.write(json.dumps(row)+'\n'); log.flush()
            print('RESULT='+json.dumps(row),flush=True)
summary = {'reason':'Initial before-6 max/min spread was 10.47%; repeat only default-six-worker comparison.',
           'cpuset':'0-5','threads':6,'method':'one warmup; five alternating runs; drop min/max and average middle three',
           'memory_metric':'child wait4 ru_maxrss, KiB; not cgroup memory.peak', 'variants':{},'rows':rows}
for variant in ['before','after']:
    selected = [row for row in rows if row['variant']==variant and row['round']>0]
    times = sorted(row['seconds'] for row in selected)
    summary['variants'][variant] = {'trimmed_mean_seconds':statistics.mean(times[1:-1]),
                                   'median_seconds':statistics.median(times),
                                   'spread_percent':100*(times[-1]/times[0]-1),
                                   'max_rss_kib':max(row['max_rss_kib'] for row in selected)}
before,after = summary['variants']['before'],summary['variants']['after']
summary['time_change_percent'] = 100*(after['trimmed_mean_seconds']/before['trimmed_mean_seconds']-1)
(root/'repeat-six-summary.json').write_text(json.dumps(summary,indent=2)+'\n')
print('SUMMARY='+json.dumps(summary['variants']),flush=True)
