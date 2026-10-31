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
Queries run sequentially; no routing index or answer cache is used.

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

## Dependencies

None.

## With another 24 hours

Validate degree rejection on the full x86-64 workload, measure loading and query
work separately, then compare parallel independent queries (D7 in the plan).
