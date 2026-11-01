#!/bin/bash
# rl-lyceum: HEAD vs AES-fold branch-free endpoint-hash prototype (exploratory, not a candidate). Alternating, 3 rounds of perf stat -r 10.
cd ~/sd-review-profiler; R=~/speed-demon-track-a-Eh6ThXY0
for r in 1 2 3; do for v in cur proto; do
  for th in 1 8; do cs=2; [ $th = 8 ] && cs=0-7
  echo "=== $v threads=$th round $r"
  sudo -n perf stat -r 10 -e cycles:u,instructions:u,branch-misses:u,task-clock taskset -c $cs ./tgt-$v/release/ingest-baseline --data $R/data/full-seed1 --out out --threads $th 2>&1 >/dev/null | grep -E "cycles|instructions|branch-misses|task-clock|elapsed"
done; done; done
