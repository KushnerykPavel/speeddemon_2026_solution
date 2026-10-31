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
On the six-vCPU Linux VPS, full seed 1 matches Python and six workers reduce
time from 12.99 s to 3.71 s (3.50x); measurements and limitations are in `PERF.md`.

Graph headers, edges and queries are parsed directly from ASCII bytes into
`u32` values, using one reusable line buffer per file. There are no temporary
strings or per-record field vectors. Parsing checks overflow, field counts and
vertex bounds, and supports LF, CRLF and a final line without a newline.

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

Validate additional seeds and compare 6 versus 12 workers on the competition
machine using both time and cgroup peak memory. Then test DFS/BFS vertex
reordering (D8 in the plan), including its preprocessing cost.
