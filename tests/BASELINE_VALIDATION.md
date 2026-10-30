# Rust baseline validation — 2026-10-29

Tracks A and D preserve the Python references' algorithms: hash-map grouping
and per-group sorting for A; adjacency lists and a fresh hash-map/binary-heap
Dijkstra search per query for D. Both Rust packages use only the standard
library. A extracts the hour from the guaranteed fixed-width UTC timestamp.
The unchanged Python sources remain under `tests/reference/`.

## Correctness

All comparisons passed, checking complete output bytes against Python 3.13:

| Cases | Track A | Track D |
|---|---|---|
| tiny, seeds 1, 7, 42 | PASS | PASS |
| sample, seeds 1, 7, 42 | PASS | PASS |
| full, seed 1 | PASS | PASS |
| Explicit edge cases | PASS | PASS |

Seed-1 sample/full output hashes also match the published specifications.
The five explicit cases cover empty log input, nearest-rank boundaries for
groups of 1–102 records, shuffled hours across the month boundary, duplicate
logs, byte ordering, a maximum-length endpoint, zero/uint32-max latencies,
error-rate truncation, zero-weight paths, parallel/duplicate edges,
self-loops, isolated nodes, unreachable queries, duplicate queries,
distances above uint32, a graph without edges, and an empty query file.
The standard generator covers the full endpoint catalogue and other injected
edge cases. The explicit fixtures are run in both suites: 17 comparisons in
the tiny/sample suite and 7 in the full suite, representing 19 distinct cases.

## Environment and reproduction

- Host: `my-vps`, Linux x86_64, 6 logical CPUs, approximately 12 GB RAM.
- Containers: 8 GB memory, no swap, no network during builds and execution;
  Rust execution uses the original `check.py` restrictions.
- Release builds: `cargo build --release --offline --locked`.
- Pinned allowlisted Rust/Debian images are in each track's Dockerfile;
  the pinned Python 3.13 reference image is in the verification script.
- Official generator checkout: `575a99c92b4f08fe9bc30c3b344d82d56ab44f0a`.
- The tested source snapshot is based on `72126ce`; exact file hashes are in
  [source-sha256.txt](validation/source-sha256.txt).
- Local `cargo check`, `cargo fmt --check` and `cargo clippy -- -D warnings`
  passed for both packages.

Commands run from `/home/student/speed-demon-2026/validation-rust-baseline`:

```sh
python3 scripts/verify-baselines.py \
  --harness ../harness --data-root ../data/rust-validation \
  --results ../results/rust-baseline-matrix

python3 scripts/verify-baselines.py \
  --harness ../harness --data-root ../data/rust-validation \
  --results ../results/rust-baseline-full --scales full --seeds 1
```

Machine-readable results:
[tiny/sample](validation/rust-baseline-matrix.json),
[full](validation/rust-baseline-full.json).
The corresponding VPS directories contain all per-case logs and Python
outputs. The VPS validation snapshot is separate from its Git checkout.

## Timing limits

These are single correctness runs, not controlled performance benchmarks.
Full seed-1 Rust wall times including Docker startup were 2.88 s for A and
21.32 s for D. No CPU affinity, repeated-run average or peak-memory comparison
was measured. They are not official board scores. No board run has been made
for these Rust baselines yet; future performance changes should be measured
separately against this baseline under consistent conditions.
