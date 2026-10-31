# D5: degree rejection — provisional local diagnostic

Baseline: `6d6732f` (byte parser, serial CSR/array Dijkstra).
Candidate: `candidate.patch`, applied to that baseline. No query parallelism or
routing index. Out-degree is read from CSR offsets; a `u64` bitset records incoming
edge presence while parsing. Extra bitset payload at full size: 217,568 bytes.
Same-node queries return zero before either rejection test. Targets with only a
self-loop still have incoming edges and use the normal search when needed.

The optimization strategy ranks D5 before D7 (parallel queries). Its intended
benefit is avoiding whole-graph traversals when the target has no incoming edge.
This remains exact for any legal directed graph; degree tests do not decide all
unreachable pairs.

## Results

macOS arm64, Rust 1.98.0. Synthetic bidirectional 300×300 grid, shuffled edges,
randomly relabelled IDs, seed 20261031, plus no-in/no-out/isolated vertices:
90,003 nodes, 358,802 edges. Unit edge weights, two added zero-weight edges.
Normal workload: 1,950 local queries with row/column displacements up to 16.
Mixed workload: those queries plus 50 special queries, including two targets
without incoming edges, no-out/isolated sources, and self queries.

Each workload/variant: one warmup then five alternating runs; remove minimum and
maximum and average the middle three. Times include parsing, CSR construction,
queries and output. Every measured output matches the Python reference byte for
byte. All timings are preserved in `summary.json` and `summary-repeat.json`.

| Workload | First before / after | Repeat before / after |
|---|---|---|
| Normal only | 75.19 / 84.96 ms | 69.70 / 70.90 ms |
| Mixed | 82.71 / 76.41 ms | 82.65 / 70.50 ms |

The mixed synthetic workload improved 7.62% and 14.70%; the normal-only control
slowed 13.00% and 1.72%. These short local measurements are noisy and provide
mechanism-level evidence only. Per the plan, performance decisions require
Linux x86-64/full-workload measurements. The candidate is provisional pending
that validation. No board run ID, measured peak memory or points available yet.

## Reproduction

From the repository root with the candidate source present:

```sh
git show 6d6732f:track-d/src/main.rs > /tmp/traverse-before-degree-rejection.rs
rustc --edition=2024 -O -C lto=fat -C codegen-units=1 -C panic=abort /tmp/traverse-before-degree-rejection.rs -o /tmp/traverse-before-degree-rejection
cargo build --manifest-path track-d/Cargo.toml --release --offline --locked
python3 tests/performance/2026-10-31-traverse-degree-rejection/runner.py \
  /tmp/traverse-before-degree-rejection track-d/target/release/traverse-baseline
```

Six regression tests pass, 204 deterministic fixtures/random cases match Python,
and eight invalid-input cases fail without panic. Commands are in `tests/README.md`.
Release Clippy with warnings denied and formatting checks also pass.
