#!/usr/bin/env python3
"""Repeat controls and test parallel BFS CSR reconstruction in the same VPS directory."""
import hashlib
import json
import os
from pathlib import Path
import statistics
import subprocess
import sys

root = Path(sys.argv[1]).resolve()
rust = 'rust:1-slim-trixie@sha256:1a939748a7cbab172de91a014e3804a97af1394a8c844faa67fce9401c4e40fb'
debian = 'debian:13-slim@sha256:62bd423aeee912639ca8ee681c2b762cb3e25137e0a92b20be455898a9f034ad'
python_image = 'python:3.13-slim@sha256:d98cc29d3266e535dd9da1099688d9cb3ca4753813571e071763d17725460962'
data = Path('/home/student/speed-demon-2026/data/rust-validation/d/full-seed1')
expected = Path('/home/student/speed-demon-2026/results/rust-baseline-full/python-d-full-seed1/distances.txt').read_bytes()
assert hashlib.sha256(expected).hexdigest() == 'a23dbdf0a3e49cc8901310a9ee1dc6a9a569a85297b6da75c2a7002cb2ed277e'


def run(command):
    print('COMMAND='+repr(command),flush=True)
    result = subprocess.run(command,check=True,text=True,capture_output=True)
    if result.stderr: print(result.stderr,end='',flush=True)
    return result


build = ['docker','run','--rm','--network=none','--user',f'{os.getuid()}:{os.getgid()}',
         '-e','CARGO_HOME=/tmp/cargo','-e','RUSTFLAGS=-C target-cpu=native','-v',f'{root}:/work']
for variant in ['bfs-par','profile-bfs-par']:
    run(build + ['-w',f'/work/{variant}',rust,'cargo','build','--release','--offline','--locked'])
run(build + ['-w','/work',rust,'rustc','--edition=2024','--test','-O','tests/bfs-par-tests.rs','-o','bfs-par-tests'])
common = ['docker','run','--rm','--network=none','--read-only','--cpuset-cpus=0-5','--memory=8g',
          '--memory-swap=8g','--pids-limit=512','--cap-drop=ALL','--security-opt=no-new-privileges',
          '--user','65534:65534','--tmpfs','/tmp:rw,size=1g','-v',f'{root}:/work']
(root/'bfs-par-regression-tests.txt').write_text(run(common+[debian,'/work/bfs-par-tests']).stdout)
(root/'bfs-par-python-checks.txt').write_text(run(common+[python_image,'python3','/work/tests/traverse-check.py',
                                                             '/work/bfs-par/target/release/traverse-baseline']).stdout)
print((root/'bfs-par-regression-tests.txt').read_text(),flush=True)
print((root/'bfs-par-python-checks.txt').read_text(),flush=True)
variants = [('before',6),('before',12),('bfs',6),('bfs-par',6),('bfs-par',12)]
rows = []


def warm():
    for path in data.iterdir():
        with path.open('rb') as stream:
            while stream.read(1024*1024): pass


with (root/'refine-runs.jsonl').open('w') as log:
    for round_number in range(6):
        shift = round_number % len(variants)
        for variant,workers in variants[shift:] + variants[:shift]:
            warm();label=f'{variant}-{workers}'
            command = common + ['-v',f'{data}:/data:ro',debian,'/work/measure',
                                f'/work/{variant}/target/release/traverse-baseline','--threads',str(workers),
                                '--data','/data','--out',f'/work/refine-out-{label}']
            result = run(command)
            assert (root/f'refine-out-{label}/distances.txt').read_bytes() == expected
            row = dict(json.loads(result.stdout),variant=label,round=round_number,python_byte_equal=True)
            rows.append(row);log.write(json.dumps(row)+'\n');log.flush()
            print('RESULT='+json.dumps(row),flush=True)
summary = {'source_metadata':json.loads((root/'refine-source-metadata.json').read_text()),'rows':rows,'variants':{}}
for variant,workers in variants:
    label=f'{variant}-{workers}';selected=[row for row in rows if row['variant']==label and row['round']>0]
    times=sorted(row['seconds'] for row in selected)
    summary['variants'][label]={'trimmed_mean_seconds':statistics.mean(times[1:-1]),'median_seconds':statistics.median(times),
                              'spread_percent':100*(times[-1]/times[0]-1),'max_rss_kib':max(row['max_rss_kib'] for row in selected),
                              'max_cgroup_peak_bytes':max(row['cgroup_peak_bytes'] for row in selected)}
(root/'refine-summary.json').write_text(json.dumps(summary,indent=2)+'\n')
print('SUMMARY='+json.dumps(summary['variants']),flush=True)
profiles=[]
for round_number in range(3):
    warm()
    result=run(common+['-v',f'{data}:/data:ro',debian,'/work/measure',
                      '/work/profile-bfs-par/target/release/traverse-baseline','--threads','6','--data','/data',
                      '--out','/work/profile-out-bfs-par'])
    assert (root/'profile-out-bfs-par/distances.txt').read_bytes()==expected
    phases=next(json.loads(line.removeprefix('PHASES=')) for line in result.stderr.splitlines() if line.startswith('PHASES='))
    profiles.append(dict(round=round_number,phases=phases,measurement=json.loads(result.stdout),python_byte_equal=True))
(root/'refine-profiles.json').write_text(json.dumps(profiles,indent=2)+'\n')
print('PHASE_MEDIANS='+json.dumps({k:statistics.median(r['phases'][k] for r in profiles) for k in profiles[0]['phases']}),flush=True)
cases=[f'{scale}-seed{seed}' for scale in ['tiny','sample'] for seed in [1,7,42]]+['edge-cases','no-edges','no-queries']
checks=[]
for case in cases:
    case_data=data.parent/case
    reference=Path('/home/student/speed-demon-2026/results/rust-baseline-matrix')/f'python-d-{case}'/'distances.txt'
    result=run(common+['-v',f'{case_data}:/data:ro',debian,'/work/bfs-par/target/release/traverse-baseline',
                      '--threads','6','--data','/data','--out',f'/work/matrix-{case}'])
    expected_case=reference.read_bytes();actual=(root/f'matrix-{case}/distances.txt').read_bytes()
    assert actual==expected_case
    checks.append(dict(case=case,python_byte_equal=True,output_sha256=hashlib.sha256(actual).hexdigest(),
                       input_sha256={name:hashlib.sha256((case_data/name).read_bytes()).hexdigest() for name in ['graph.txt','queries.txt']}))
(root/'matrix-checks.json').write_text(json.dumps(checks,indent=2)+'\n')
print('MATRIX_CHECKS='+json.dumps(checks),flush=True)
print('DONE='+str(root),flush=True)
