# Track D: bidirectional Dijkstra with paged distances

Baseline: `974ec7e` (`before-main.rs`, forward search, dense `u32` distances).
Candidate: `bidir-main.rs` (scored source, paged distances). Rejected
intermediate: `bidir-dense-main.rs` (dense per-side arrays). `profile-*-main.rs`
add stderr phase timers (and a transpose timer) only.

## Change

- Reverse CSR built in parallel inside `answer_queries` (count in-degrees over
  disjoint source rows, prefix, scatter through atomic cursors). It replaces the
  incoming bitset: an empty reverse row means no incoming edge.
- Bidirectional Dijkstra: grow the side with the smaller queue; update `μ` on
  every relaxation into a vertex the other side has labelled; stop when
  `top_f + top_b ≥ μ` or a side is empty. Lazy heaps need no cleanup before the
  test: a stale top is still a lower bound.
- Narrow `u32` pass: relaxations that do not fit are skipped and flagged. A
  result below `u32::MAX` is exact (every prefix and suffix of a shorter path
  fits); otherwise, if anything was skipped, rerun in the shared `u64` search.
- Paged distances: per side, a block table (1,024 vertices per block) maps to
  4 KB pages taken from a pool on first write; reset refills used pages and
  returns them. Memory follows the largest query instead of the vertex count.

## Protocol

`my-vps`, six Haswell KVM vCPUs, CPU set 0-5, 8 GB cgroup, pinned Rust image,
offline native release builds. Full official seed 1, prewarmed inputs, one
warmup plus five rotating-order runs, trimmed mean of the middle three, cgroup
v2 `memory.peak` via `measure.rs`. Phase profile: four runs per variant and
worker count, medians. All outputs byte-equal to the saved Python reference
(SHA-256 `a23dbdf0…277e`). VPS results, not board measurements.

| Variant | Workers | T (s) | Spread | cgroup peak (MB) |
|---|---:|---:|---:|---:|
| `974ec7e` | 6 | 1.2398 | 13.8% | 113.1 |
| dense | 6 | 0.7713 | 17.7% | 223.4 |
| paged | 6 | 0.7652 | 5.8% | 128.8 |
| `974ec7e` | 12 | 1.2213 | 7.4% | 158.7 |
| dense | 12 | 0.8244 | 12.2% | 309.1 |
| paged | 12 | 0.7937 | 11.7% | 143.3 |

| Phase median (s) | `974ec7e` 6w | paged 6w | `974ec7e` 12w | paged 12w |
|---|---:|---:|---:|---:|
| graph load | 0.400 | 0.377 | 0.392 | 0.373 |
| renumbering | 0.107 | 0.111 | 0.109 | 0.096 |
| search (incl. transpose) | 0.689 | 0.300 | 0.721 | 0.306 |
| of which transpose | — | 0.043 | — | 0.045 |

`bidir-dense/` holds the first run (baseline vs dense; 1.195 → 0.802 s and
113.3 → 223.6 MB at six workers, 1.139 → 0.816 s and 158.6 → 309.0 MB at twelve),
kept for provenance.

## Correctness

Fourteen Rust tests (`measured-bidir-tests.rs`) pass on macOS and VPS. New or
changed: an independent `u64` reference Dijkstra compared on 60 random graphs
(all pairs, 1/3/12 workers, weights 0 / small / ~2^31 / ~2^32, IDs spread over
pages in odd rounds); reverse-graph multisets under cursor contention; reset
checks on both sides. Mutations caught: stop at first meeting (3 failing tests),
always accept the narrow answer (4), skip page clearing on reset (6).
`traverse-check.py`: 204 cases byte-equal to Python under default and
1/2/4/6/12 workers; invalid inputs and thread options fail cleanly. Release
Clippy and rustfmt pass.

Raw rows: `runs.jsonl`, `profiles.jsonl`; summaries: `summary.json`,
`profile-summary.json`; console: `console.txt`; hashes: `source-sha256.txt`.
