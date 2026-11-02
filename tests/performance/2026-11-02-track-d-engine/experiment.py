#!/usr/bin/env python3
"""Track D: bucket-ring search engine (engine) vs 539b15f (before). Runs on the VPS.

usage: experiment.py ROOT   (ROOT holds before-main.rs, engine-main.rs, diag-*-main.rs, measure.rs,
                             engine-tests.rs, tests/, cargo/)
Same protocol as 2026-11-01-traverse-mu-prune: pinned Rust image, native offline release builds, Docker
CPU set 0-5, 8 GB cgroup, prewarmed input, alternating order, round 0 discarded, trimmed mean.
"""
import hashlib
import json
import os
from pathlib import Path
import re
import shutil
import statistics
import subprocess
import sys

root = Path(sys.argv[1]).resolve()
root.chmod(0o777)
rust = 'rust:1-slim-trixie@sha256:1a939748a7cbab172de91a014e3804a97af1394a8c844faa67fce9401c4e40fb'
debian = 'debian:13-slim@sha256:62bd423aeee912639ca8ee681c2b762cb3e25137e0a92b20be455898a9f034ad'
python_image = 'python:3.13-slim@sha256:d98cc29d3266e535dd9da1099688d9cb3ca4753813571e071763d17725460962'
datasets = Path('/home/student/speed-demon-2026/data/rust-validation/d')
data = datasets / 'full-seed1'
expected = Path('/home/student/speed-demon-2026/results/rust-baseline-full/python-d-full-seed1/distances.txt').read_bytes()
assert hashlib.sha256(expected).hexdigest() == 'a23dbdf0a3e49cc8901310a9ee1dc6a9a569a85297b6da75c2a7002cb2ed277e'
(root/'cpu.txt').write_text(subprocess.check_output(['lscpu'], text=True))
thp_state = {f: Path(f'/sys/kernel/mm/transparent_hugepage/{f}').read_text().strip() for f in ['enabled', 'defrag']}
thp_state['kernel'] = os.uname().release
(root/'thp-state.json').write_text(json.dumps(thp_state, indent=2) + '\n')
print('THP=' + json.dumps(thp_state), flush=True)


def run(command, capture=False):
    print('COMMAND=' + ' '.join(map(str, command))[:300], flush=True)
    return subprocess.run(command, check=True, text=True, capture_output=capture)


VARIANTS = {'before': 'before-main.rs', 'engine': 'engine-main.rs',
            'diag-before': 'diag-before-main.rs', 'diag-engine': 'diag-engine-main.rs'}
for name, source in VARIANTS.items():
    d = root/name
    (d/'src').mkdir(parents=True, exist_ok=True)
    shutil.copy(root/source, d/'src'/'main.rs')
    for f in ['Cargo.toml', 'Cargo.lock']:
        shutil.copy(root/'cargo'/f, d/f)
    for p in [d, *d.rglob('*')]:
        p.chmod(0o777)

build = ['docker', 'run', '--rm', '--network=none', '--user', f'{os.getuid()}:{os.getgid()}',
         '-e', 'CARGO_HOME=/tmp/cargo', '-e', 'RUSTFLAGS=-C target-cpu=native', '-v', f'{root}:/work']
for name in VARIANTS:
    run(build + ['-w', f'/work/{name}', rust, 'cargo', 'build', '--release', '--offline', '--locked'])
run(build + ['-w', '/work', rust, 'rustc', '--edition=2024', '-O', 'measure.rs', '-o', 'measure'])
(root/'compiler.txt').write_text(run(build + [rust, 'rustc', '-Vv'], True).stdout)

common = ['docker', 'run', '--rm', '--network=none', '--read-only', '--cpuset-cpus=0-5', '--memory=8g',
          '--memory-swap=8g', '--pids-limit=512', '--cap-drop=ALL', '--security-opt=no-new-privileges',
          '--user', '65534:65534', '--tmpfs', '/tmp:rw,size=1g', '-v', f'{root}:/work']

# All-pairs vs an independent u64 Dijkstra on 300 random graphs (weights 0..~2^32), both sources.
for label in ['engine', 'before']:
    d = root/f'unit-{label}'
    d.mkdir(exist_ok=True)
    d.chmod(0o777)
    shutil.copy(root/VARIANTS[label], d/'cand-main.rs')
    shutil.copy(root/'engine-tests.rs', d/'engine-tests.rs')
    run(build + ['-w', f'/work/unit-{label}', rust, 'rustc', '--edition=2024', '--test', '-O', 'engine-tests.rs', '-o', 'unit'])
    (root/f'unit-{label}.txt').write_text(run(common + [debian, f'/work/unit-{label}/unit'], True).stdout)
    print(f'UNIT {label}: ' + (root/f'unit-{label}.txt').read_text().strip().splitlines()[-1], flush=True)

for name in ['before', 'engine']:
    out = run(common + [python_image, 'python3', '/work/tests/traverse-check.py',
                        f'/work/{name}/target/release/traverse-baseline'], True).stdout
    (root/f'python-checks-{name}.txt').write_text(out)
    print(f'PYTHON {name}: {out.strip()}', flush=True)

for case in sorted(datasets.iterdir()):
    outs = {}
    for name in ['before', 'engine']:
        o = root/f'cmp-{name}-{case.name}'
        o.mkdir(exist_ok=True)
        o.chmod(0o777)
        run(common + ['-v', f'{case}:/data:ro', debian, f'/work/{name}/target/release/traverse-baseline',
                      '--data', '/data', '--out', f'/work/cmp-{name}-{case.name}'])
        outs[name] = (o/'distances.txt').read_bytes()
    assert outs['before'] == outs['engine'], case.name
    print(f'EQUAL {case.name} {hashlib.sha256(outs["engine"]).hexdigest()[:12]}', flush=True)


def warm():
    for path in data.iterdir():
        with path.open('rb') as stream:
            while stream.read(1024*1024):
                pass


def measured(name, workers, out_label):
    warm()
    (root/f'out-{out_label}').mkdir(exist_ok=True)
    (root/f'out-{out_label}').chmod(0o777)
    cmd = common + ['-v', f'{data}:/data:ro', debian, '/work/measure', f'/work/{name}/target/release/traverse-baseline',
                    '--threads', str(workers), '--data', '/data', '--out', f'/work/out-{out_label}']
    return run(cmd, True)


ROUNDS = int(os.environ.get('ROUNDS', '11'))

# Diagnostic phase builds (not the timed binaries): phase marks and AnonHugePages after the transpose.
diag_rows = []
with (root/'diag.jsonl').open('w') as log:
    for round_number in range(6):
        order = ['diag-before', 'diag-engine'] if round_number % 2 == 0 else ['diag-engine', 'diag-before']
        for name in order:
            result = measured(name, 12, name)
            assert (root/f'out-{name}/distances.txt').read_bytes() == expected
            marks = dict((m.group(1), float(m.group(2))) for m in re.finditer(r'PHASE (\S+) (\S+) \S+', result.stderr))
            cpu = dict((m.group(1), float(m.group(2))) for m in re.finditer(r'PHASE (\S+) \S+ (\S+)', result.stderr))
            exp = re.search(r'EXPANDED (\d+)', result.stderr)
            row = dict(json.loads(result.stdout), variant=name, round=round_number, marks=marks, cpu=cpu,
                       expanded=int(exp.group(1)) if exp else None)
            diag_rows.append(row)
            log.write(json.dumps(row) + '\n')
            log.flush()
            print(f'DIAG {name} expanded={row["expanded"]} marks={marks}', flush=True)

names = ['before', 'engine']
rows = []
with (root/'runs.jsonl').open('w') as log:
    for workers in [12, 6]:
        for round_number in range(ROUNDS):
            order = names if round_number % 2 == 0 else names[::-1]
            for name in order:
                label = f'{name}-{workers}'
                result = measured(name, workers, label)
                assert (root/f'out-{label}/distances.txt').read_bytes() == expected
                row = dict(json.loads(result.stdout), variant=name, workers=workers, round=round_number)
                rows.append(row)
                log.write(json.dumps(row) + '\n')
                log.flush()

summary = {'cpuset': '0-5', 'rounds_excluding_warmup': ROUNDS - 1, 'order': 'alternating', 'thp': thp_state,
           'method': 'round 0 discarded; trimmed mean (drop min/max) of remaining rounds',
           'output_sha256': hashlib.sha256(expected).hexdigest(), 'results': {}, 'diag': {}}
for name in ['diag-before', 'diag-engine']:
    sel = [r for r in diag_rows if r['variant'] == name and r['round'] > 0]
    phase = lambda r, a, b: r['marks'][b] - r['marks'][a]
    summary['diag'][name] = {
        'expanded': sorted({r['expanded'] for r in sel}),
        'search_cpu_s_median': statistics.median(r['cpu']['search'] - r['cpu']['transpose'] for r in sel),
        'load_s_median': statistics.median(r['marks']['load'] for r in sel),
        'reorder_s_median': statistics.median(phase(r, 'queries', 'reorder') for r in sel),
        'transpose_s_median': statistics.median(phase(r, 'reorder', 'transpose') for r in sel),
        'search_s_median': statistics.median(phase(r, 'transpose', 'search') for r in sel),
        'total_s_median': statistics.median(r['marks']['output'] for r in sel)}
for workers in [12, 6]:
    entry = {}
    for name in names:
        sel = [r for r in rows if r['variant'] == name and r['workers'] == workers and r['round'] > 0]
        secs = sorted(r['seconds'] for r in sel)
        entry[name] = {'trimmed_mean_s': statistics.mean(secs[1:-1]), 'median_s': statistics.median(secs),
                       'spread_percent': 100*(secs[-1]/secs[0]-1),
                       'user_s_median': statistics.median(r['user_s'] for r in sel),
                       'sys_s_median': statistics.median(r['sys_s'] for r in sel),
                       'minflt_median': statistics.median(r['minflt'] for r in sel),
                       'max_cgroup_peak_bytes': max(r['cgroup_peak_bytes'] for r in sel)}
    pairs = {}
    for r in rows:
        if r['workers'] == workers and r['round'] > 0:
            pairs.setdefault(r['round'], {})[r['variant']] = r['seconds']
    deltas = [100*(p['engine']/p['before']-1) for p in pairs.values()]
    entry['change_percent_trimmed_mean'] = 100*(entry['engine']['trimmed_mean_s']/entry['before']['trimmed_mean_s']-1)
    entry['paired_change_percent_median'] = statistics.median(deltas)
    entry['paired_engine_faster_rounds'] = f'{sum(d < 0 for d in deltas)}/{len(deltas)}'
    summary['results'][str(workers)] = entry
(root/'summary.json').write_text(json.dumps(summary, indent=2) + '\n')
print('SUMMARY=' + json.dumps(summary, indent=1), flush=True)
