#!/usr/bin/env python3
"""Search tail profile: per-query begin/end times and per-worker finish times for 21df4fd."""
import hashlib
import json
import os
from pathlib import Path
import subprocess
import sys

root = Path(sys.argv[1]).resolve()
root.chmod(0o777)
print('REMOTE_ARTIFACTS=' + str(root), flush=True)
rust = 'rust:1-slim-trixie@sha256:1a939748a7cbab172de91a014e3804a97af1394a8c844faa67fce9401c4e40fb'
debian = 'debian:13-slim@sha256:62bd423aeee912639ca8ee681c2b762cb3e25137e0a92b20be455898a9f034ad'
data = Path('/home/student/speed-demon-2026/data/rust-validation/d/full-seed1')
expected = Path('/home/student/speed-demon-2026/results/rust-baseline-full/python-d-full-seed1/distances.txt').read_bytes()
assert hashlib.sha256(expected).hexdigest() == 'a23dbdf0a3e49cc8901310a9ee1dc6a9a569a85297b6da75c2a7002cb2ed277e'
build = ['docker', 'run', '--rm', '--network=none', '--user', f'{os.getuid()}:{os.getgid()}',
         '-e', 'CARGO_HOME=/tmp/cargo', '-e', 'RUSTFLAGS=-C target-cpu=native', '-v', f'{root}:/work']
subprocess.run(build + ['-w', '/work/tail', rust, 'cargo', 'build', '--release', '--offline', '--locked'], check=True)
subprocess.run(build + ['-w', '/work', rust, 'rustc', '--edition=2024', '-O', 'measure.rs', '-o', 'measure'], check=True)
common = ['docker', 'run', '--rm', '--network=none', '--read-only', '--cpuset-cpus=0-5', '--memory=8g',
          '--memory-swap=8g', '--pids-limit=512', '--cap-drop=ALL', '--security-opt=no-new-privileges',
          '--user', '65534:65534', '--tmpfs', '/tmp:rw,size=1g', '-v', f'{root}:/work', '-v', f'{data}:/data:ro']
with (root/'tail.jsonl').open('w') as log:
    for round_number in range(4):
        for workers in [6, 12]:
            for path in data.iterdir():
                path.read_bytes()
            result = subprocess.run(common + [debian, '/work/measure', '/work/tail/target/release/traverse-baseline',
                                              '--threads', str(workers), '--data', '/data', '--out', f'/work/out-{workers}'],
                                    check=True, text=True, capture_output=True)
            assert (root/f'out-{workers}/distances.txt').read_bytes() == expected
            tail = next(json.loads(line.removeprefix('TAIL=')) for line in result.stderr.splitlines()
                        if line.startswith('TAIL='))
            row = dict(workers=workers, round=round_number, measurement=json.loads(result.stdout), **tail)
            log.write(json.dumps(row) + '\n')
            print('ROUND', round_number, workers, json.loads(result.stdout)['seconds'], flush=True)
print('DONE=' + str(root), flush=True)
