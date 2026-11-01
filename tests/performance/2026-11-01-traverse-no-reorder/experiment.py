#!/usr/bin/env python3
"""Track D: renumbering disabled (noreorder) vs 21df4fd (before). Runs on the VPS.

usage: experiment.py ROOT
"""
import hashlib
import json
import os
from pathlib import Path
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

COUNTER = ('static PUSHES: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);\n'
           'static SETTLED: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);\n')


def run(command, capture=False):
    print('COMMAND=' + ' '.join(map(str, command))[:300], flush=True)
    return subprocess.run(command, check=True, text=True, capture_output=capture)


def make_variant(name, source, counting=False):
    text = (root/source).read_text()
    if counting:
        text = COUNTER + text
        text = text.replace('        self.heap.push(Reverse(distance.key(node)));',
                            '        PUSHES.fetch_add(1, Relaxed);\n        self.heap.push(Reverse(distance.key(node)));', 1)
        text = text.replace('        let (distance, node) = D::unkey(key);\n        if distance > self.distance(node) {',
                            '        let (distance, node) = D::unkey(key);\n        SETTLED.fetch_add(1, Relaxed);\n        if distance > self.distance(node) {', 1)
        text = text.replace('    output.flush()?;\n    Ok(())',
                            '    output.flush()?;\n    eprintln!("COUNTS={{\\"pushes\\":{},\\"pops\\":{}}}", PUSHES.load(Relaxed), SETTLED.load(Relaxed));\n    Ok(())', 1)
        assert 'COUNTS=' in text and 'PUSHES.fetch_add' in text and 'SETTLED.fetch_add' in text
    d = root/name
    (d/'src').mkdir(parents=True, exist_ok=True)
    (d/'src'/'main.rs').write_text(text)
    for f in ['Cargo.toml', 'Cargo.lock']:
        shutil.copy(root/'cargo'/f, d/f)
    d.chmod(0o777)
    for p in d.rglob('*'):
        p.chmod(0o777)


variants_src = {'before': 'before-main.rs', 'noreorder': 'noreorder-main.rs'}
for name, src in variants_src.items():
    make_variant(name, src)

build = ['docker', 'run', '--rm', '--network=none', '--user', f'{os.getuid()}:{os.getgid()}',
         '-e', 'CARGO_HOME=/tmp/cargo', '-e', 'RUSTFLAGS=-C target-cpu=native', '-v', f'{root}:/work']
for name in ['before', 'noreorder']:
    run(build + ['-w', f'/work/{name}', rust, 'cargo', 'build', '--release', '--offline', '--locked'])
run(build + ['-w', '/work', rust, 'rustc', '--edition=2024', '-O', 'measure.rs', '-o', 'measure'])
(root/'compiler.txt').write_text(run(build + [rust, 'rustc', '-Vv'], True).stdout)

common = ['docker', 'run', '--rm', '--network=none', '--read-only', '--cpuset-cpus=0-5', '--memory=8g',
          '--memory-swap=8g', '--pids-limit=512', '--cap-drop=ALL', '--security-opt=no-new-privileges',
          '--user', '65534:65534', '--tmpfs', '/tmp:rw,size=1g', '-v', f'{root}:/work']

# Python fixtures/random cases (204) for both binaries.
for name in ['before', 'noreorder']:
    out = run(common + [python_image, 'python3', '/work/tests/traverse-check.py',
                        f'/work/{name}/target/release/traverse-baseline'], True).stdout
    (root/f'python-checks-{name}.txt').write_text(out)
    print(f'PYTHON {name}: {out.strip()}', flush=True)

# Byte-equality of both binaries on every validation dataset.
for case in sorted(datasets.iterdir()):
    outs = {}
    for name in ['before', 'noreorder']:
        o = root/f'cmp-{name}-{case.name}'
        o.mkdir(exist_ok=True)
        o.chmod(0o777)
        run(common + ['-v', f'{case}:/data:ro', debian, f'/work/{name}/target/release/traverse-baseline',
                      '--data', '/data', '--out', f'/work/cmp-{name}-{case.name}'])
        outs[name] = (o/'distances.txt').read_bytes()
    assert outs['before'] == outs['noreorder'], case.name
    print(f'EQUAL {case.name} {hashlib.sha256(outs["noreorder"]).hexdigest()[:12]}', flush=True)


def warm():
    for path in data.iterdir():
        with path.open('rb') as stream:
            while stream.read(1024*1024):
                pass


def measured(name, workers, out_label):
    warm()
    cmd = common + ['-v', f'{data}:/data:ro', debian, '/work/measure', f'/work/{name}/target/release/traverse-baseline',
                    '--threads', str(workers), '--data', '/data', '--out', f'/work/out-{out_label}']
    return run(cmd, True)


counts = {}

# Timing: alternating order, 12 and 6 workers, warmup round discarded.
ROUNDS = int(os.environ.get('ROUNDS', '9'))
names = ['before', 'noreorder']
rows = []
with (root/'runs.jsonl').open('w') as log:
    for workers in [12, 6, 1]:
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
summary = {'cpuset': '0-5', 'rounds_excluding_warmup': ROUNDS - 1, 'order': 'alternating',
           'method': 'round 0 discarded; trimmed mean (drop min/max) of remaining rounds',
           'output_sha256': hashlib.sha256(expected).hexdigest(), 'counts': counts, 'results': {}}
for workers in [12, 6, 1]:
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
    deltas = [100*(p['noreorder']/p['before']-1) for p in pairs.values()]
    entry['change_percent_trimmed_mean'] = 100*(entry['noreorder']['trimmed_mean_s']/entry['before']['trimmed_mean_s']-1)
    entry['paired_change_percent_median'] = statistics.median(deltas)
    entry['paired_noreorder_faster_rounds'] = f'{sum(d < 0 for d in deltas)}/{len(deltas)}'
    summary['results'][str(workers)] = entry
(root/'summary.json').write_text(json.dumps(summary, indent=2) + '\n')
print('SUMMARY=' + json.dumps(summary, indent=1), flush=True)
