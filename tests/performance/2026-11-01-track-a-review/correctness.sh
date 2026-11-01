#!/bin/bash
# VPS: reference (harness/reference/a_ingest.py, python3.14) vs committed HEAD binary (docker-built, bin-cur) at several thread counts.
cd ~/sd-review
for d in data/tiny-seed20261101 data/sample-seed20261101 data/sample-seed987654321 data/full-seed20261101 ~/speed-demon-2026/data/rust-validation/a/edge-cases ~/speed-demon-2026/data/rust-validation/a/empty; do
  n=$(basename $d); mkdir -p refout/$n; /usr/bin/time -f "ref $n %es" python3 refrun.py $d refout/$n
  r=$(sha256sum refout/$n/summary.txt | cut -d' ' -f1)
  for th in 1 2 3 6 12 16; do
    rm -rf o3; mkdir o3; ./bin-cur --data $d --out o3 --threads $th
    if cmp -s o3/summary.txt refout/$n/summary.txt; then echo "OK   $n threads=$th sha=${r:0:16} bytes=$(stat -c %s o3/summary.txt)"; else echo "FAIL $n threads=$th"; fi
  done
done
