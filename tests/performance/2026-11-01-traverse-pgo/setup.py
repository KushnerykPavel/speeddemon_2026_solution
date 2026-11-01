#!/usr/bin/env python3
"""Generate training data (full seeds 11, 12; sample seeds 3, 5) and build the merged PGO profile.

Seed 1 stays out of the training set so the A/B on it measures a held-out input.
"""
import os
from pathlib import Path
import subprocess
import sys

root = Path(sys.argv[1]).resolve()
root.chmod(0o777)
print('REMOTE_ARTIFACTS=' + str(root), flush=True)
rust = 'rust:1-slim-trixie@sha256:1a939748a7cbab172de91a014e3804a97af1394a8c844faa67fce9401c4e40fb'
python_image = 'python:3.13-slim@sha256:d98cc29d3266e535dd9da1099688d9cb3ca4753813571e071763d17725460962'
harness = Path('/home/student/speed-demon-2026/harness')


def run(command):
    print('COMMAND=' + repr(command), flush=True)
    subprocess.run(command, check=True)


train = [('full', 11), ('full', 12), ('sample', 3), ('sample', 5)]
for scale, seed in train:
    out = root / 'train' / f'{scale}-seed{seed}'
    if not (out / 'queries.txt').exists():
        run(['docker', 'run', '--rm', '--network=none', '--user', f'{os.getuid()}:{os.getgid()}',
             '-v', f'{harness}:/h:ro', '-v', f'{root}:/work', python_image, 'python3', '/h/generators/d_traverse.py',
             '--seed', str(seed), '--scale', scale, '--out', f'/work/train/{scale}-seed{seed}'])
script = r'''
set -e
rustup component add llvm-tools >/dev/null 2>&1 || rustup component add llvm-tools-preview
PROFDATA=$(find $(rustc --print sysroot) -name llvm-profdata | head -1)
echo PROFDATA=$PROFDATA
cd /work/src
rm -rf /work/pgo-raw /work/pgo-target
RUSTFLAGS="-C target-cpu=native -C profile-generate=/work/pgo-raw" cargo build --release --offline --locked --target-dir /work/pgo-target
for d in /work/train/*; do
  for t in 1 6 12; do
    /work/pgo-target/release/traverse-baseline --threads $t --data $d --out /work/pgo-out
  done
done
$PROFDATA merge -o /work/pgo.profdata /work/pgo-raw
ls -la /work/pgo.profdata
rustc -Vv > /work/pgo-rustc.txt
'''
run(['docker', 'run', '--rm', '-e', 'CARGO_HOME=/tmp/cargo', '-e', 'HOME=/tmp/home', '-e', 'RUSTUP_HOME=/usr/local/rustup',
     '-v', f'{root}:/work', rust, 'sh', '-c', 'mkdir -p /tmp/home && ' + script])
print('DONE=' + str(root), flush=True)
