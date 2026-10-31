# Track D: performance log

Every step from the reference to your final version, each with the live-board run ID that measured it. Judges check that the run IDs exist and that each change moved the way you say it did.

| Step | Change | Run ID | T (s) | M (MB) | Points |
|---|---|---|---|---|---|
| 0 | The reference, unchanged | — | 72.69 | 893 | 0 |
| 1 | Rust baseline port (`55522c4`) | run-7fd912d41bce | 12.531876 | 256.811008 | 22.057032 |
| 2–3 | CSR + array distances/touched reset; release LTO/codegen/panic and native CPU (joint submission) (`f70b655`) | Not in public history | 7.467422 | 115.048448 | 29.954002 |
| 4 | Direct checked uint32 byte parser (board time regression) (`6d6732f`) | Not in public history | 8.915943 | 114.974720 | 27.265802 |
| 5 | D5: self-query shortcut and degree rejection (`292002c`) | Not in public history | 6.927519 | 115.212288 | 29.716415 |
| 6 | D7: dynamically scheduled independent queries, up to 12 workers (`b8e5bc1`) | run-cffe0163d7c1 | 1.626762 | 228.868096 | 41.704470 |
| 8 | Read-only mmap + parallel two-pass CSR (`e5bad19`) | run-d07b2b8242d7 | 1.142983 | 228.335616 | 44.060351 |
| 9 | D8: BFS vertex order + parallel row-slice CSR rebuild (`accbb64`) | run-c18ebeae7015 | 0.637161 | 234.110976 | 49.692323 |
| 10 | D12: `u32` distances + shared exact `u64` fallback (`5bb06f2`) | run-e55665d62b35 | 0.587022 | 163.717120 | 51.755970 |
| 11 | Parallel claimed-region renumbering (`974ec7e`) | run-f1a2f8d1a891 | 0.473104 | 160.182272 | 53.841416 |
| 12 | Bidirectional Dijkstra + paged `u32` distances (`b938156`) | run-9ace8f534287 | 0.309259 | 159.694848 | 58.000036 |
| 13 | Single-pass bucketed graph build (`853ae88`) | run-07992c342c3a | 0.238915 | 141.635584 | 61.108794 |
| 14 | Packed 32-bit edges + exact heavy-weight list (`1767ec0`) | run-f3b44d092f5b | 0.242579 | 132.395008 | 60.976955 |
| 15 | 4 KB pages: `prctl(PR_SET_THP_DISABLE)` (`a37144d`) | run-b5ad6625b248 | 0.253390 | 101.629952 | 61.599011 |
| 16 | Single-`u64` heap keys for the `u32` search (`b9a93cd`) | run-3a1c4950924c | 0.221692 | 101.761024 | 62.975003 |
| 17 | Software prefetch: top-of-heap row and BFS rows ahead (`7d0e8ab`) | run-79245fc6bd21 | 0.205766 | 101.924864 | 63.676026 |
| 18 | SWAR record parsing (`d6954e2`) | run-39656346ac0a | 0.210076 | 101.093376 | 63.332170 |
| 19 | Memory diet: 8-byte buckets, order-then-copy renumbering, `mallopt` (`3b648eb`) | run-b507a889127d | 0.195022 | 79.945728 | **64.951278** |
| 20 | Code-size pass, 866 → 713 code lines (`1851b13`) | run-49484fa644ae | 0.211929 | 79.613952 | 64.692701 |
| 20 (re-sample) | Same track code, record-only commit (`2012f4e`) | run-abe73b97cc7d | 0.203360 | 79.261696 | **65.160080** |
| 21 | `Graph::row` back on `enumerate()`, Dial rejected (`21df4fd`) | run-711cf3f5f6f1 | 0.206146 | 79.745024 | 64.981435 |
| 23 | Chained BFS blobs of 512 (`cae88c9`) | run-81554f49a3d1 | 0.198285 | 79.855616 | 65.339497 |

Initial audit feed generated at **2026-10-31 15:19:48 UTC (16:19:48 Europe/Warsaw)**; history
snapshot generated at 15:19:40 UTC. All 18 A/D attempts in that snapshot passed.
Rows 8 and 9 were appended from later official feeds; their evidence is linked below.
The table uses the first board measurement of each implementation, not a VPS
or synthetic result. MB means decimal 1,000,000 bytes. These are single live-board
runs; they do not isolate noise or replace final holdout scoring.
`Not in public history` means the result is verified by commit/time, but its Run
ID is omitted from the public compact history. IDs are recorded only when found
in a saved or current full feed; none are inferred.
Raw evidence, all 18 attempts (including unchanged-track reruns), sources and
provenance: [board results](../tests/performance/2026-10-31-board-results/README.md).

Steps 2 and 3 were first submitted together at `f70b655`; no separate board run
exists to attribute their effects individually. The combined submission reduced
time 40.41%, memory 55.20%, and added 7.896970 points versus the Rust baseline.
Step 4 increased time 19.40% and lost 2.688200 points compared with that first
combined run (the unchanged-source rerun at `531def7` took 7.804536 s).
Step 5 reduced time 22.30% and added 2.450613 points versus step 4.
Step 6 reduced time **76.52% (4.26x)** and added **11.988055 points** versus step 5,
while measured memory rose 98.65%. It was the best D run before step 8:
**1.626762 s / 228.868096 MB / 41.704470 points**, `run-cffe0163d7c1`.

The Rust baseline preserves adjacency lists and one fresh Dijkstra search per
query, with a hash map and binary heap. No external dependencies.
VPS tiny/sample/full outputs match Python byte-for-byte. Validation evidence
is in `../tests/BASELINE_VALIDATION.md`. The official baseline measurement and
its saved Run ID are now recorded in the table above.

Step 2, initial local check (not a live-board run): 200 random small graphs match the
Python reference byte-for-byte. On a synthetic 90k-node grid with 300 queries
(Mac), wall time fell from 6.3 s to 1.7 s with identical output. The subsequent
board submission measured steps 2 and 3 together, as shown above.

Step 3: build configuration only. Docker sets `-C target-cpu=native`; per the
documented board behavior, this requests a build on the scoring machine so CPU
features match the runtime. Local Cargo builds keep their default CPU target.
Local VPS full-seed1 comparison (same CSR source, CPU 2, warm page cache, one
warmup plus five alternating measurements per version): default release
18.340520 s versus tuned release 17.407554 s, trimmed means (5.09% less time).
All twelve outputs match the saved Python reference byte-for-byte. Baseline
max/min spread was 28.26%, so the improvement is inconclusive on the shared VPS.
The profile alone remains unisolated on the board; the joint CSR/profile result
is recorded above. This VPS experiment did not measure memory or points.
Reproduction and raw evidence:
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
speedup. That local experiment did not measure memory or points. The later board
result above was slower than the previous implementation.
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
speedup; this does not establish a full-track or x86-64 improvement by itself.
The subsequent board result is recorded above and shows a time improvement.
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
outputs match Python. These are local diagnostics; the later board time, memory,
points and Run ID are in the table above. Controlled six-vs-twelve-worker tuning
on the scoring machine remains unmeasured. Raw local runs and reproduction:
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


## Step 7 experiment — phase profiling and two-pass CSR (rejected)

Compared submitted D7 (`b8e5bc1`) with an isolated candidate that replaces only
`read_graph`: first parse/count, rewind the same reader, parse/scatter directly
into CSR. Same checked byte parser, edge order, Dijkstra and query scheduling.
It removes the 60,000,000-byte temporary triple payload at full scale. This
candidate was not adopted into the scored source or submitted to the board.

Linux x86-64 VPS, six Haswell KVM vCPUs, CPU set 0-5, identical native/offline
release builds in the pinned Rust 1.99.1 image, same runtime limits as step 6.
Full official seed 1, warm input. Per variant: one warmup, five rotating-order
runs, remove min/max and average middle three. These are VPS results, not board
measurements; RSS below is child `wait4` peak, not cgroup memory.peak.

| Workers | Baseline T (s) | Two-pass T (s) | Change | Baseline peak RSS (MiB) | Two-pass peak RSS (MiB) |
|---|---:|---:|---:|---:|---:|
| 1 | 12.856764 | 13.414020 | +4.33% | 110.73 | 60.96 |
| 6 | 3.755675 | 4.154059 | +10.61% | 132.58 | 132.46 |
| 12 | 3.630838 | 4.285671 | +18.04% | 217.51 | 217.72 |

Baseline six-worker spread was 10.47%, so repeated that comparison on the same
binaries (one warmup and five alternating runs, same trimmed-mean calculation):
3.680718 → 4.124607 s (+12.06%); RSS 132.86 → 132.71 MiB. Repeat spreads remained
7.01%/5.96%, so precise slowdown is not established on the shared host. Both
batches show a slowdown and no parallel peak-memory saving. One worker saves
44.95% RSS, but the current parallel mode peaks in graph-plus-worker search state.

Separate diagnostic builds add phase timers outside the scored track. Three
alternating runs per variant at six workers; medians of each phase:

| Phase | Baseline (s) | Two-pass (s) |
|---|---:|---:|
| Graph load/build | 1.575736 | 2.143336 |
| Parse queries | 0.000311 | 0.000189 |
| Search, worker state and joins | 1.991407 | 1.980895 |
| Format/flush output | 0.000121 | 0.000109 |

Graph first parse/count: 0.870965 → 0.778087 s; prefix/allocation:
0.006498 → 0.006355 s; scatter (including the candidate second parse):
0.731631 → 1.325531 s. Outer graph phase rises 36.02%; search is approximately
unchanged. Individual medians need not sum to the median outer phase. Baseline
phase-time shares are about 44% graph and 56% search; query parsing/output are
negligible. Timed normal binaries contain no profiling instrumentation.

All 54 full outputs (36 normal, six diagnostic, twelve repeat) match the saved
Python reference. The candidate passes eight Rust regression tests on VPS and
1,224 Python fixture/random comparisons, eight malformed-input checks and three
invalid thread options. A new direct loader test verifies offsets, all edges and
their within-source order, incoming presence, u32 limits, CRLF/EOF and zero-edge
graphs. It passes on the baseline too. Candidate release Clippy and formatting
passed locally. No dependencies or unsafe code were added to the candidate.

Reject this serial buffered implementation; keep submitted D7 as the scored
source. Next loading experiments can test mmap/parallel parsing independently;
D8 remains the next search-layout experiment. Exact snapshots, patches, profiling
generator, remote scripts, all logs and source hashes:
[phase/CSR evidence](../tests/performance/2026-10-31-traverse-two-pass-csr/README.md).


## Step 8 — mmap and parallel CSR (implemented; VPS measurement)

Graph input is read-only MAP_PRIVATE, divided into newline-aligned slices.
Scoped std workers parse directly from borrowed bytes. First pass validates
records and counts degrees/incoming presence with relaxed atomics; a prefix
allocates CSR. Second pass scatters into uniquely assigned atomic cursor slots.
Every edge is retained; within-source order may vary without changing shortest
paths. The 60 MB temporary triple array is removed. Graph mapping is released
before query processing, including loader error paths. Queries retain their small
buffer and share the same checked u32 field parser. Loader and query worker limits
both use available parallelism capped at 12, overridable via --threads.

Std-only 64-bit Unix mmap/munmap wrapper, no new dependencies. Unsafe writes are
limited to unique in-bounds edge slots: the allocation is stable, each record
belongs to one chunk, atomic cursors assign unique positions, and scoped joins
finish before any edge is read. Input is immutable under the read-only data mount.

Compared against submitted D7 b8e5bc1 on the same six-vCPU Linux x86-64 VPS,
CPU set 0-5, same pinned native/offline release toolchain and runtime constraints.
Each variant has one warmup and five rotating-order measurements, then min/max
are discarded and the middle three averaged. Whole-container v2 memory.peak is
now collected by an external wrapper after child exit (fresh 8 GB cgroup per run,
inputs prewarmed outside the cgroup); RSS is also saved and includes mapped input
pages, so it is not substituted for the cgroup metric. No board score inferred.

| Workers | D7 T (s) | Mmap/parallel T (s) | Time change | D7 cgroup peak (MB) | Mmap/parallel peak (MB) |
|---|---:|---:|---:|---:|---:|
| 1 | 12.794612 | 13.718822 | +7.22% | 115.802112 | 69.505024 |
| 6 | 3.738694 | 2.403746 | -35.71% | 139.251712 | 140.656640 |
| 12 | 3.708020 | 2.377229 | -35.89% | 228.446208 | 228.941824 |

Candidate six-worker spread was 6.62%, prompting one repeat on the exact same
binaries: **3.767454 → 2.380925 s (-36.80%, 1.58x)**; maximum cgroup peak
139.730944 → 140.783616 MB (+0.75%). Spreads were 2.31% / 7.40%, so exact speedup
remains uncertain on this shared VPS. Both batches show the same large time
improvement. One worker is slower but saves about 40% cgroup peak; twelve workers
oversubscribe this six-CPU VM, so this does not choose six vs twelve for the board.

Separate diagnostic builds, three alternating runs each at six workers, medians:

| Phase | D7 (s) | Mmap/parallel (s) |
|---|---:|---:|
| Graph load/build | 1.658454 | 0.378113 |
| Parse queries | 0.000206 | 0.000199 |
| Search incl. state/joins | 1.983557 | 1.961185 |
| Format/flush output | 0.000182 | 0.000116 |

Graph subphases: first parse/count 0.938488 → 0.140717 s; prefix/allocation
0.007990 → 0.016769 s; scatter (with second parse after) 0.754754 → 0.214049 s.
Graph loading/building is 4.39x faster by diagnostic medians. Search now accounts
for about 84% of phase time, making D8 the larger remaining target. Individual
phase medians need not sum to the outer/process median; normal timings use
uninstrumented binaries.

All 54 full outputs (36 normal/warmups, six diagnostics, twelve repeat) match
Python byte-for-byte. Ten Rust tests and 1,224 Python random/fixture comparisons
pass on macOS and Linux, along with eight invalid-input and three invalid-thread
checks. New tests verify chunk coverage, retained edge multisets and repeated
20,000-edge contention on three source rows. Release Clippy, build and formatting
pass. The candidate is retained in the scored source; its board result follows below.
Full sources, patches, preparation/remote scripts, all raw measurements, normalized
memory labels, compiler/CPU records and source hashes:
[mmap/parallel evidence](../tests/performance/2026-10-31-traverse-mmap-parallel/README.md).


Step 8 official board result, feed 2026-10-31 16:22:57 UTC (17:22:57 Warsaw):
`run-d07b2b8242d7`, commit `e5bad19`, all gates passed. **1.142983 s,
228.335616 MB, S=363, D=1, K=7.606888, 44.060351 points**. New team best;
29.74% less time (1.4233x) and +2.355881 points versus D7. Memory is essentially
unchanged; source increased from 230 to 363 measured code lines. Derived Track D
position in this snapshot: 66 of 84 scored teams. Step 7 was rejected and has no
board row; step 8 is the implementation first measured here. This is a single
live-board run, not final holdout scoring. Evidence:
[official mmap result](../tests/performance/2026-10-31-board-mmap-results/README.md).


## Step 9 — D8 BFS vertex order with parallel CSR reconstruction

DFS and BFS discovery orders were compared against submitted mmap/parallel CSR
`e5bad19`. Both cover every vertex via a traversal forest, including isolated
vertices and regions not reached from earlier roots. A bijection translates
old IDs to discovery IDs. CSR rows, targets, incoming presence and query endpoints
are remapped; every edge/weight/duplicate and query order are retained. Dijkstra
and its scheduling are unchanged. Empty query input skips all preprocessing.

Initial complete-process comparison on six-vCPU Linux x86-64 VPS, CPU set 0-5,
identical pinned Rust/native/offline release builds and existing runtime limits.
Inputs prewarmed outside fresh cgroups. Per variant: one warmup and five rotating
measurements; discard min/max and average middle three. Cgroup memory.peak below
is whole-container peak, maximum of five measured runs, decimal MB; no board
points inferred. Twelve workers oversubscribe six vCPUs.

| Layout | Workers | Complete T (s) | Max cgroup peak (MB) |
|---|---:|---:|---:|
| Original | 6 | 2.419476 | 140.849152 |
| DFS | 6 | 2.003979 | 148.889600 |
| BFS, serial rebuild | 6 | 1.891317 | 147.537920 |
| Original | 12 | 2.462673 | 228.917248 |
| DFS | 12 | 2.069629 | 235.229184 |
| BFS, serial rebuild | 12 | 1.986810 | 233.791488 |

BFS gave the better full time (21.83% less at six workers versus DFS's 17.17%).
Its serial preprocessing cost was 0.706970 s by diagnostic medians, so refined
it with parallel row reconstruction: compute new row offsets, split the new edge
Vec into disjoint mutable row slices, copy/remap rows on scoped workers. BFS
discovery remains serial. This adds no unsafe code or dependency. Old graph,
order and permutation are released before queries. Reconstruction honors the
same --threads cap as loading and query work.

Refinement reran the controls and candidate on unchanged baseline binaries:

| Layout | Workers | Complete T (s) | Max cgroup peak (MB) |
|---|---:|---:|---:|
| Original | 6 | 2.276060 | 141.078528 |
| BFS, serial rebuild | 6 | 1.947441 | 147.718144 |
| BFS, parallel rebuild | 6 | 1.597091 | 147.447808 |
| Original | 12 | 2.343141 | 229.167104 |
| BFS, parallel rebuild | 12 | 1.626069 | 233.906176 |

Selected BFS+parallel rebuild reduces six-worker time 29.83%, increasing peak
memory 4.51%; twelve-worker time falls 30.60% with 2.07% more peak memory on this
VM. Candidate six-worker spread was 6.68%, triggering one confirmation, same
binaries and limits, one warmup and five alternating runs per version:
**2.347075 → 1.630284 s (-30.54%, 1.44x)**, peak **140.779520 → 147.820544 MB**
(+5.00%). Spreads remained 5.00%/7.38%; exact speedup is uncertain on the shared
host, but both batches show the same large net gain including preprocessing.
Initial per-variant spreads and all individual runs are retained in the evidence.

Separate phase-build medians at six workers (three runs each): original search
2.012080 s versus selected 0.837218 s (2.40x); selected BFS/permutation, CSR rebuild,
query translation and cleanup 0.424362 s. Graph load is 0.366568 s in the baseline
phase batch and 0.376768 s in the selected batch. Serial BFS preprocessing was
0.706970 s and DFS 0.836939 s. These are diagnostic builds/batches, not substituted
for normal timings, and phase medians need not sum to whole-process medians.

All 90 full outputs match the Python reference: 36 initial normal/warmups, nine
initial diagnostics, 30 refinement normal/warmups, three selected diagnostics,
and twelve confirmation runs. The selected candidate also matches Python and
the preserved baseline output hashes on nine official-generator/edge datasets:
tiny/sample seeds 1/7/42, edge-cases, no-edges and no-queries. Input/output hashes
are recorded. Each prototype and the selected candidate pass eleven Rust tests
and 1,224 Python fixture/random comparisons plus invalid-input/thread checks.
The new permutation test checks every ordered pair on a disconnected 130-node
graph, remapped edge multisets, incoming bits, self paths, zero/max weights and
sums above u32. Local release build, Clippy and formatting pass.

Decision: retain BFS order and parallel row-slice reconstruction. DFS and serial
BFS remain experiment snapshots. The subsequent official board result follows below.
Exact sources, scripts, test/manifest snapshots, all logs, source hashes and
multi-seed checks: [D8 evidence](../tests/performance/2026-10-31-traverse-reorder/README.md).


Step 9 official board result, feed 2026-10-31 18:03:56 UTC (19:03:56 Warsaw):
`run-c18ebeae7015`, commit `accbb64`, all gates passed. **0.637161 s,
234.110976 MB, S=437, D=1, K=9.859309, 49.692323 points**. New team best;
44.25% less time (1.7939x) and +5.631972 points versus step 8, with 2.53% more
memory. Derived Track D position: 64 of 88 scored teams. Leader in this snapshot:
Cause Of Death, 73.008704 points / 0.121536 s. This is a single live-board run,
not final hidden-seed scoring. Evidence:
[official D8 result](../tests/performance/2026-10-31-board-reorder-results/README.md).


## Step 10 — D12: `u32` distances with shared exact `u64` fallback (VPS measurement)

Workers search with `u32` distances (7 MB instead of 14 MB per worker) and 8-byte
heap entries. A relaxation that does not fit below the `u32::MAX` sentinel is
skipped and flags the search; a found distance is exact because every prefix of
a shorter path also fits. A missed target after a skip reruns in `u64`.
First attempt: a lazily allocated `u64` search per worker. On full seed 1, at
least the 4 of 2,000 queries whose answers exceed `u32` retry in `u64`, plus
unreachable queries whose narrow pass overflowed. Peak arithmetic implies about
six (6 workers) and seven (12 workers) 14 MB fallback arrays, so VPS cgroup peak
rose 28.4% / 5.4%. Kept version: one lock-protected `u64` search shared by all
workers.

Same VPS protocol as step 9 (CPU set 0-5, one warmup + five rotating runs,
trimmed mean of middle three, cgroup v2 `memory.peak`), baseline `accbb64`:

| Workers | `accbb64` T (s) | D12 T (s) | ΔT | `accbb64` peak (MB) | D12 peak (MB) | ΔM |
|---|---:|---:|---:|---:|---:|---:|
| 6 | 1.6469 | 1.5429 | −6.3% | 147.5 | 119.7 | −18.8% |
| 12 | 1.6163 | 1.5737 | −2.6% | 234.0 | 163.2 | −30.3% |

Time change is within run spread (8–18%); the memory cut matches −84 MB of
distance arrays plus one shared 14 MB array at twelve workers. All 36 outputs
match Python; twelve Rust tests (new fallback test at the `u32::MAX` boundary)
and 1,224 Python comparisons pass on macOS and VPS. Board result follows below.
Evidence: [D12 evidence](../tests/performance/2026-10-31-traverse-u32-distances/README.md).

Step 10 official board result, feed 2026-10-31 18:28:53 UTC (19:28:53 Warsaw):
`run-e55665d62b35`, commit `5bb06f2`, all gates passed. **0.587022 s,
163.717120 MB, S=491, D=1, K=10.842249, 51.755970 points**. New team best;
versus step 9: memory −30.07% (234.11 → 163.72 MB), time −7.87% (single runs;
within board noise), +2.063647 points despite S 437 → 491. Derived Track D
position: 54 of 88 scored teams. Single live-board run, not final hidden-seed
scoring. Evidence: [official D12 result](../tests/performance/2026-10-31-board-u32-results/README.md).


## Step 11 — parallel region renumbering (VPS measurement)

Step 9's serial BFS order took about 0.42 s on the VPS, as long as half the
search phase. Now every worker grows breadth-first regions from unclaimed roots
in its own slice of old IDs and claims vertices with a compare-and-swap of a
provisional ID (`worker << bits | position + 1`, 0 = free). Edges are written
with provisional heads during the walk, so the old graph is read once; a parallel
pass concatenates regions into the new CSR via disjoint `split_at_mut` slices
(no new unsafe). Graphs with ≥ 2^27 vertices keep the input order. Idea credit:
Team7's public Track D renumbering (MIT); implementation is ours.

First attempt let region buffers grow by doubling: time fell, but cgroup peak
rose 20.4% (6 workers) / 6.3% (12) from reallocation copies and slack. Kept
version reserves the whole graph per worker; untouched reserved pages are not
charged and nothing reallocates.

Same VPS protocol as step 10, baseline `5bb06f2`:

| Variant | Workers | T (s) | ΔT | cgroup peak (MB) | ΔM |
|---|---:|---:|---:|---:|---:|
| `5bb06f2` serial BFS | 6 | 1.5628 | — | 119.8 | — |
| growing buffers | 6 | 1.2108 | −22.5% | 144.3 | +20.4% |
| **reserved buffers** | 6 | **1.1620** | **−25.6%** | **113.3** | **−5.5%** |
| `5bb06f2` serial BFS | 12 | 1.5541 | — | 163.1 | — |
| growing buffers | 12 | 1.1593 | −25.4% | 173.3 | +6.3% |
| **reserved buffers** | 12 | **1.1876** | **−23.6%** | **158.6** | **−2.7%** |

Phase medians (separate instrumented builds, four runs each): renumbering
0.421 → 0.114 s (6 workers) and 0.413 → 0.106 s (12); search 0.704 → 0.664 s and
0.727 → 0.698 s, so region order keeps the locality of the serial BFS order.
All outputs match Python; thirteen Rust tests (new: 3,000-vertex random graph,
2/6/12 workers, ten repetitions, checking permutation, edge multiset, incoming
bits and answers) and 1,224 Python comparisons pass on macOS and VPS. A mutated
ID translation fails both renumbering tests. Board result follows below.
Evidence: [parallel renumbering](../tests/performance/2026-10-31-traverse-parallel-reorder/README.md).

Step 11 official board result, feed 2026-10-31 18:54:45 UTC (19:54:45 Warsaw):
`run-f1a2f8d1a891`, commit `974ec7e`, all gates passed. **0.473104 s,
160.182272 MB, S=547, D=1, K=11.935162, 53.841416 points**. New team best;
versus step 10: time −19.41% (1.2408x), memory −2.16%, +2.085446 points despite
S 491 → 547. Derived Track D position: 53 of 88 scored teams. Single live-board
run, not final hidden-seed scoring. Evidence:
[official renumbering result](../tests/performance/2026-10-31-board-regions-results/README.md).


## Step 12 — bidirectional Dijkstra with paged distances (VPS measurement)

After step 11 the search phase was ~0.69 s of ~1.2 s on the VPS. Queries now
run bidirectional Dijkstra over the forward CSR and a reverse CSR built in
parallel (two passes with atomic cursors, ~0.045 s). Each step grows the side
with the smaller queue (choosing by smaller key lets one side flood the graph
when the other side's first edge weighs close to 2^32). `μ` is updated on every relaxation that reaches a
vertex labelled by the other side; stop when `top_f + top_b ≥ μ` or a side is
empty. Stale heap tops are valid lower bounds, so no cleanup is needed before
the test. The `u32` narrow pass stays exact for answers below `u32::MAX`
(all prefixes and suffixes fit); otherwise an overflowed query reruns in the
shared `u64` search. The incoming bitset is gone: an empty reverse row means no
incoming edge, which also removes one atomic `fetch_or` per edge from parsing.

First attempt kept dense per-worker distance arrays: one per side, so 14 MB per
worker plus the 47 MB reverse graph, and cgroup peak almost doubled. Kept
version stores distances in 1,024-vertex (4 KB) pages assigned on first write
and recycled after each query; worker memory follows the largest search.

Same VPS protocol as step 11, baseline `974ec7e`:

| Variant | Workers | T (s) | ΔT | cgroup peak (MB) | ΔM |
|---|---:|---:|---:|---:|---:|
| `974ec7e` forward search | 6 | 1.2398 | — | 113.1 | — |
| bidirectional, dense | 6 | 0.7713 | −37.8% | 223.4 | +97.5% |
| **bidirectional, paged** | 6 | **0.7652** | **−38.3%** | **128.8** | **+13.8%** |
| `974ec7e` forward search | 12 | 1.2213 | — | 158.7 | — |
| bidirectional, dense | 12 | 0.8244 | −32.5% | 309.1 | +94.8% |
| **bidirectional, paged** | 12 | **0.7937** | **−35.0%** | **143.3** | **−9.7%** |

Phase medians: search including the reverse graph 0.689 → 0.300 s (6 workers)
and 0.721 → 0.306 s (12), of which transpose 0.043–0.045 s; paging costs no
measurable search time versus dense arrays (0.328 s / 0.344 s in the first run).
Graph loading (~0.37 s) is now the largest phase.

All outputs match Python. Fourteen Rust tests pass on macOS and VPS, including
a new comparison against an independent `u64` reference Dijkstra on 60 random
graphs (all pairs, 1/3/12 workers, weights 0 / small / ~2^31 / ~2^32 so sums
cross `u32`, vertex IDs spread over many pages in odd rounds) and a reverse-graph
test under cursor contention. Mutations caught: stopping at the first meeting
(3 tests fail), always accepting the narrow answer (4), not clearing pages on
reset (6). 1,224 Python comparisons pass. Board result follows below.
Evidence: [bidirectional search](../tests/performance/2026-10-31-traverse-bidirectional/README.md).

Step 12 official board result, feed 2026-10-31 19:37:32 UTC (20:37:32 Warsaw):
`run-9ace8f534287`, commit `b938156`, all gates passed. **0.309259 s,
159.694848 MB, S=632, D=1, K=14.454421, 58.000036 points**. New team best;
versus step 11: time −34.63% (1.5298x), memory −0.30%, +4.158620 points despite
S 547 → 632. Board memory did not fall as on the 12-worker VPS run (143.3 MB);
the VPS has six vCPUs, so the board's 12 workers likely run more searches at
once. Derived Track D position: 51 of 90 scored teams. Single live-board run,
not final hidden-seed scoring. Evidence:
[official bidirectional result](../tests/performance/2026-10-31-board-bidirectional-results/README.md).


## Step 13 — single-pass bucketed graph build (VPS measurement)

Phase profile before this step put graph loading at ~0.36 s of ~0.80 s on the
VPS, the largest phase. Loading now parses the file once: chunk workers bucket
each edge by its source's part (power-of-two ID block, about eight per worker),
then each part counts and places its own edges into disjoint `split_at_mut`
slices of the CSR, a cache-sized region with plain writes. No atomics or unsafe
remain in loading; within-row edge order is deterministic. Idea credit: Team7's
public Track D build (MIT).

Same VPS protocol as step 12, baseline `b938156`:

| Workers | `b938156` T (s) | bucketed T (s) | ΔT | `b938156` peak (MB) | bucketed peak (MB) | ΔM |
|---|---:|---:|---:|---:|---:|---:|
| 6 | 0.7903 | 0.5793 | -26.7% | 128.1 | 122.0 | -4.7% |
| 12 | 0.8050 | 0.5959 | -26.0% | 144.0 | 136.0 | -5.6% |

Graph-load phase medians: 0.379 → 0.172 s (6 workers) and
0.350 → 0.158 s (12); renumbering and search unchanged.
Run spreads were 17–22% on the shared host, below the measured change.

A separate instrumented run of `b938156` located the cgroup peak: it occurs
during search (about 112 MB of forward plus reverse graph and 17–31 MB of search
state), not in loading or renumbering, so the bucket transient stays below it:
[memory phases](../tests/performance/2026-10-31-traverse-memory-phases/README.md).

All outputs match Python; fourteen Rust tests (loader test now spans five
parts) and 1,224 Python comparisons pass on macOS and VPS. Mutation caught:
dropping the per-part edge base (3 tests fail). Board result follows below.
Evidence: [bucketed build](../tests/performance/2026-10-31-traverse-bucketed-parse/README.md).

Step 13 official board result, feed 2026-10-31 19:51:31 UTC (20:51:31 Warsaw):
`run-07992c342c3a`, commit `853ae88`, all gates passed. **0.238915 s,
141.635584 MB, S=675, D=1, K=16.679225, 61.108794 points**. New team best;
versus step 12: time −22.75% (1.2944x), memory −11.31%, +3.108758 points despite
S 632 → 675. Derived Track D position: 36 of 90 scored teams (tenth place at
68.37 points in this feed). Single live-board run, not final hidden-seed
scoring. Evidence:
[official bucketed-build result](../tests/performance/2026-10-31-board-bucketed-results/README.md).


## Step 14 — packed 32-bit edges (VPS measurement)

The step-12 memory profile put ~112 MB of the peak in the forward and reverse
graphs. Every edge is now one `u32`: the other endpoint in the low `head_bits`
(21 for the public graph, derived from `n`), the weight above it. Weights that do
not fit below the all-ones escape code are stored as that code with the exact
value in an `(edge index, weight)` list, sorted after each parallel build and
binary-searched on read (3,496 of 5,000,000 seed-1 weights). Applied to the
loaded, renumbered and reverse graphs; correct for any input.

Same VPS protocol, baseline `853ae88`:

| Workers | `853ae88` T (s) | packed T (s) | ΔT | `853ae88` peak (MB) | packed peak (MB) | ΔM |
|---|---:|---:|---:|---:|---:|---:|
| 6 | 0.6011 | 0.5956 | -0.9% | 121.4 | 96.5 | -20.5% |
| 12 | 0.6014 | 0.5735 | -4.6% | 136.2 | 101.4 | -25.6% |

Time is unchanged within run spread (10–28%); the step buys memory only. The
6- and 12-worker peaks now differ by ~5 MB; an instrumented run confirms the
peak moved to loading (bucketed triples plus the placed CSR: 96.2 / 101.2 MB at
6 / 12 workers), with renumbering at ~93 MB just below it. All outputs match Python; fifteen Rust tests (new:
packing widths and escapes, full-range weights in the loader test) and 1,224
Python comparisons pass on macOS and VPS. Mutation caught: decoding escaped
edges without the heavy list (6 tests fail). Board result follows below.
Evidence: [packed edges](../tests/performance/2026-10-31-traverse-packed-edges/README.md).

Step 14 official board result, feed 2026-10-31 19:58:32 UTC (20:58:32 Warsaw):
`run-f3b44d092f5b`, commit `1767ec0`, all gates passed. **0.242579 s,
132.395008 MB, S=731, D=1, K=16.578266, 60.976955 points**. Versus step 13:
memory −6.52%, time +1.53% (single runs, within board noise), **−0.131839
points**: the memory gain did not cover S 675 → 731. Step 13 remains the team
best. The board saved far less memory than the VPS (−25.6%). The VPS kernel
uses transparent huge pages only on `madvise`; public board logs from other
teams (Cache Me, FlyHigh) report that the scoring box backs anonymous memory
with 2 MB pages, so touching any byte of a reserved region buffer or distance
page commits 2 MB. That undoes this design's reliance on lazily touched 4 KB
pages and motivates step 15. Evidence:
[official packed-edge result](../tests/performance/2026-10-31-board-packed-results/README.md).


## Step 15 — 4 KB pages for the process (board experiment)

`main` first calls `prctl(PR_SET_THP_DISABLE, 1)` on Linux (four lines plus the
declaration; failure is ignored). Hypothesis from step 14: the box backs memory
with 2 MB transparent huge pages, so the sparsely touched reserved buffers and
distance pages commit far more than on the VPS. The VPS uses THP only on
`madvise`, so it cannot show the effect: there the candidate matches `1767ec0`
within noise (0.547 / 0.563 s vs 0.557 / 0.572 s at 6 / 12 workers, peak
unchanged at 96.6 / 101.4 MB) and all outputs match Python. Fifteen Rust tests
and 1,224 Python comparisons pass on macOS. The board run decides; see below.
Evidence: [THP check](../tests/performance/2026-10-31-traverse-thp-disable/README.md).

Step 15 official board result, feed 2026-10-31 20:12:15 UTC (21:12:15 Warsaw):
`run-b5ad6625b248`, commit `a37144d`, all gates passed. **0.253390 s,
101.629952 MB, S=743, D=1, K=17.060047, 61.599011 points**. New team best.
Versus step 14: memory −23.24% (132.40 → 101.63 MB), time +4.46%, +0.622056
points; versus step 13 (previous best) +0.490217. The board peak now matches
the VPS (101.4 MB), confirming the huge-page explanation: from here VPS memory
measurements transfer to the board. The time rise may be 4 KB-page TLB cost or
board noise; it is a single run. Derived Track D position: 36 of 90 (tenth
place 68.45). Evidence:
[official THP result](../tests/performance/2026-10-31-board-thp-results/README.md).


## Step 16 — single-u64 heap keys (VPS measurement)

The `u32` search now pushes `distance << 32 | node` as one `u64` heap key
(ordered exactly like the `(distance, node)` tuple, one comparison per sift);
the `u64` fallback keeps tuple keys through an associated `Distance::Key` type.
Idea credit: Team7's public PERF.md.

Same VPS protocol, baseline `a37144d`: total 0.5917 → 0.5627 s
(-4.9%) at 6 workers and 0.5835 → 0.5587 s (-4.2%) at
12, peak unchanged (96.4 / 101.4 MB). Search-phase medians
0.293 → 0.262 s and 0.304 → 0.283 s. Run spreads were
14–19%, so the total change is not cleanly resolved; both search phases moved
the same way. All outputs match Python; fifteen Rust tests and 1,224 Python
comparisons pass. Mutation caught: swapped key halves (2 tests fail). Board
result follows below.
Evidence: [heap key](../tests/performance/2026-10-31-traverse-heap-key/README.md).

Step 16 official board result, feed 2026-10-31 20:24:12 UTC (21:24:12 Warsaw):
`run-3a1c4950924c`, commit `b9a93cd`, all gates passed. **0.221692 s,
101.761024 MB, S=759, D=1, K=18.176073, 62.975003 points**. New team best;
versus step 15: time −12.51% (1.1430x), memory +0.13%, +1.375992 points. The
board gain exceeds the VPS estimate (−4.3% at 12 workers); part may be board
noise around step 15's slower run. Derived Track D position: 32 of 90 (tenth
place 68.45). Evidence:
[official heap-key result](../tests/performance/2026-10-31-board-heapkey-results/README.md).


## Step 17 — software prefetch (VPS measurement)

`_mm_prefetch` hints on x86-64 (no-op elsewhere): after each heap pop, the row
bounds and first edge of the vertex now on top of that side's heap; in the
renumbering BFS, row bounds eight queued vertices ahead and the first edge four
ahead. Idea credit: Team7's public PERF.md.

Same VPS protocol, baseline `b9a93cd`: total 0.5409 → 0.4818 s (-10.9%) at
6 workers and 0.5411 → 0.4734 s (-12.5%) at 12, peak unchanged.
Phase medians: renumbering 0.110 → 0.083 s / 0.112 → 0.088 s; search
0.270 → 0.252 s / 0.272 → 0.236 s (6 / 12 workers). Run spreads were 14–25%;
both phases moved the same way at both worker counts. All outputs match
Python; fifteen Rust tests and 1,224 Python comparisons pass. Board result below.
Evidence: [prefetch](../tests/performance/2026-10-31-traverse-prefetch/README.md).

Step 17 official board result, feed 2026-10-31 20:29:58 UTC (21:29:58 Warsaw):
`run-79245fc6bd21`, commit `7d0e8ab`, all gates passed. **0.205766 s,
101.924864 MB, S=783, D=1, K=18.772431, 63.676026 points**. New team best;
versus step 16: time −7.18% (1.0774x), memory +0.16%, +0.701023 points despite
S 759 → 783. Derived Track D position: 29 of 91 (tenth place 68.45). Evidence:
[official prefetch result](../tests/performance/2026-10-31-board-prefetch-results/README.md).


## Step 18 — SWAR record parsing (VPS measurement)

Graph records `u v w\n` are parsed with one 8-byte load per number: the first
byte with bit 4 clear gives the length, a nibble check rejects any non-digit
before it, and three multiplies combine the digits. Any other record shape
falls back to the unchanged strict parser, so accepted input and errors match.
Idea credit: Team7's public parser (MIT).

Same VPS protocol, baseline `7d0e8ab`: total 0.4966 → 0.4218 s (-15.0%)
at 6 workers and 0.4951 → 0.4428 s (-10.6%) at 12, peak unchanged.
Graph-load medians 0.163 → 0.116 s and 0.137 → 0.106 s; other phases
unchanged. All outputs match Python; sixteen Rust tests (new: 200,000 random
records, fast path always equal to the strict parser) and 1,224 Python
comparisons pass. Mutations caught: no digit check (1 test), wrong combine
multiplier (3). Board result below.
Evidence: [SWAR parsing](../tests/performance/2026-10-31-traverse-swar-parse/README.md).

Step 18 official board result, feed 2026-10-31 20:39:03 UTC (21:39:03 Warsaw):
`run-39656346ac0a`, commit `d6954e2`, all gates passed. **0.210076 s,
101.093376 MB, S=821, D=1, K=18.477508, 63.332170 points**. Versus step 17:
time +2.09%, memory −0.82%, **−0.343856 points** (S 783 → 821). The VPS
load-phase gain (−23 to −29%) did not show on the board; with twelve real
cores parsing is likely a small share of the board run, so its saving is
within single-run noise while the added lines are certain. Step 17 remains the
team best. The parser is kept for now and will be re-judged on the next board
run; if time does not follow, it is a candidate for removal in the code-size
pass. Evidence:
[official SWAR result](../tests/performance/2026-10-31-board-swar-results/README.md).


## Step 19 — memory diet (VPS measurement)

The peak is the maximum over phases, and at `d6954e2` load (101 MB),
renumbering (94 MB) and search (94 MB) were all close, so all three were cut
together. Load buckets now hold 8-byte `(source, packed target|weight)`
records with escaped weights in a side list. Renumbering only builds the
visiting order during the breadth-first pass, then copies rows from the old
graph into the new CSR in equal pieces (8 per worker), so no region edge
buffers exist. `mallopt(M_MMAP_THRESHOLD, 256 KB)` makes freed large buffers
return to the kernel. Trimming search state after long queries, freeing bucket
parts during placement, and prefetching claims in the copy were tried and
dropped (no peak gain or slower).

Same VPS protocol at 12 workers, baseline `d6954e2`: 0.4642 → 0.4807 s
(+3.6%), cgroup peak 101.2 → 79.0 MB (−22.0%). Phase medians: load
0.102 → 0.091 s, renumbering 0.087 → 0.114 s, search 0.248 → 0.268 s (noise
range). Phase marks put the new peak at the end of placement (78.8 MB), with
the renumbering copy next (74.7 MB). Without `mallopt` the peak stays at
98 MB. All outputs match Python; sixteen Rust tests and 1,224 Python
comparisons pass. Board result below.
Evidence: [memory diet](../tests/performance/2026-10-31-traverse-memory-diet/README.md).

Step 19 official board result, feed 2026-10-31 21:01:34 UTC (22:01:34 Warsaw):
`run-b507a889127d`, commit `3b648eb`, all gates passed. **0.195022 s,
79.945728 MB, S=875, D=1, K=19.907905, 64.951278 points**, new team best.
Versus step 17: time −5.22%, memory −21.56%, **+1.275252 points** despite
S 783 → 875. The board run got faster rather than the +3.6% seen on the VPS.
Rank 25 of 91. Evidence:
[official memory-diet result](../tests/performance/2026-10-31-board-memory-diet-results/README.md).

## Step 20 — code-size pass (VPS measurement)

S is now the main lever left after the memory diet: `3b648eb` scored S=875.
Code lines drop from 866 to 713 (S ≈ 722 with the Dockerfile) without
changing behaviour: the mapped file keeps its mapping for the run (no `Drop`),
queries are parsed from the mapped file with the same strict field parser,
the serial search path and the `Searcher` wrapper are gone, renumbering notes
degrees during the breadth-first pass instead of a separate parallel pass, and
several helpers are shorter. Same VPS protocol: 0.4847 → 0.4561 s at 6 workers
and 0.4816 → 0.4681 s at 12, peak memory within ±0.5%. All outputs match
Python; thirteen Rust tests and 1,224 Python comparisons pass. Expected board
gain from S alone: 50·0.15·log10(875/722) ≈ +0.63 points. Board result below.
Evidence: [code-size pass](../tests/performance/2026-10-31-traverse-code-size/README.md).

Step 20 official board result, feed 2026-10-31 21:19:47 UTC (22:19:47 Warsaw):
`run-49484fa644ae`, commit `1851b13`, all gates passed. **0.211929 s,
79.613952 MB, S=722, D=1, K=19.672249, 64.692701 points**. Versus step 19:
S 875 → 722 (+0.63 points alone), but time +8.67%, so **−0.258577 points**;
step 19 stays the team best. The VPS A/B had the slim build faster, and board
times for steps 17–20 span 0.195–0.212 s, so this is not yet evidence of a
regression. Evidence:
[official code-size result](../tests/performance/2026-10-31-board-code-size-results/README.md).

Step 20 re-sample: `2012f4e` only records the result above, so the board
rebuilt the same binary (`run-abe73b97cc7d`): **0.203360 s, 79.261696 MB,
S=722, 65.160080 points**, the new team best (+0.208802 over step 19). Two
runs of one binary differ by 4.0% in time, so single board runs cannot
resolve changes smaller than that. Evidence:
[official code-size result](../tests/performance/2026-10-31-board-code-size-results/README.md).

## Step 21 — Dial bucket queue (VPS measurement, rejected)

Public Track D code (Gigatron, FAHHHH) replaced the binary heap with a Dial
ring and recorded 15–33% gains. Two versions were tried inside the
bidirectional search: a ring of `Vec` buckets (4096 or 8192) with a far list,
and a pooled linked-list ring with an occupancy bitmap. Against the current
heap at 12 workers: +11%, +13% and +1.3% time, and cgroup peaks of 86.8, 89.7
and 84.8 MB, because buckets hold every pushed entry until the query ends.
Not adopted; the code is archived with the measurements. `Graph::row` goes
back to `enumerate()` (the slim pass had switched to `zip(start..)`, which
loses the slice-zip specialization on the hottest loop; same line count).
Evidence: [Dial queue](../tests/performance/2026-11-01-traverse-dial-queue/README.md).

## Step 22 — search tail and transpose (VPS diagnostics, rejected changes)

A per-query timing build showed the search makespan within 1–4% of perfect
balance (the 100 longest queries are half of all search work, but the atomic
counter already spreads them), so query ordering cannot help. Transpose
takes 51–62 ms (about 11%): about 15 ms counting in-degrees, 16 ms prefix and
allocation, and 25 ms scatter. First-touch page faults cost about 20 ms per
28 MB on one thread. Counting from the raw edge array saved about 2 ms
(noise). Counting in the renumbering copy and reusing the old graph's
buffers was 7.6% slower at 12 workers with an 85 MB peak, because the atomic
increments contended and the old graph lived longer. Both were rejected.
Evidence: [search tail](../tests/performance/2026-11-01-traverse-search-tail/README.md),
[transpose](../tests/performance/2026-11-01-traverse-transpose/README.md).

## Step 23 — chained BFS blobs (VPS neutral, board +0.18)

Cache Me and FlyHigh publish gains from replacing one ring-by-ring BFS region
per worker with chained balls of 512 (or 64) vertices. On the VPS the search
phase is unchanged with 512 (258.5 vs 259.5 ms) and slower with 128 and 64
(295 and 310 ms, peaks 91.5 and 104.5 MB). The VPS cache is larger than the
board's, so blob 512 is queued as a board-only experiment (+~10 lines).
Evidence: [blobs](../tests/performance/2026-11-01-traverse-blobs/README.md).

Step 23 board result: `run-81554f49a3d1`, commit `cae88c9`, all gates passed.
**0.198285 s, 79.855616 MB, S=736, 65.339497 points**, +0.18 over the step 20
best sample, inside the ±4% single-run spread (S +13 included). A record-only
commit gives the second sample. Evidence:
[official blobs result](../tests/performance/2026-11-01-board-blobs-results/README.md).
