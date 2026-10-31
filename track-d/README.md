# Track D: Traverse

The task, the input files and the exact output format are in the [spec](https://github.com/hackathon-raptors/speed-demon-2026/blob/main/specs/d-traverse.md).

## Approach

Rust implementation of the organisers' forward Dijkstra algorithm. Load a CSR
graph with `u32` offsets and `(u32 target, u32 weight)` edges, retaining every
edge. Reuse a `u64` distance array and binary min-heap across queries, resetting
only touched vertices. Stop when the target is popped, skipping stale entries.
Zero weights, parallel edges, duplicates and isolated nodes are supported.
Before searching, return zero for identical endpoints and reject queries with
no outgoing edge at the source or no incoming edge at the target. Incoming-edge
presence uses one bit per vertex (217,568 bytes at 1,740,512 vertices).
Independent queries run on scoped standard-library threads sharing the immutable
graph. Each worker reuses its own search state and claims its next query from an
atomic counter; indexed answers preserve input order. No routing index or answer
cache is used. The default uses available parallelism capped at 12 workers and
the query count; `--threads N` overrides it (positive integer, capped at 12).
One worker follows the serial path; empty queries allocate no search state.
On the six-vCPU Linux VPS, full seed 1 matches Python. Adding mmap/parallel CSR
to D7 reduced time from 3.77 s to 2.38 s at six workers (36.8% less time), with
container peak memory 139.7 to 140.8 MB. One-worker loading is slower; the
improvement applies to parallel modes. Details and limitations are in `PERF.md`.

The graph is mapped read-only with `mmap` and divided into newline-aligned chunks.
Scoped workers make two passes: count degrees/incoming presence with relaxed
atomics, prefix counts, then parse/scatter directly into CSR using unique atomic
cursor positions. No temporary triple array is stored. The mapping is released
after the loader's workers join. Within-source edge order may vary; every edge
is retained. Loading uses the same worker limit as query processing.

Graph fields are parsed directly from borrowed ASCII byte slices; queries retain
a reusable line buffer and share the checked `u32` parser. No temporary strings
or field vectors. Overflow, field counts, vertex bounds, LF/CRLF and final lines
without a newline are handled. The std-only mapping wrapper targets 64-bit Unix
(the Linux competition target and macOS development host); input files remain
immutable during loading, as with the read-only competition data mount.

Before searching, relabel vertices by BFS discovery order over every component.
Rebuild CSR rows and incoming presence in the new order and translate query
endpoints. Disjoint mutable row slices let scoped workers copy/remap edges in
parallel without additional unsafe code. All vertices and duplicate edges remain;
query order and exact distances are preserved. Empty queries skip the rebuild.
The temporary old graph and permutation are released before query processing.
On the six-vCPU VPS, this reduces full-seed1 time from 2.35 s to 1.63 s including
preprocessing (30.5% less time), with container peak 140.8 to 147.8 MB. See PERF.md
for the DFS/BFS comparison, phase timings, repeated measurements and limitations.

Release builds use fat LTO, one codegen unit and abort on panic. Docker builds
also use `target-cpu=native`. Local VPS measurements are recorded in `PERF.md`;
the small measured build-profile improvement remains inconclusive.

Build: `cargo build --release --offline --locked`.
Run: `target/release/traverse-baseline --data /data --out /out`.
The container defaults to `/data` and `/out`.
For a serial comparison, append `--threads 1`.

## Dependencies

None.

## With another 24 hours

Compare 6 versus 12 workers on the competition machine using both time and
cgroup peak memory, then compare 4-ary and radix priority queues (D9 in the plan)
against the binary heap, including all loading/reordering costs.
