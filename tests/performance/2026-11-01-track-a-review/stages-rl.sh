#!/bin/bash
# rl-lyceum native stage timing of the instrumented copy; output to /dev/shm; warm cache; alternating thread order
cd ~/sd-review-profiler; R=~/speed-demon-track-a-Eh6ThXY0; mkdir -p /dev/shm/sdrev-out
cat $R/data/full-seed1/requests.log >/dev/null
for r in $(seq 15); do
 for th in 1 2 4 8 12; do
  s=$( { /usr/bin/time -f "WALL %e" env SD_STAGES=1 ./tgt-inst/release/ingest-baseline --data $R/data/full-seed1 --out /dev/shm/sdrev-out --threads $th >/dev/null; } 2>&1 | tr '\n' ' ')
  echo "threads=$th $s"
 done
done
