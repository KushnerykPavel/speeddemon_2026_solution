#!/usr/bin/env python3
"""Track D Experiment 0: proxy calibration with competitor binaries of known board time. Runs on the VPS.

usage: calib.py ROOT   (ROOT holds src/{ours,flyhigh,cacheme,hv}, measure.rs)
Protocol as 2026-11-01-traverse-thp-dense: pinned images, offline release builds (each team's own flags),
Docker CPU set 0-5, 8 GB cgroup, read-only sandbox without network, prewarmed input, rotating order,
round 0 discarded, trimmed mean. Every timed output is checked against the Python full-seed-1 answer.
"""
import hashlib, json, os, shutil, statistics, subprocess, sys
from pathlib import Path

root = Path(sys.argv[1]).resolve()
rust = 'rust:1-slim-trixie@sha256:1a939748a7cbab172de91a014e3804a97af1394a8c844faa67fce9401c4e40fb'
gcc = 'gcc:15@sha256:eb5e64286ad47187b32bda75c6b6916166ecb41da56559b8b09e0001748d2cce'
debian = 'debian:13-slim@sha256:62bd423aeee912639ca8ee681c2b762cb3e25137e0a92b20be455898a9f034ad'
datasets = Path('/home/student/speed-demon-2026/data/rust-validation/d')
data = datasets / 'full-seed1'
expected = Path('/home/student/speed-demon-2026/results/rust-baseline-full/python-d-full-seed1/distances.txt').read_bytes()
assert hashlib.sha256(expected).hexdigest().startswith('a23dbdf0')
(root/'cpu.txt').write_text(subprocess.check_output(['lscpu'], text=True))


def run(command, capture=False, env=None):
    print('COMMAND=' + ' '.join(map(str, command))[:300], flush=True)
    return subprocess.run(command, check=True, text=True, capture_output=capture)


for p in [root, *root.rglob('*')]:
    p.chmod(0o777)
uid = ['--user', f'{os.getuid()}:{os.getgid()}']
build = ['docker', 'run', '--rm', '--network=none', *uid, '-e', 'CARGO_HOME=/tmp/cargo', '-v', f'{root}:/work']
native = ['-e', 'RUSTFLAGS=-C target-cpu=native']
run(build + native + ['-w', '/work/src/ours', rust, 'cargo', 'build', '--release', '--offline', '--locked'])
run(build + native + ['-w', '/work/src/flyhigh', rust, 'cargo', 'build', '--release', '--offline'])
run(build + native + ['-w', '/work/src/cacheme', rust, 'cargo', 'build', '--release', '--offline'])
# HighVelocity's own Dockerfile flags (no -march=native).
run(build + ['-w', '/work/src/hv', gcc, 'g++', '-O3', '-std=c++17', '-pthread', '-flto', '-static', '-s',
             '-o', 'traverse', 'd_traverse.cpp'])
run(build + ['-w', '/work', rust, 'rustc', '--edition=2024', '-O', 'measure.rs', '-o', 'measure'])
(root/'compiler.txt').write_text(run(build + [rust, 'rustc', '-Vv'], True).stdout +
                                 run(build + [gcc, 'g++', '--version'], True).stdout)

BIN = {'ours': '/work/src/ours/target/release/traverse-baseline',
       'cacheme': '/work/src/cacheme/target/release/traverse',
       'flyhigh': '/work/src/flyhigh/target/release/solution',
       'hv': '/work/src/hv/traverse'}
(root/'binaries.json').write_text(json.dumps(
    {k: hashlib.sha256((root/v.removeprefix('/work/')).read_bytes()).hexdigest() for k, v in BIN.items()}, indent=1))

common = ['docker', 'run', '--rm', '--network=none', '--read-only', '--cpuset-cpus=0-5', '--memory=8g',
          '--memory-swap=8g', '--pids-limit=512', '--cap-drop=ALL', '--security-opt=no-new-privileges',
          '--user', '65534:65534', '--tmpfs', '/tmp:rw,size=1g', '-v', f'{root}:/work']


def invoke(name, workers, case, out):
    out.mkdir(parents=True, exist_ok=True)
    out.chmod(0o777)
    args = ['--threads', str(workers)] if name == 'ours' else []
    return common + ['-e', f'SD_THREADS={workers}', '-v', f'{case}:/data:ro', '-v', f'{out}:/out',
                     debian, '/work/measure', BIN[name], *args]


# Correctness on every validation dataset (recorded; the timed gate is full seed 1).
correct = {}
for case in sorted(datasets.iterdir()):
    for name in BIN:
        o = root/'cmp'/name/case.name
        try:
            run(invoke(name, 6, case, o), True)
            correct.setdefault(name, {})[case.name] = hashlib.sha256((o/'distances.txt').read_bytes()).hexdigest()[:12]
        except subprocess.CalledProcessError as e:
            correct.setdefault(name, {})[case.name] = f'FAIL {e.returncode}'
(root/'correctness.json').write_text(json.dumps(correct, indent=1))
print('CORRECT=' + json.dumps(correct), flush=True)


def warm():
    for path in data.iterdir():
        with path.open('rb') as stream:
            while stream.read(1 << 20):
                pass


ROUNDS = int(os.environ.get('ROUNDS', '11'))
names = list(BIN)
rows = []
with (root/'runs.jsonl').open('w') as log:
    for workers in [12, 6]:
        for rnd in range(ROUNDS):
            order = names[rnd % 4:] + names[:rnd % 4]
            if rnd % 2:
                order = order[::-1]
            for name in order:
                warm()
                o = root/'out'/f'{name}-{workers}'
                result = run(invoke(name, workers, data, o), True)
                assert (o/'distances.txt').read_bytes() == expected, name
                row = dict(json.loads(result.stdout), variant=name, workers=workers, round=rnd)
                rows.append(row)
                log.write(json.dumps(row) + '\n')
                log.flush()
                print(f'RUN {name} w={workers} r={rnd} {row["seconds"]:.4f}', flush=True)

board = {'ours': 0.1986,  # best-of-board basis for all four; ours mean of 21 runs is 0.2069
         'cacheme': 0.1677, 'flyhigh': 0.1507, 'hv': 0.1103}
summary = {'cpuset': '0-5', 'rounds_excluding_warmup': ROUNDS - 1, 'board_best_or_mean_s': board, 'results': {}}
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
                       'max_cgroup_peak_mb': max(r['cgroup_peak_bytes'] for r in sel) / 1e6}
    base = entry['ours']['trimmed_mean_s']
    entry['ratio_vs_ours_vps'] = {n: entry[n]['trimmed_mean_s']/base for n in names}
    entry['ratio_vs_ours_board'] = {n: board[n]/board['ours'] for n in names}
    summary['results'][str(workers)] = entry
(root/'summary.json').write_text(json.dumps(summary, indent=2) + '\n')
print('SUMMARY=' + json.dumps(summary, indent=1), flush=True)
