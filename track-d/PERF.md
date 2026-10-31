# Track D: performance log

Every step from the reference to your final version, each with the live-board run ID that measured it. Judges check that the run IDs exist and that each change moved the way you say it did.

| Step | Change | Run ID | T (s) | M (MB) | Points |
|---|---|---|---|---|---|
| 0 | The reference, unchanged | | 72.69 | 893 | 0 |
| 1 | Rust baseline port; VPS correctness verified | | | | |
| 2 | CSR graph (`u32` offsets, packed edges) + reusable array distances with touched-list reset, replacing per-query HashMap | | | | |
| 3 | Release build: fat LTO, one codegen unit, abort on panic; native CPU in Docker build | | | | |
| 4 | Direct checked `u32` byte parser with reusable line buffers for graph and queries | | | | |
| 5 | D5: self-query shortcut and degree rejection using an incoming-edge bitset; provisional pending x86-64 measurement | | | | |
| 6 | D7: dynamically scheduled independent queries with private search state, up to 12 standard-library workers | | | | |

The Rust baseline preserves adjacency lists and one fresh Dijkstra search per
query, with a hash map and binary heap. No external dependencies.
VPS tiny/sample/full outputs match Python byte-for-byte. Validation evidence
is in `../tests/BASELINE_VALIDATION.md`. No live-board run ID or official
performance measurement is available for this step yet.

Step 2: local check only (not a live-board run). 200 random small graphs match the
Python reference byte-for-byte. On a synthetic 90k-node grid with 300 queries
(Mac), wall time fell from 6.3 s to 1.7 s with identical output. No run ID yet.

Step 3: build configuration only. Docker sets `-C target-cpu=native`; per the
documented board behavior, this requests a build on the scoring machine so CPU
features match the runtime. Local Cargo builds keep their default CPU target.
Local VPS full-seed1 comparison (same CSR source, CPU 2, warm page cache, one
warmup plus five alternating measurements per version): default release
18.340520 s versus tuned release 17.407554 s, trimmed means (5.09% less time).
All twelve outputs match the saved Python reference byte-for-byte. Baseline
max/min spread was 28.26%, so the improvement is inconclusive on the shared VPS;
settings are retained provisionally pending a scoring-machine measurement.
No memory/points measurement or live-board run ID. Reproduction and raw evidence:
`../tests/performance/2026-10-30-traverse-build-profile/README.md`.
Local validation: native-CPU release build, rustfmt and release Clippy passed;
all three existing D fixtures match Python byte-for-byte (edge cases, no edges,
no queries). VPS Docker builds and full-seed1 execution passed; the local Docker
daemon remains stopped.

Step 4: removed per-line `String` allocation, whitespace field vectors and
string-to-integer parsing. The graph header, edges and queries share a checked
ASCII decimal parser; CSR construction and Dijkstra are unchanged. Local Mac
synthetic loading-dominated run (100k vertices, 1m edges, 10k source=target
queries): median wall time 0.1700 s before versus 0.0447 s after, 73.71% less
time, after one warmup and five alternating measurements per variant. All
twelve outputs matched. This isolates loading and does not estimate full-track
speedup; no board run, Linux/full-dataset measurement, memory or points result.
Five parser regression tests passed, including every reader buffer boundary,
numeric limits, malformed records and read failures. 203 graph/query cases
matched Python byte-for-byte; eight invalid-input cases failed cleanly.
Release build, rustfmt and release Clippy passed. Reproduction and raw timings:
`../tests/performance/2026-10-31-traverse-byte-parser/README.md`.

Step 5: identical endpoints return zero first (including isolated vertices).
Different endpoints return unreachable without Dijkstra when the source has no
outgoing edges or the target has no incoming edges. Out-degree comes from CSR
offsets; incoming presence uses a `u64` bitset built during input parsing, adding
217,568 bytes at the documented full vertex count. This is a necessary-condition
filter: other unreachable queries still use ordinary Dijkstra.
Six regression tests passed; 204 deterministic cases matched Python byte-for-byte
and eight invalid inputs failed cleanly. Release build, rustfmt and Clippy passed.
Local Mac diagnostic on a shuffled/relabelled 90,003-node grid: mixed workload
(1,950 local queries, two no-in targets, no-out/isolated/self cases) trimmed means
82.71 → 76.41 ms in the first experiment and 82.65 → 70.50 ms on repeat (7.62%
and 14.70% less time). Normal-only control: 75.19 → 84.96 ms first, 69.70 →
70.90 ms on repeat. Short timings are noisy and the control does not show a
speedup; this does not establish a full-track or x86-64 improvement. Retained
provisionally pending the plan's x86-64/full-workload validation; no board run,
measured memory/points result for this step yet.
Raw runs, workload generator and exact source patch:
`../tests/performance/2026-10-31-traverse-degree-rejection/README.md`.

Step 6: scoped workers share the immutable CSR, claim query indices via a relaxed
atomic counter, and publish answers into indexed atomic slots. Scoped joins
complete before reading the answers in input order. Every worker owns its
distance/touched/heap state. Available parallelism is capped at 12 and the query
count; `--threads N` enables 1/2/4/6/12 comparisons. One worker uses a serial
path; empty input creates no search state. No new dependencies or unsafe code.
At full size each worker initializes 13,924,096 bytes of distances (167,089,152
bytes for 12), plus variable touched-list/heap memory; these are payload estimates,
not peak-memory measurements.
Seven regression tests pass; 204 cases match Python under default and each of
1/2/4/6/12 workers (1,224 output comparisons). Eight invalid-input cases and
three invalid thread options fail cleanly. Release build, Clippy and formatting
pass. Local Mac synthetic 90k-node shuffled/relabelled grid, 1,000 skewed queries:
trimmed means 296.58 → 68.74 ms (12 workers, 4.31x) and repeat 266.87 → 56.30 ms
(4.74x); six workers gave 70.22 and 68.86 ms (4.22x and 3.88x). The new serial
path took 339.84 and 285.14 ms, slower than the previous binary. All 72 benchmark
outputs match Python. These are local diagnostics; default worker choice remains
provisional pending scoring-machine time and cgroup memory measurements. No board
run ID or measured memory/points result yet. Raw runs and reproduction:
`../tests/performance/2026-10-31-traverse-parallel/README.md`.

Step 6 VPS validation: Linux x86-64, six Haswell KVM vCPUs, CPU set 0-5 for
every variant. Exact baseline `292002c` versus hashed D7 working-tree source,
same pinned Rust 1.99.1 image, offline release/native builds and runtime limits.
Official full seed 1, prewarmed input, one warmup plus five rotating-order
measurements; discard min/max and average the middle three. Serial baseline
12.9859 s; candidate 1/2/4/6/12 workers: 13.1116 / 7.3969 / 4.6269 / 3.7130 /
3.7406 s. Six workers give 3.50x speedup (71.41% less time). Maximum child RSS
over five measured runs: 110.61 MiB baseline, 110.73 / 110.76 / 110.69 / 132.86 /
216.91 MiB for 1/2/4/6/12 workers. RSS is not cgroup memory.peak; no points claim.
All 36 full outputs match the saved Python reference, SHA-256
`a23dbdf0a3e49cc8901310a9ee1dc6a9a569a85297b6da75c2a7002cb2ed277e`.
Seven Rust regression tests and all 1,224 Python comparisons also pass on VPS,
along with eight invalid-input and three invalid-thread checks. Run spreads
were 6.85% baseline and 3.58 / 4.74 / 6.77 / 4.55 / 5.44% by worker count;
shared-host noise exceeds the board's 5% stability limit in some variants.
Default selects six workers on this VPS. Twelve workers oversubscribe six CPUs;
this result does not choose between six and twelve on the competition machine.
Remote checkout was left unchanged; evidence and exact commands:
`../tests/performance/2026-10-31-traverse-parallel-vps/README.md`.
