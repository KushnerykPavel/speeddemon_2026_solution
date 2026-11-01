#!/usr/bin/env python3
"""Phase diagnostics on the VPS: diag-before vs diag-v3 (post-renumber contraction), 12 workers, alternating."""
import hashlib, json, os, re, shutil, statistics, subprocess, sys
from pathlib import Path
root = Path(sys.argv[1]).resolve(); root.chmod(0o777)
rust = 'rust:1-slim-trixie@sha256:1a939748a7cbab172de91a014e3804a97af1394a8c844faa67fce9401c4e40fb'
debian = 'debian:13-slim@sha256:62bd423aeee912639ca8ee681c2b762cb3e25137e0a92b20be455898a9f034ad'
data = Path('/home/student/speed-demon-2026/data/rust-validation/d/full-seed1')
expected = Path('/home/student/speed-demon-2026/results/rust-baseline-full/python-d-full-seed1/distances.txt').read_bytes()
names = {'diag-before': 'diag-before-main.rs', 'diag-v3': 'diag-v3-main.rs'}
build = ['docker', 'run', '--rm', '--network=none', '--user', f'{os.getuid()}:{os.getgid()}', '-e', 'CARGO_HOME=/tmp/cargo',
         '-e', 'RUSTFLAGS=-C target-cpu=native', '-v', f'{root}:/work']
for n, src in names.items():
    d = root/n; (d/'src').mkdir(parents=True, exist_ok=True); shutil.copy(root/src, d/'src'/'main.rs')
    for f in ['Cargo.toml', 'Cargo.lock']: shutil.copy(root/'cargo'/f, d/f)
    for p in [d, *d.rglob('*')]: p.chmod(0o777)
    subprocess.run(build + ['-w', f'/work/{n}', rust, 'cargo', 'build', '--release', '--offline', '--locked'], check=True)
subprocess.run(build + ['-w', '/work', rust, 'rustc', '--edition=2024', '-O', 'measure.rs', '-o', 'measure'], check=True)
common = ['docker', 'run', '--rm', '--network=none', '--read-only', '--cpuset-cpus=0-5', '--memory=8g', '--memory-swap=8g',
          '--pids-limit=512', '--cap-drop=ALL', '--security-opt=no-new-privileges', '--user', '65534:65534',
          '--tmpfs', '/tmp:rw,size=1g', '-v', f'{root}:/work', '-v', f'{data}:/data:ro']
rows = []
with (root/'phases.jsonl').open('w') as log:
    for r in range(9):
        for n in (list(names) if r % 2 == 0 else list(names)[::-1]):
            for f in data.iterdir(): f.read_bytes()
            o = root/f'out-{n}'; o.mkdir(exist_ok=True); o.chmod(0o777)
            res = subprocess.run(common + [debian, '/work/measure', f'/work/{n}/target/release/traverse-baseline', '--threads', '12',
                                 '--data', '/data', '--out', f'/work/out-{n}'], check=True, text=True, capture_output=True)
            assert (o/'distances.txt').read_bytes() == expected
            marks = {m.group(1): float(m.group(2)) for m in re.finditer(r'PHASE (\S+) (\S+)', res.stderr)}
            row = dict(json.loads(res.stdout), variant=n, round=r, marks=marks); rows.append(row)
            log.write(json.dumps(row) + '\n'); log.flush(); print(n, marks, flush=True)
summary = {}
for n in names:
    sel = [x for x in rows if x['variant'] == n and x['round'] > 0]
    keys = list(sel[0]['marks'])
    summary[n] = {'seconds_median': statistics.median(x['seconds'] for x in sel),
                  'peak_mb_max': max(x['cgroup_peak_bytes'] for x in sel) / 1e6,
                  'phase_ms_median': {k: 1000 * statistics.median(x['marks'][k] - (x['marks'][keys[i-1]] if i else 0) for x in sel)
                                      for i, k in enumerate(keys)}}
(root/'summary.json').write_text(json.dumps(summary, indent=2) + '\n'); print('SUMMARY=' + json.dumps(summary, indent=1))
