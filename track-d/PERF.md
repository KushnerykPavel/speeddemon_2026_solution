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
and 1,224 Python comparisons pass on macOS and VPS. No board run ID yet.
Evidence: [D12 evidence](../tests/performance/2026-10-31-traverse-u32-distances/README.md).
