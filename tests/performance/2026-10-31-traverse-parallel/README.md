# D7: dynamic parallel queries — local thread sweep

Baseline: `292002c` (D5 degree rejection + byte parser + serial CSR/array Dijkstra).
Candidate: `candidate.patch` against that baseline. Standard-library scoped
threads, immutable shared graph, private reusable search state, atomic next-query
index and indexed atomic answers. Scoped joins finish before results are collected;
relaxed atomics suffice because task claiming needs only unique indices, and
joins synchronize answer publication. `u64::MAX` marks unreachable, as in the
existing distance array. Output order follows query indices.

Default: available parallelism, at most 12 workers and at most one per query.
`--threads N` selects a positive worker limit, also capped at 12. One worker uses
a serial loop; empty input starts no worker and creates no distance array.
Distance-array payload per worker at full size: 13,924,096 bytes, or 167,089,152
bytes for 12 workers. Touched lists and heaps add variable memory. No measured
peak memory is claimed; worker-count tuning must account for it.

## Diagnostic results

macOS arm64, Rust 1.98.0. Shuffled and randomly relabelled bidirectional 300×300
grid, unit weights, seed 20261031: 90,000 vertices, 358,800 edges, 1,000 queries.
Local offsets have power-of-two radius caps 1 through 64; every seventeenth query
crosses toward the opposite part of the grid, creating uneven work. This is a
synthetic workload, not the official generator or the scoring machine.

Each experiment uses one warmup and five measurements per variant. Variant order
rotates each round. Discard min/max, average the middle three. Whole-process wall
times include parsing, CSR build, search-state initialization, queries and output.
All 72 outputs across both experiments match the preserved Python reference.

| Variant | First trimmed mean | Repeat trimmed mean |
|---|---:|---:|
| Previous serial binary | 296.58 ms | 266.87 ms |
| New, 1 worker | 339.84 ms | 285.14 ms |
| New, 2 workers | 166.61 ms | 144.29 ms |
| New, 4 workers | 114.07 ms | 95.05 ms |
| New, 6 workers | 70.22 ms | 68.86 ms |
| New, 12 workers | 68.74 ms | 56.30 ms |

Twelve workers gave 4.31x and 4.74x relative to the old serial binary; six gave
4.22x and 3.88x. The new serial path was 14.59% and 6.85% slower; no isolated
cause is established. Both complete raw experiments are preserved as
`summary.json` and `summary-repeat.json`, including compiler/platform and input
hashes. No default-worker performance claim is made (the default may differ from
these explicit counts on a laptop). The default cap of 12 follows the plan and
remains provisional: final selection requires Linux x86-64/full-workload time
and peak-memory measurements. No board run ID or measured points.

## Reproduction

From the repository root with the candidate applied:

```sh
git show 292002c:track-d/src/main.rs > /tmp/traverse-before-parallel.rs
rustc --edition=2024 -O -C lto=fat -C codegen-units=1 -C panic=abort /tmp/traverse-before-parallel.rs -o /tmp/traverse-before-parallel
cargo build --manifest-path track-d/Cargo.toml --release --offline --locked
python3 tests/performance/2026-10-31-traverse-parallel/runner.py \
  /tmp/traverse-before-parallel track-d/target/release/traverse-baseline
```

Seven Rust regression tests pass, including repeated ordered-result checks against
serial Dijkstra with varying query lengths, duplicates, over-u32 distances,
unreachable/self paths, empty input and fewer queries than workers. The external
Python checker matches 204 deterministic cases at default/1/2/4/6/12 workers
(1,224 output comparisons), plus eight invalid-input and three invalid-thread
checks. Release Clippy with warnings denied, release build and formatting pass.
