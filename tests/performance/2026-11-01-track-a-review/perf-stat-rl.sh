#!/bin/bash
# on rl-lyceum (i7-7700, 4C/8T, HW counters). binaries: tgt-{cur,prevgen,prev}/release/ingest-baseline
cd ~/sd-review-profiler; R=~/speed-demon-track-a-Eh6ThXY0
for th in 1 8; do for v in cur prevgen prev; do
 cs=0-7; [ $th = 1 ] && cs=2
 for ev in "cycles:u,cycles:k,instructions:u,instructions:k" "branches:u,branch-misses:u,cache-references:u,cache-misses:u" "task-clock,context-switches,page-faults,cpu-migrations"; do
  echo "=== $v threads=$th taskset=$cs events=$ev"
  sudo -n perf stat -r 10 -e $ev taskset -c $cs ./tgt-$v/release/ingest-baseline --data $R/data/full-seed1 --out out --threads $th 2>&1 >/dev/null | grep -v '^$' | grep -v Performance
 done; done; done
