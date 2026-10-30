# Track D: Traverse

The task, the input files and the exact output format are in the [spec](https://github.com/hackathon-raptors/speed-demon-2026/blob/main/specs/d-traverse.md).

## Approach

Rust implementation of the organisers' forward Dijkstra algorithm. Load a CSR
graph with `u32` offsets and `(u32 target, u32 weight)` edges, retaining every
edge. Reuse a `u64` distance array and binary min-heap across queries, resetting
only touched vertices. Stop when the target is popped, skipping stale entries.
Zero weights, parallel edges, duplicates and isolated nodes are supported.
Queries run sequentially; no routing index or answer cache is used.

Release builds use fat LTO, one codegen unit and abort on panic. Docker builds
also use `target-cpu=native`. Local VPS measurements are recorded in `PERF.md`;
the small measured build-profile improvement remains inconclusive.

Build: `cargo build --release --offline --locked`.
Run: `target/release/traverse-baseline --data /data --out /out`.
The container defaults to `/data` and `/out`.

## Dependencies

None.

## With another 24 hours

Measure loading and query work separately, add degree-based query rejection,
then compare byte parsing and parallel independent queries.
