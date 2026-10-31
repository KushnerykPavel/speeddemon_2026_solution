#!/usr/bin/env python3
"""Linux build and correctness of the THP-disable candidate; timing/memory beside 1767ec0.

The VPS kernel uses transparent huge pages only on madvise, so this cannot show
the board's memory effect; it checks that prctl builds, runs and costs no time.
"""
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
data = Path('/home/student/speed-demon-2026/data/rust-validation/d/full-seed1')
expected = Path('/home/student/speed-demon-2026/results/rust-baseline-full/python-d-full-seed1/distances.txt').read_bytes()
(root/'thp.txt').write_text(Path('/sys/kernel/mm/transparent_hugepage/enabled').read_text())
build = ['docker', 'run', '--rm', '--network=none', '--user', f'{os.getuid()}:{os.getgid()}',
         '-e', 'CARGO_HOME=/tmp/cargo', '-e', 'RUSTFLAGS=-C target-cpu=native', '-v', f'{root}:/work']
for variant in ['before', 'thp']:
    subprocess.run(build + ['-w', f'/work/{variant}', rust, 'cargo', 'build', '--release', '--offline', '--locked'], check=True)
subprocess.run(build + ['-w', '/work', rust, 'rustc', '--edition=2024', '-O', 'measure.rs', '-o', 'measure'], check=True)
common = ['docker', 'run', '--rm', '--network=none', '--read-only', '--cpuset-cpus=0-5', '--memory=8g',
          '--memory-swap=8g', '--pids-limit=512', '--cap-drop=ALL', '--security-opt=no-new-privileges',
          '--user', '65534:65534', '--tmpfs', '/tmp:rw,size=1g', '-v', f'{root}:/work', '-v', f'{data}:/data:ro']
variants = [(name, n) for name in ['before', 'thp'] for n in [6, 12]]
rows = []
for round_number in range(6):
    k = round_number % len(variants)
    for variant, workers in variants[k:] + variants[:k]:
        for path in data.iterdir():
            path.read_bytes()
        label = f'{variant}-{workers}'
        result = subprocess.run(common + [debian, '/work/measure', f'/work/{variant}/target/release/traverse-baseline',
                                          '--threads', str(workers), '--data', '/data', '--out', f'/work/out-{label}'],
                                check=True, text=True, capture_output=True)
        assert (root/f'out-{label}/distances.txt').read_bytes() == expected
        row = dict(json.loads(result.stdout), variant=label, round=round_number, python_byte_equal=True)
        rows.append(row)
        print('RESULT=' + json.dumps(row), flush=True)
summary = {}
for variant, workers in variants:
    label = f'{variant}-{workers}'
    seconds = sorted(r['seconds'] for r in rows if r['variant'] == label and r['round'] > 0)
    summary[label] = {'trimmed_mean_seconds': statistics.mean(seconds[1:-1]),
                      'spread_percent': 100 * (seconds[-1] / seconds[0] - 1),
                      'max_cgroup_peak_bytes': max(r['cgroup_peak_bytes'] for r in rows if r['variant'] == label and r['round'] > 0)}
(root/'runs.json').write_text(json.dumps(rows, indent=2) + '\n')
(root/'summary.json').write_text(json.dumps(summary, indent=2) + '\n')
print('SUMMARY=' + json.dumps(summary), flush=True)
print('DONE=' + str(root), flush=True)
