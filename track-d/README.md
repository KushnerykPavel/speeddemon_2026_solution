# Track D: Traverse

The task, the input files and the exact output format are in the [spec](https://github.com/hackathon-raptors/speed-demon-2026/blob/main/specs/d-traverse.md).

## Approach

Rust baseline port of the organisers' Python reference. Load a vector of
per-node adjacency lists, retaining every edge. For each query, run a fresh
forward Dijkstra search with a hash map of distances and a binary min-heap.
Stop when the target is popped, skipping stale entries. Distances use 64 bits;
zero weights, parallel edges, duplicates and isolated nodes are supported.
No preprocessing, caching or parallelism yet.

Build: `cargo build --release --offline --locked`.
Run: `target/release/traverse-baseline --data /data --out /out`.
The container defaults to `/data` and `/out`.

## Dependencies

None.

## With another 24 hours

Measure loading and query work separately before changing the baseline.
