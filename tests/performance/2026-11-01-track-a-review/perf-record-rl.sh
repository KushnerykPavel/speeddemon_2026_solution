#!/bin/bash
# rl-lyceum: attribute cycles and branch-misses (user) to source lines; HEAD source built with haswell + debug=1 (line tables only), 1 thread, cpu 2
cd ~/sd-review-profiler; R=~/speed-demon-track-a-Eh6ThXY0
mkdir -p tgt-prof; sudo -n -H env RUSTC=/root/.cargo/bin/rustc RUSTFLAGS="-C target-cpu=haswell" /root/.cargo/bin/cargo build --release --offline --locked --manifest-path cur/Cargo.toml --target-dir tgt-prof --config profile.release.debug=1 2>&1 | tail -1
sudo chown -R $USER tgt-prof
cat > loop.sh <<'L'
for i in $(seq 20); do taskset -c 2 ./tgt-prof/release/ingest-baseline --data $HOME/speed-demon-track-a-Eh6ThXY0/data/full-seed1 --out out --threads 1; done
L
for ev in cycles:u branch-misses:u; do
  n=${ev%%:*}
  sudo -n perf record -e $ev -c $( [ $n = cycles ] && echo 200000 || echo 2000 ) -o perf-$n.data -- bash loop.sh >/dev/null 2>&1
  echo "##### $ev by symbol"; sudo -n perf report -i perf-$n.data --no-children --sort sym --stdio 2>/dev/null | grep -v '^#' | grep -v '^$' | head -12
  echo "##### $ev by srcline (top 45)"; sudo -n perf report -i perf-$n.data --no-children --sort srcline --stdio 2>/dev/null | grep -v '^#' | grep -v '^$' | head -45
done
