# Track D: read-only mmap and parallel two-pass CSR

Baseline: submitted D7 `b8e5bc17c22d380898a650b399bd04aefb21ec04`, exact source
`before-main.rs`. Candidate: `after-main.rs`, patch in `candidate.patch`.
Search, result ordering and the query worker policy are unchanged.

## Implementation

A std-only 64-bit Unix wrapper creates a read-only MAP_PRIVATE graph mapping.
It owns the mapping and unmaps it after graph loading, including error paths.
The input stays immutable during loading under the competition read-only mount.
Empty files are handled without calling mmap with a zero length. The mapping is
borrowed as byte slices; it does not need a custom Send/Sync implementation.

Graph header and records use the same checked decimal-u32 parser as buffered
queries. Workers receive disjoint newline-aligned slices; the EOF tail belongs
to exactly one slice. First pass counts source degrees and incoming-edge presence
with relaxed atomic RMWs; the record count and every endpoint are validated.
A serial prefix builds CSR offsets. Second pass re-parses slices and scatters
directly into the final edge Vec using per-source atomic cursors. There is no
60 MB temporary triple array. The same positive --threads limit (cap 12) applies
to loading and query workers; default is available parallelism.

The scatter's unsafe writes are confined to unique in-bounds slots. Each input
record is visited once per pass, source counts fix disjoint CSR ranges, and
fetch_add linearizes slot assignment within a source. The edge Vec's allocation
remains stable and no edge is read until all scoped workers join. The pointer
is shared via AtomicPtr; the graph itself retains ordinary packed (u32,u32)
edges. Adjacent edge slots do not overlap. Within-source ordering may differ
between runs; edge multiplicity and shortest-path results must remain exact.

Queries remain buffered (about 29 KB at full scale) and share parse_fields;
only the graph is mapped and parsed in parallel. No dependencies were added.
The FFI wrapper and raw CSR writes have local safety comments in the source.

## Validation and VPS protocol

Local macOS arm64: ten Rust tests, 204 deterministic graph/query cases under
default/1/2/4/6/12 workers (1,224 Python comparisons), eight invalid inputs and
three invalid thread options. Clippy with warnings denied, release build and
formatting pass. Separate profile builds pass a small output/JSON smoke check.

Tests verify chunk coverage/newline alignment, empty mappings, numeric limits,
LF/CRLF and EOF tails, exact CSR offsets and per-source edge multisets, and
20,000 edges contending on three source cursors, repeated at 1/2/4/6/12 workers.
The existing search tests cover skewed work, duplicates, unreachable/self paths,
over-u32 distances and ordered answers. Tests stay outside the scored track.

Remote host `my-vps`: six Haswell KVM vCPUs, Linux x86-64. The clean remote
workspace at `f78482e` was left unchanged; all work is isolated in
`/tmp/traverse-mmap-XV2yq8`. Before and after use the identical pinned Rust image,
offline release LTO/one codegen unit/abort profile, and target-cpu=native.
Exact source, Cargo, regression-test and runner hashes are in source-metadata.json.

Pinned Debian runtime; CPU set 0-5 for every variant; 8 GB memory with equal
memory+swap limit, 512 pids, no network, read-only root, dropped capabilities,
no-new-privileges, UID 65534, 1 GB /tmp. All six variants (before/after × 1/6/12
workers) run once for warmup and five times for measurement, rotating order.
Discard min/max and average middle three; medians, spreads and every raw run
are retained. Twelve workers oversubscribe six vCPUs. Each full-seed1 input is
prewarmed before creating a fresh measured container. All outputs are checked
byte-for-byte against the saved Python reference (SHA-256
`a23dbdf0a3e49cc8901310a9ee1dc6a9a569a85297b6da75c2a7002cb2ed277e`).

The external measure.rs wrapper measures child wall time inside the container,
excluding Docker startup, and captures child ru_maxrss in KiB using wait4.
It also reads the container's cgroup peak after the child exits, verifying the
8 GB limit identifies the intended cgroup. On this VPS it is **v2 memory.peak**.
This includes the wrapper and the container workload. Mmap's prewarmed input
pages appear in RSS but are already charged outside the fresh cgroup, so RSS
alone is not an appropriate scoring-memory comparison. These remain VPS
measurements, not a board run or certified points.

Separate diagnostic sources derived by prepare.py add timers around graph load,
query parsing, search/state/joins and output, plus graph first pass, prefix /
allocation, and scatter/second pass. Three alternating profiled runs per variant,
six workers; phase summaries are medians. Instrumented sources are outside the
scored track; normal timings come from uninstrumented binaries.

## Reproduction

From a copy of the repository to preserve evidence, run:

```sh
python3 tests/performance/2026-10-31-traverse-mmap-parallel/prepare.py
ssh my-vps 'run_dir=$(mktemp -d /tmp/traverse-mmap-XXXXXX) && tar -xf - -C "$run_dir" && python3 -u "$run_dir/experiment.py" "$run_dir"' < /tmp/traverse-mmap-vps.tar
```

This requires the existing official data/reference paths and cached pinned
images listed in experiment.py. Full expanded commands and output are in console.txt.
The public-board audit and previous rejected serial two-pass experiment are
preserved separately. That prior experiment now also retains its original
measured-parser-tests.rs so its preparation script remains reproducible after
this task's loader API/test changes.

## Initial measurements

Normal binaries, one warmup and five measured executions per variant. Time is
the mean of the middle three; memory is the maximum cgroup peak over all five.
Cgroup MB is decimal 1,000,000 bytes, RSS MiB is binary 1,048,576 bytes.

| Workers | Baseline time | Mmap/parallel time | Time change | Baseline cgroup peak | Mmap/parallel cgroup peak |
|---|---:|---:|---:|---:|---:|
| 1 | 12.794612 s | 13.718822 s | +7.22% | 115.802112 MB | 69.505024 MB |
| 6 | 3.738694 s | 2.403746 s | -35.71% | 139.251712 MB | 140.656640 MB |
| 12 | 3.708020 s | 2.377229 s | -35.89% | 228.446208 MB | 228.941824 MB |

The first six-worker result is 1.56x faster, 35.71% less time, with a 1.01%
increase in maximum container peak. Twelve workers show 35.89% less time and
0.22% more peak memory on this six-vCPU VM. One worker is 7.22% slower but uses
40.0% less cgroup peak memory. The new loader is aimed at the parallel default;
its extra parse/atomic work does not pay for itself with one worker.

Six-worker RSS rises 132.27 → 141.01 MiB while cgroup peak stays near 140 MB.
This is why the mapped input must not be evaluated through RSS alone. Full raw
RSS and per-run cgroup source names are retained in the results.

Spreads before-1/6/12: 12.12% / 4.27% / 3.14%; after-1/6/12:
5.96% / 6.62% / 4.90%. A targeted six-worker repeat follows because the first
candidate spread exceeded 5%. No precise board speedup or score is claimed.

### Phase medians (separate diagnostic builds, six workers)

| Phase | Baseline | Mmap/parallel |
|---|---:|---:|
| Graph load/build | 1.658454 s | 0.378113 s |
| Parse queries | 0.000206 s | 0.000199 s |
| Search incl. state/joins | 1.983557 s | 1.961185 s |
| Format/flush output | 0.000182 s | 0.000116 s |

| Graph subphase | Baseline | Mmap/parallel |
|---|---:|---:|
| Parse/count (+ save triples before) | 0.938488 s | 0.140717 s |
| Prefix/allocate | 0.007990 s | 0.016769 s |
| Scatter (+ second parse after) | 0.754754 s | 0.214049 s |

Graph loading/building is **4.39x faster** by diagnostic medians.
The new baseline phase-time share is about 84% search and 16% graph: search layout
(D8) is now the larger target. Phase medians need not add to an outer-phase or
process-time median; normal and diagnostic runs are separate measurements.

`summary-raw.json` is the exact remote aggregate. Its generic memory_metric
header had an outdated "v1 on VPS" label; every actual row and the wrapper
reports v2 memory.peak. `summary.json` corrects only that header, annotates the
normalization and adds cgroup delta calculations; individual measurements are
unchanged. The measured experiment.py is preserved exactly to match its hash.

All 36 full normal/warmup outputs and six full diagnostic outputs match the
saved Python reference. Ten regression tests and all 1,224 Python comparisons
also pass on Linux, including invalid input/options; raw logs are retained.

## Six-worker repeat and decision

The first candidate spread (6.62%) triggered one repeat with the same binaries,
warm inputs, wrapper, CPU set, runtime limits and six workers. One warmup and
five alternating measurements; discard min/max and average middle three.
Exact script and command log: repeat-six.py and repeat-six-console.txt.

| Repeat, six workers | Time | Max cgroup peak | Max RSS | Time spread |
|---|---:|---:|---:|---:|
| D7 baseline | 3.767454 s | 139.730944 MB | 132.97 MiB | 2.31% |
| Mmap/parallel | 2.380925 s | 140.783616 MB | 141.02 MiB | 7.40% |

The repeat shows **36.80% less time (1.58x)** and 0.75% more cgroup peak memory.
The candidate spread remains 7.40%, so a precise scoring-machine speedup is not
established; both independent batches show a large improvement and profile data
explains it. No further repetitions are used to select a favorable number.
All twelve repeat outputs match the full Python reference. Total full-output
checks: **54** (36 normal/warmups, six diagnostics, twelve repeat).

**Decision: retain mmap/parallel CSR in track-d/src/main.rs.** It removes the
triple array and improves the default parallel mode on this VPS. One-worker mode
remains slower; memory primarily remains graph plus per-query-worker state in
the parallel modes. No board submission or official run ID for this step yet.
The next search experiment is D8 vertex reordering; worker-count selection on
actual six-core/twelve-thread competition hardware remains separate.

The preparation helper uses frozen measured-Cargo.toml/lock, measured-parser-tests.rs,
measured-python-check.py and measured-reference.py. It fixes the baseline commit
to b8e5bc1 and reconstructs exact measured sources instead of depending on future
changes to the checkout. Original measured runner/test/source hashes are preserved.
