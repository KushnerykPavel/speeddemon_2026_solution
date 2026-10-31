#!/usr/bin/env python3
"""Offline D8 DFS/BFS A/B/C experiment in an isolated my-vps directory."""
import hashlib
import json
import os
from pathlib import Path
import statistics
import subprocess
import sys

root = Path(sys.argv[1]).resolve()
root.chmod(0o777)
print('REMOTE_ARTIFACTS=' + str(root), flush=True)
rust = 'rust:1-slim-trixie@sha256:1a939748a7cbab172de91a014e3804a97af1394a8c844faa67fce9401c4e40fb'
debian = 'debian:13-slim@sha256:62bd423aeee912639ca8ee681c2b762cb3e25137e0a92b20be455898a9f034ad'
python_image = 'python:3.13-slim@sha256:d98cc29d3266e535dd9da1099688d9cb3ca4753813571e071763d17725460962'
data = Path('/home/student/speed-demon-2026/data/rust-validation/d/full-seed1')
expected = Path('/home/student/speed-demon-2026/results/rust-baseline-full/python-d-full-seed1/distances.txt').read_bytes()
assert hashlib.sha256(expected).hexdigest() == 'a23dbdf0a3e49cc8901310a9ee1dc6a9a569a85297b6da75c2a7002cb2ed277e'
(root/'cpu.txt').write_text(subprocess.check_output(['lscpu'], text=True))
(root/'host.txt').write_text(subprocess.check_output(['uname','-a'], text=True))


def run(command, capture=False):
    print('COMMAND=' + repr(command), flush=True)
    return subprocess.run(command, check=True, text=True, capture_output=capture)


build = ['docker','run','--rm','--network=none','--user',f'{os.getuid()}:{os.getgid()}',
         '-e','CARGO_HOME=/tmp/cargo','-e','RUSTFLAGS=-C target-cpu=native','-v',f'{root}:/work']
for variant in ['before','dfs','bfs','profile-before','profile-dfs','profile-bfs']:
    run(build + ['-w',f'/work/{variant}',rust,'cargo','build','--release','--offline','--locked'])
run(build + ['-w','/work',rust,'rustc','--edition=2024','-O','measure.rs','-o','measure'])
(root/'compiler.txt').write_text(run(build + [rust,'rustc','-Vv'], True).stdout)
common = ['docker','run','--rm','--network=none','--read-only','--cpuset-cpus=0-5','--memory=8g',
          '--memory-swap=8g','--pids-limit=512','--cap-drop=ALL','--security-opt=no-new-privileges',
          '--user','65534:65534','--tmpfs','/tmp:rw,size=1g','-v',f'{root}:/work']
for variant in ['dfs','bfs']:
    run(build + ['-w','/work',rust,'rustc','--edition=2024','--test','-O',f'tests/{variant}-tests.rs','-o',f'{variant}-tests'])
    (root/f'{variant}-regression-tests.txt').write_text(run(common + [debian,f'/work/{variant}-tests'], True).stdout)
    (root/f'{variant}-python-checks.txt').write_text(run(common + [python_image,'python3','/work/tests/traverse-check.py',
                                                       f'/work/{variant}/target/release/traverse-baseline'], True).stdout)
    print('TESTS=' + variant + ' ' + (root/f'{variant}-regression-tests.txt').read_text(), flush=True)
    print('PYTHON_CHECKS=' + variant + ' ' + (root/f'{variant}-python-checks.txt').read_text(), flush=True)


def warm():
    for path in data.iterdir():
        with path.open('rb') as stream:
            while stream.read(1024*1024):
                pass


variants = [(name,n) for name in ['before','dfs','bfs'] for n in [6,12]]
rows = []
with (root/'runs.jsonl').open('w') as log:
    for round_number in range(6):
        for variant, workers in variants[round_number:] + variants[:round_number]:
            warm()
            label = f'{variant}-{workers}'
            command = common + ['-v',f'{data}:/data:ro',debian,'/work/measure',
                                f'/work/{variant}/target/release/traverse-baseline','--threads',str(workers),
                                '--data','/data','--out',f'/work/out-{label}']
            result = run(command, True)
            assert (root/f'out-{label}/distances.txt').read_bytes() == expected
            row = dict(json.loads(result.stdout), variant=label, round=round_number, python_byte_equal=True)
            rows.append(row)
            log.write(json.dumps(row)+'\n'); log.flush()
            print('RESULT='+json.dumps(row), flush=True)
summary = {'source_metadata':json.loads((root/'source-metadata.json').read_text()),
           'cpuset':'0-5','memory_limit':'8g','method':'one warmup, five rotating-order runs; trimmed mean of middle three',
           'output_sha256':hashlib.sha256(expected).hexdigest(),'rows':rows,'variants':{}}
for variant, workers in variants:
    label = f'{variant}-{workers}'
    selected = [row for row in rows if row['variant']==label and row['round']>0]
    seconds = sorted(row['seconds'] for row in selected)
    summary['variants'][label] = {'trimmed_mean_seconds':statistics.mean(seconds[1:-1]),
                                  'median_seconds':statistics.median(seconds),'spread_percent':100*(seconds[-1]/seconds[0]-1),
                                  'max_rss_kib':max(row['max_rss_kib'] for row in selected),
                                  'max_cgroup_peak_bytes':max(row['cgroup_peak_bytes'] for row in selected)}
summary['comparison'] = {}
for workers in [6,12]:
    before = summary['variants'][f'before-{workers}']
    for name in ['dfs','bfs']:
        after = summary['variants'][f'{name}-{workers}']
        summary['comparison'][f'{name}-{workers}'] = {
            'time_change_percent':100*(after['trimmed_mean_seconds']/before['trimmed_mean_seconds']-1),
            'memory_change_percent':100*(after['max_cgroup_peak_bytes']/before['max_cgroup_peak_bytes']-1)}
(root/'summary.json').write_text(json.dumps(summary,indent=2)+'\n')
print('SUMMARY='+json.dumps(summary['variants']),flush=True)
profiles = []
with (root/'profiles.jsonl').open('w') as log:
    for round_number in range(3):
        names = ['before','dfs','bfs']
        for variant in names[round_number:]+names[:round_number]:
            warm()
            command = common + ['-v',f'{data}:/data:ro',debian,'/work/measure',
                                f'/work/profile-{variant}/target/release/traverse-baseline','--threads','6',
                                '--data','/data','--out',f'/work/profile-out-{variant}']
            result = run(command, True)
            assert (root/f'profile-out-{variant}/distances.txt').read_bytes() == expected
            phases = next(json.loads(line.removeprefix('PHASES=')) for line in result.stderr.splitlines() if line.startswith('PHASES='))
            row = dict(variant=variant,round=round_number,measurement=json.loads(result.stdout),phases=phases,
                       stderr=result.stderr,python_byte_equal=True)
            profiles.append(row);log.write(json.dumps(row)+'\n');log.flush()
            print('PROFILE='+json.dumps(row),flush=True)
phase_summary = {}
for variant in ['before','dfs','bfs']:
    selected = [row for row in profiles if row['variant']==variant]
    phase_summary[variant] = {key:statistics.median(row['phases'][key] for row in selected) for key in selected[0]['phases']}
(root/'profile-summary.json').write_text(json.dumps(phase_summary,indent=2)+'\n')
print('PROFILE_SUMMARY='+json.dumps(phase_summary),flush=True)
print('DONE='+str(root),flush=True)
