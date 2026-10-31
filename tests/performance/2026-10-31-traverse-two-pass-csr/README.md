# Track D: phase profiling and two-pass CSR experiment

Baseline: submitted D7 at `b8e5bc17c22d380898a650b399bd04aefb21ec04`.
Candidate: `two-pass-main.rs`, exactly the baseline with only the graph loader
changed (`candidate.patch`). The candidate remains outside the scored track
while it is evaluated. Existing uncommitted official-board documentation is
preserved separately from this experiment.

## Variants

The baseline parses each graph record once into `(u, v, w)` triples, counts
out-degrees and incoming presence, prefixes counts, then scatters triples into
CSR. At 5,000,000 edges the temporary triple payload is 60,000,000 bytes.

The candidate uses the same checked byte parser and buffer. Pass one counts
out-degrees/incoming presence and verifies the edge count. It allocates CSR,
rewinds the same open reader, and parses/scatters the records again. It checks
header/count/range consistency on the second pass. Both loaders retain the
input order of edges within each source vertex. No mmap or parallel parsing
was added, to isolate two-pass CSR. Search and scheduling are identical.

## Profiling

`prepare.py` deterministically derives `profile-before-main.rs` and
`profile-after-main.rs` from the exact normal sources. Timing instrumentation
exists only in these separate diagnostic builds, outside the competition track.

Main phases: graph loading/building, query parsing, search, formatting/flush.
Graph subphases: first parse/count (also saving triples in baseline), prefix and
allocation, scatter (including rewind and second parse in candidate). Graph
subphases exclude opening/reading the header; the outer graph phase includes
that setup and loader cleanup. Search includes worker creation, distance-array
initialization, all searches, joins, worker-state cleanup and ordered collection.
Directory creation/output-file opening and process startup/exit are outside
these phase timers but inside the external normal wall-time measurement.

The profiler changes search result collection into a named temporary to put a
timer around it, preserving behavior. It prints JSON to stderr; stdout and
`distances.txt` remain unchanged. Three alternating diagnostic runs per variant,
six workers, warm inputs. Profile summaries are medians of three, not timed-run
trimmed means. Instrumented timings are diagnostic, never substituted for the
normal binary's time.

## VPS method

`my-vps`: six Haswell KVM vCPUs, Linux x86-64, all variants pinned to CPU set
`0-5`. Remote workspace at `f78482e` was clean and left unchanged. Temporary
build/test directory: `/tmp/traverse-two-pass-7Eo3Hy`.

Both normal and both diagnostic projects are built offline using the track's
pinned Rust image, release LTO/one codegen unit/abort profile and identical
`RUSTFLAGS=-C target-cpu=native`. Exact source/Cargo/test/runner hashes are in
`source-metadata.json`; `compiler.txt` records the toolchain.

Normal timed variants: baseline and candidate, each with 1, 6 and 12 workers.
One warmup and five measured runs each; variant order rotates per round.
Discard minimum and maximum and average the middle three. All individual runs,
medians, spread and maximum RSS over measured runs are retained.

Runtime: pinned Debian 13; no network, read-only root, dropped capabilities,
no-new-privileges, UID 65534, 8 GB memory with equal memory+swap limit, 512 pids,
1 GB `/tmp`. Full official seed-1 inputs are mounted read-only and prewarmed
before each container is created. The six-CPU VM is oversubscribed at 12 workers.

`measure.rs` times only the child inside the container, excluding Docker startup,
and uses Linux x86-64 `wait4` to collect child maximum RSS in KiB. RSS is **not**
the competition's cgroup `memory.peak`; no official score is inferred from it.
Every output is compared with the saved Python full reference, expected SHA-256
`a23dbdf0a3e49cc8901310a9ee1dc6a9a569a85297b6da75c2a7002cb2ed277e`.

## Reproduction

From the repository root, `python3 tests/performance/2026-10-31-traverse-two-pass-csr/prepare.py`
creates exact instrumented snapshots and `/tmp/traverse-two-pass-vps.tar`.
On the configured VPS, using only fresh temporary directories and cached images:

```sh
ssh my-vps 'run_dir=$(mktemp -d /tmp/traverse-two-pass-XXXXXX) && tar -xf - -C "$run_dir" && python3 -u "$run_dir/experiment.py" "$run_dir"' < /tmp/traverse-two-pass-vps.tar
```

The experiment requires existing official data/reference paths documented in
`experiment.py`. Full expanded commands and build/test output are in `console.txt`.

## Measured results

Linux x86-64, pinned Rust 1.99.1, full seed 1. Normal uninstrumented binaries:

| Workers | Baseline time | Two-pass time | Time change | Baseline max RSS | Two-pass max RSS |
|---|---:|---:|---:|---:|---:|
| 1 | 12.856764 s | 13.414020 s | +4.33% | 110.73 MiB | 60.96 MiB |
| 6 | 3.755675 s | 4.154059 s | +10.61% | 132.58 MiB | 132.46 MiB |
| 12 | 3.630838 s | 4.285671 s | +18.04% | 217.51 MiB | 217.72 MiB |

RSS falls 44.95% with one worker. With six or twelve workers, graph-loading
memory is no longer the process peak: the graph plus private search state
uses more memory, so removing temporary triples gives no observed peak benefit.
At six workers, first-batch time is 10.61% worse; at twelve, 18.04% worse.

The six-worker baseline max/min spread was 10.47%, prompting the separate
six-worker repeat below. Other first-batch spreads: before-1 6.46%, before-12
8.89%, after-1 2.67%, after-6 3.48%, after-12 8.23%. This is a shared VPS;
small differences and absolute speed are not certified competition results.

### Phase medians, six-worker diagnostic builds

| Phase | Baseline | Two-pass |
|---|---:|---:|
| Graph load/build | 1.575736 s | 2.143336 s |
| Parse queries | 0.000311 s | 0.000189 s |
| Search incl. workers/state | 1.991407 s | 1.980895 s |
| Format/flush output | 0.000121 s | 0.000109 s |

| Graph subphase | Baseline | Two-pass |
|---|---:|---:|
| First parse/count (+ triples in baseline) | 0.870965 s | 0.778087 s |
| Prefix / allocate CSR | 0.006498 s | 0.006355 s |
| Scatter (+ second parse in candidate) | 0.731631 s | 1.325531 s |

Medians of individual phase durations need not add up to the median outer
phase or whole-process time. Relative to the sum of baseline main-phase medians,
graph load/build is about 44%, search about 56%; parsing queries and writing are
negligible here. Candidate graph load/build is 36.02% longer by these diagnostic
medians; search is approximately unchanged. The second parse raises scatter
from 0.732 to 1.326 s, outweighing the first-pass saving from dropping triples.

This isolates a **serial buffered two-pass** loader. It does not measure mmap,
parallel parsing, reordered vertices, a new heap, or the scoring machine.

### Correctness and supporting files

All 36 normal timed/warmup outputs and six diagnostic outputs match the Python
full reference byte-for-byte. Eight regression tests pass on VPS, including
exact CSR order/offsets, parallel and duplicate edges, max-u32 weights, incoming
bits, empty/zero-edge graphs and CRLF/EOF. All 1,224 random/fixture Python checks
also pass at default/1/2/4/6/12 workers, plus eight malformed-input and three
invalid-thread checks. Candidate release Clippy with warnings denied and source
formatting passed locally; both normal/profiled builds passed on VPS.

`runs.jsonl` and `summary.json` hold the complete normal measurements.
`profiles.jsonl` and `profile-summary.json` hold each diagnostic stderr and the
phase medians. Source snapshots, `candidate.patch`, build/test logs, metadata,
CPU/compiler details and exact scripts are retained alongside them.

## Six-worker repeat and decision

The first six-worker baseline spread (10.47%) justified one repeat using the
same exact binaries, cached input, cpuset, runtime limits and wrapper. One
warmup and five alternating measurements per variant; no rebuild or source
change. `repeat-six.py`, `repeat-six-console.txt`, `repeat-six-runs.jsonl` and
`repeat-six-summary.json` preserve the complete repeat and its motivation.
The remote command streams the script into the existing isolated directory:

```sh
ssh my-vps 'python3 -u - /tmp/traverse-two-pass-7Eo3Hy' < tests/performance/2026-10-31-traverse-two-pass-csr/repeat-six.py
```

| Repeat, 6 workers | Trimmed mean | Max child RSS | Max/min time spread |
|---|---:|---:|---:|
| Baseline | 3.680718 s | 132.86 MiB | 7.01% |
| Two-pass | 4.124607 s | 132.71 MiB | 5.96% |

Two-pass time increased **12.06%**, with essentially unchanged RSS.
All twelve repeat outputs matched the full Python reference. Across the initial
batch, diagnostic builds and repeat, **54 full outputs** matched. The spreads
remain above 5% on this shared VPS, so the exact slowdown is uncertain; both
independent batches show a slowdown, and the phase data explains its direction.

**Decision: reject this serial buffered two-pass candidate for the current
parallel implementation.** `track-d/src/main.rs` remains byte-for-byte the
submitted D7 baseline; no competition submission was made for this experiment.
The new graph-loader regression test is retained outside the scored track.
This is a rejected implementation, not a claim that all two-pass architectures
are unsuitable: mmap/parallel parsing would be separate experiments.

The baseline spends roughly 44% of its diagnostic phase time loading/building
the graph and 56% searching. Both phases matter for the first-place target.
A next loading experiment should compare mmap and parallel parse/build, while
search work should separately evaluate D8 vertex reordering. Removing the triple
buffer alone does not reduce the parallel query-state peak.
