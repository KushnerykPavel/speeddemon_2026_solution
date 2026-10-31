# Track D: parallel region renumbering

Baseline: `5bb06f2` (`before-main.rs`, serial BFS order). Candidate:
`regions-main.rs` (scored source). Rejected intermediate: `regions-grow-main.rs`
(growing region buffers). `profile-*-main.rs` add stderr phase timers only.

## Change

Each worker grows breadth-first regions from unclaimed roots in its own slice of
old IDs. A vertex is claimed by compare-and-swap of `worker << bits | position + 1`
(0 = free); a reader that loses the race uses the winner's provisional ID. Edges
are written with provisional heads during the walk. A parallel pass translates
provisional IDs (`region base + position`) and copies each region into disjoint
`split_at_mut` slices of the new CSR. Incoming bits and query endpoints are
remapped serially. Graphs with ≥ 2^27 vertices keep the input order (provisional
IDs must fit in `u32`). Idea credit: Team7's public Track D solution (MIT).

## Protocol

`my-vps`, six Haswell KVM vCPUs, CPU set 0-5, 8 GB cgroup, pinned Rust image,
offline native release builds. Full official seed 1, prewarmed inputs, one warmup
plus five rotating-order runs, trimmed mean of the middle three, cgroup v2
`memory.peak` via `measure.rs`. Phase profile: four runs per variant and worker
count, medians. All outputs byte-equal to the saved Python reference
(SHA-256 `a23dbdf0…277e`). VPS results, not board measurements.

| Variant | Workers | T (s) | Spread | cgroup peak (MB) |
|---|---:|---:|---:|---:|
| `5bb06f2` | 6 | 1.5628 | 10.5% | 119.8 |
| growing buffers | 6 | 1.2108 | 12.5% | 144.3 |
| reserved buffers | 6 | 1.1620 | 5.3% | 113.3 |
| `5bb06f2` | 12 | 1.5541 | 9.1% | 163.1 |
| growing buffers | 12 | 1.1593 | 9.2% | 173.3 |
| reserved buffers | 12 | 1.1876 | 15.9% | 158.6 |

| Phase median (s) | `5bb06f2` 6w | regions 6w | `5bb06f2` 12w | regions 12w |
|---|---:|---:|---:|---:|
| graph load | 0.387 | 0.423 | 0.373 | 0.379 |
| renumbering | 0.421 | 0.114 | 0.413 | 0.106 |
| search | 0.704 | 0.664 | 0.727 | 0.698 |

`first-attempt/` holds the earlier two-variant run (baseline vs growing buffers,
its profile used growing buffers too), kept for provenance; it showed the memory
regression that motivated reserving.

## Correctness

Thirteen Rust tests (`measured-regions-tests.rs`) pass on macOS and VPS,
including a 3,000-vertex random multi-component graph renumbered with 2/6/12
workers ten times each (permutation, remapped edge multiset, incoming bits, all
answers versus the original graph). A deliberately broken ID translation makes
both renumbering tests fail. `traverse-check.py`: 204 cases byte-equal to Python
under default and 1/2/4/6/12 workers; invalid inputs and thread options fail
cleanly. Release Clippy and rustfmt pass.

Raw rows: `runs.jsonl`, `profiles.jsonl`; summaries: `summary.json`,
`profile-summary.json`; console: `console.txt`; hashes: `source-sha256.txt`.
