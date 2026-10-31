# D8: vertex reordering, including preprocessing cost

Baseline: submitted mmap/parallel CSR at
`e5bad19ffc9c7c18bb543758bde5a33a470bdd8c` (`before-main.rs`).
Initial alternatives: `dfs-main.rs` and `bfs-main.rs`. Selected refinement:
`bfs-par-main.rs`, BFS discovery order with parallel safe row-slice CSR rebuild.
The input parser, original CSR loader, Dijkstra and query scheduling are unchanged.

## Algorithms and correctness

Every component is covered by starting a traversal at each still-unseen old
vertex, in ascending old-ID order. The permutation maps old IDs to discovery IDs;
a separate order vector is new-ID-to-old-ID. Isolated vertices and directed
components unreachable from earlier roots still receive IDs. No graph-shape,
coordinate, seed, vertex-count or answer assumptions are used.

DFS uses an explicit stack and assigns an ID when first popped. BFS marks on
enqueue and uses the order vector itself as its queue. Both rebuild each CSR row
in new source-ID order, translate targets, preserve every weight and duplicate,
remap incoming presence and translate query endpoints without reordering queries.
Empty query input skips the preprocessing entirely.

The selected refinement keeps BFS discovery serial, computes new offsets from
old row degrees, and splits the new edge Vec into disjoint mutable slices at
row boundaries. Scoped workers copy/remap their rows; Rust's slice ownership
provides exclusive destinations. No new unsafe code or dependency is introduced.
The same --threads limit, capped at 12, applies to reconstruction. The old graph,
permutation and order are released before query processing.

Edge order from the original atomic CSR scatter can vary between runs; traversal
order may consequently vary too. The result remains exact. Measured spreads are
preserved instead of claiming a deterministic layout or precise speedup.

## Validation

Each DFS/BFS/parallel-BFS candidate passes eleven Rust tests and the same 204
Python-reference cases at default/1/2/4/6/12 workers: 1,224 output comparisons per
candidate. Eight invalid-input cases and three invalid-thread options pass too.
The new test checks a complete permutation, remapped edge multisets/incoming bits,
and all 16,900 ordered vertex pairs on a disconnected 130-node graph with
isolated nodes, islands, one-way edges, self-loops, parallel/duplicate edges,
zero/max-u32 weights and an over-u32 distance. Existing parser, mmap, contention
and search-state tests remain enabled. Local release build, Clippy and formatting
pass; profiled DFS/BFS builds pass an output/JSON smoke check.

The final candidate also matches saved Python outputs on tiny/sample seeds 1,7,42
and three edge cases (nine datasets). Output hashes agree with the preserved
`tests/validation/rust-baseline-matrix.json`; current input/output hashes are
recorded in `matrix-checks.json`.

## VPS protocol and artifacts

`my-vps`, six Haswell KVM vCPUs, Linux x86-64. Remote workspace at `f78482e` was
clean and left unchanged; all work lives in `/tmp/traverse-reorder-pohjf0`.
Offline builds use the identical pinned Rust image, native CPU, fat LTO, one
codegen unit and abort profile. The external wrapper records wall time excluding
Docker startup, child RSS and the whole fresh container's v2 memory.peak.
Inputs are prewarmed outside each measured cgroup. The container peak includes
the wrapper; these are VPS results, not board measurements or certified points.

All variants use CPU set 0-5, 8 GB memory and equal memory+swap limit, 512 pids,
no network, read-only root, dropped capabilities, no-new-privileges, UID 65534,
and 1 GB /tmp. Twelve workers oversubscribe this six-vCPU VM.
One warmup and five timed runs per variant, rotating variant order; remove
min/max and average the middle three. Reported peak memory is the maximum of
all five measured runs. Every full output is compared byte-for-byte to the
preserved Python reference, SHA-256
`a23dbdf0a3e49cc8901310a9ee1dc6a9a569a85297b6da75c2a7002cb2ed277e`.

Initial three-way experiment: baseline/DFS/BFS, each at 6 and 12 workers.
`summary.json`, `runs.jsonl` and `console.txt` preserve all 36 normal runs.
Three separate phase builds, three rotating runs each at 6 workers, add timers
around graph load, query parsing, reordering (including CSR rebuild and cleanup),
search/state/joins and output. `profiles.jsonl` and `profile-summary.json` preserve
all nine diagnostics and their phase medians. Individual phase medians need not
sum to a median process time; normal binaries have no instrumentation.

Refinement reruns baseline at 6/12, serial BFS at 6, and parallel-rebuild BFS at
6/12 on the same host and unchanged baseline binaries. New source/test hashes
are in `refine-source-metadata.json`; initial hashes in `source-metadata.json`.
`refine-summary.json`, `refine-runs.jsonl` and `refine-console.txt` preserve the
30 normal runs; three separate candidate diagnostics are in `refine-profiles.json`.
The refinement also performs the nine-case official-generator/edge-case matrix.

## Initial DFS/BFS comparison

| Variant | Workers | Trimmed mean | Max cgroup peak | Time spread |
|---|---:|---:|---:|---:|
| before | 6 | 2.419476 s | 140.849152 MB | 8.89% |
| before | 12 | 2.462673 s | 228.917248 MB | 4.60% |
| dfs | 6 | 2.003979 s | 148.889600 MB | 13.77% |
| dfs | 12 | 2.069629 s | 235.229184 MB | 6.56% |
| bfs | 6 | 1.891317 s | 147.537920 MB | 9.53% |
| bfs | 12 | 1.986810 s | 233.791488 MB | 19.86% |

BFS improved complete six-worker time 21.83%, DFS 17.17%. BFS was selected for
refinement because it gave the better complete time and uses a bounded n-entry
order/queue, while the DFS stack can hold duplicate pending vertices.

## Parallel-rebuild refinement and repeated controls

| Variant | Workers | Trimmed mean | Max cgroup peak | Time spread |
|---|---:|---:|---:|---:|
| before | 6 | 2.276060 s | 141.078528 MB | 3.21% |
| before | 12 | 2.343141 s | 229.167104 MB | 4.25% |
| bfs | 6 | 1.947441 s | 147.718144 MB | 6.40% |
| bfs-par | 6 | 1.597091 s | 147.447808 MB | 6.68% |
| bfs-par | 12 | 1.626069 s | 233.906176 MB | 9.84% |

The selected parallel-rebuild BFS reduces six-worker time 29.83% against its
same-batch baseline, while peak cgroup memory rises 4.51%. Twelve workers reduce
time 30.60%, with 2.07% more peak memory; this VM cannot select the best count for
the actual competition hardware. Serial BFS's six-worker time was 1.947441 s in
this batch versus 1.597091 s with parallel reconstruction.

## Diagnostic phase medians, six workers

| Phase | Before | DFS | BFS | BFS + parallel rebuild |
|---|---:|---:|---:|---:|
| Graph load | 0.366568 s | 0.397146 s | 0.364155 s | 0.376768 s |
| Parse queries | 0.000213 s | 0.000193 s | 0.000193 s | 0.000224 s |
| Permutation + CSR + cleanup | 0.000000 s | 0.836939 s | 0.706970 s | 0.424362 s |
| Search incl. state/joins | 2.012080 s | 0.773160 s | 0.836417 s | 0.837218 s |
| Format/flush | 0.000135 s | 0.000135 s | 0.000146 s | 0.000112 s |

Search improves from 2.012080 s to about 0.837218 s (2.40x), while preprocessing
costs 0.424362 s. Both costs are included in normal full-process timings. The
serial BFS preprocessing median was 0.706970 s; parallel copying reduces it
without changing the traversal or graph semantics. Phase batches are distinct,
so they explain direction rather than certifying exact subtraction of costs.

## Reproduction

The checked-in source/manifest/test snapshots are frozen. From a copy of the repo:

```sh
python3 tests/performance/2026-10-31-traverse-reorder/prepare.py
ssh my-vps 'run_dir=$(mktemp -d /tmp/traverse-reorder-XXXXXX) && tar -xf - -C "$run_dir" && python3 -u "$run_dir/experiment.py" "$run_dir"' < /tmp/traverse-reorder-vps.tar
```

Note the newly printed remote directory. For the refinement, use that fresh
experiment directory in place of the original below:

```sh
python3 tests/performance/2026-10-31-traverse-reorder/prepare-refine.py
ssh my-vps 'tar -xf - -C /tmp/traverse-reorder-pohjf0 && python3 -u /tmp/traverse-reorder-pohjf0/refine.py /tmp/traverse-reorder-pohjf0' < /tmp/traverse-reorder-refine.tar
```

Do not rerun into the original result directory: keep each experiment's evidence.
Measured Cargo, reference and two test-template versions are retained: the
original DFS/BFS helper takes two arguments, the parallel refinement takes three.
Preparation scripts derive matching test includes and separate profile builds.

## Confirmation and final decision

The refinement's chosen six-worker spread (6.68%) triggered one confirmation
against the same original baseline binary, on the same CPU set and limits.
One warmup plus five alternating measurements each; discard min/max and average
the middle three. `repeat-chosen.py`, `repeat-chosen-console.txt`,
`repeat-six-runs.jsonl` and `repeat-six-summary.json` retain all twelve runs.

| Confirmation | Time | Max cgroup peak | Time spread |
|---|---:|---:|---:|
| Original e5bad19 | 2.347075 s | 140.779520 MB | 5.00% |
| BFS + parallel rebuild | 1.630284 s | 147.820544 MB | 7.38% |

The confirmation shows **30.54% less time (1.44x)** with about 5.00% more peak
container memory. Spread remains above 5% for the candidate; exact speedup is
uncertain on a shared VM. Both refinement and confirmation show the same large
improvement including full preprocessing. No board score is inferred.

**Decision: retain BFS discovery order with parallel row-slice reconstruction**
in track-d/src/main.rs. DFS and serial BFS remain as experiment snapshots.
All **90 full outputs** (36 initial normal + 9 initial diagnostics + 30 refinement
normal + 3 refinement diagnostics + 12 confirmation) match Python. Nine additional
matrix outputs match both the saved Python bytes and original baseline hashes.
The selected candidate passes eleven Rust tests, 1,224 Python comparisons,
eight invalid inputs and three invalid thread options; the prototypes each pass
the same eleven tests and Python matrix too. No competition submission/run ID
for this step yet.
