#!/bin/bash
# rl-lyceum: precise (PEBS, :pp) branch-misses and cycles attribution to source line in the 1-thread HEAD build (tgt-prof, debug=1).
cd ~/sd-review-profiler
for ev in branch-misses:upp cycles:upp; do
  n=${ev%%:*}
  sudo -n perf record -e $ev -c $( [ $n = cycles ] && echo 200000 || echo 2000 ) -o perfp-$n.data -- bash loop.sh >/dev/null 2>perfp-$n.err; tail -2 perfp-$n.err
  echo "##### $ev by srcline (top 30), --no-children"
  sudo -n perf report -i perfp-$n.data --no-children --sort srcline --stdio 2>/dev/null | grep -v '^#' | grep -v '^$' | head -30
done
