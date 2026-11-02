#!/usr/bin/env python3
"""Phase split (wall and process CPU at each mark) of ours vs HighVelocity, diagnostic builds. Runs on the VPS."""
import hashlib, json, os, re, statistics, subprocess, sys
from pathlib import Path
root = Path(sys.argv[1]).resolve()
rust = 'rust:1-slim-trixie@sha256:1a939748a7cbab172de91a014e3804a97af1394a8c844faa67fce9401c4e40fb'
gcc = 'gcc:15@sha256:eb5e64286ad47187b32bda75c6b6916166ecb41da56559b8b09e0001748d2cce'
debian = 'debian:13-slim@sha256:62bd423aeee912639ca8ee681c2b762cb3e25137e0a92b20be455898a9f034ad'
data = Path('/home/student/speed-demon-2026/data/rust-validation/d/full-seed1')
expected = Path('/home/student/speed-demon-2026/results/rust-baseline-full/python-d-full-seed1/distances.txt').read_bytes()
def run(c, cap=False):
    print('COMMAND=' + ' '.join(c)[:200], flush=True); return subprocess.run(c, check=True, text=True, capture_output=cap)
for p in [root, *root.rglob('*')]: p.chmod(0o777)
build = ['docker', 'run', '--rm', '--network=none', '--user', f'{os.getuid()}:{os.getgid()}', '-e', 'CARGO_HOME=/tmp/cargo', '-v', f'{root}:/work']
run(build + ['-w', '/work/src/hvnb', gcc, 'g++', '-O3', '-std=c++17', '-pthread', '-flto', '-static', '-s', '-o', 'traverse', 'd_traverse.cpp'])
run(build + ['-w', '/work/src/hvnp', gcc, 'g++', '-O3', '-std=c++17', '-pthread', '-flto', '-static', '-s', '-o', 'traverse', 'd_traverse.cpp'])
run(build + ['-w', '/work', rust, 'rustc', '--edition=2024', '-O', 'measure.rs', '-o', 'measure'])
BIN = {'hvnb': '/work/src/hvnb/traverse', 'hvnp': '/work/src/hvnp/traverse'}
common = ['docker', 'run', '--rm', '--network=none', '--read-only', '--cpuset-cpus=0-5', '--memory=8g', '--memory-swap=8g',
          '--pids-limit=512', '--cap-drop=ALL', '--security-opt=no-new-privileges', '--user', '65534:65534',
          '--tmpfs', '/tmp:rw,size=1g', '-v', f'{root}:/work']
rows = []
with (root/'phases-np.jsonl').open('w') as log:
    for w in [6]:
        for rnd in range(9):
            for name in (['hvnb', 'hvnp'] if rnd % 2 == 0 else ['hvnp', 'hvnb']):
                for f in data.iterdir(): f.read_bytes()
                o = root/'out'/f'{name}-{w}'; o.mkdir(parents=True, exist_ok=True); o.chmod(0o777)
                args = []
                r = run(common + ['-e', f'SD_THREADS={w}', '-v', f'{data}:/data:ro', '-v', f'{o}:/out', debian, '/work/measure', BIN[name], *args], True)
                assert (o/'distances.txt').read_bytes() == expected
                marks = [(m.group(1), float(m.group(2)), float(m.group(3))) for m in re.finditer(r'PHASE (\S+) (\S+) (\S+)', r.stderr)]
                row = dict(json.loads(r.stdout), variant=name, workers=w, round=rnd, marks=marks)
                rows.append(row); log.write(json.dumps(row) + '\n'); log.flush()
                print('RUN', name, w, rnd, round(row['seconds'], 4), flush=True)
summary = {}
for w in [6]:
    for name in BIN:
        sel = [r for r in rows if r['variant'] == name and r['workers'] == w and r['round'] > 0]
        labels = [m[0] for m in sel[0]['marks']]
        ph = {}
        for i, l in enumerate(labels):
            pw = [r['marks'][i][1] - (r['marks'][i-1][1] if i else 0) for r in sel]
            pc = [r['marks'][i][2] - (r['marks'][i-1][2] if i else 0) for r in sel]
            ph[l] = {'wall_ms': 1e3*statistics.median(pw), 'cpu_ms': 1e3*statistics.median(pc)}
        summary[f'{name}-{w}'] = {'total_s_median': statistics.median(r['seconds'] for r in sel),
                                 'last_mark_s_median': statistics.median(r['marks'][-1][1] for r in sel), 'phases': ph}
(root/'phases-np-summary.json').write_text(json.dumps(summary, indent=1) + '\n')
print('SUMMARY=' + json.dumps(summary, indent=1), flush=True)
