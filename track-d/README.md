# Track D: Traverse

The task, the input files and the exact output format are in the [spec](https://github.com/hackathon-raptors/speed-demon-2026/blob/main/specs/d-traverse.md).

## Result

Board, 21 runs of the final code: T 0.2069 ± 0.0069 s, M 79.6 MB, S 723, no dependencies,
64.95 ± 0.35 points. Best sample: `run-5857d62f5472`, 0.198704 s, 79.2 MB, 65.41 points. Every step, its run ID and the
rejected experiments are in [PERF.md](PERF.md).

## Approach

Rust, standard library only. Exact point-to-point shortest paths with bidirectional Dijkstra over a
renumbered, packed CSR graph. Five phases, each on scoped worker threads (available parallelism, capped
at 12; `--threads N` overrides it):

1. **Load.** `graph.txt` is mapped read-only with `mmap` and kept mapped for the run. It is split into
   newline-aligned chunks, one per worker. Plain `u v w` records are parsed with one 8-byte load per
   number (SWAR: the length from the first byte with bit 4 clear, a nibble check for digits, three
   multiplies). Any other shape falls back to a strict field parser, so overflow, field counts, vertex
   bounds, CRLF and a missing final newline are still handled. Each worker buckets its edges by the part
   (a power-of-two block of vertex IDs, about eight per worker) that owns the source. Bucket records are
   8 bytes: the source and the packed target and weight, with escaped weights kept in a side list. Each
   part then counts and places its own edges into disjoint `split_at_mut` slices of the CSR: a cache-sized
   region, plain writes, no atomics. Every input edge is kept, duplicates and self-loops included.
2. **Packed edges.** Each edge is one `u32`: the other endpoint in the low `ceil(log2 n)` bits, the weight
   above it. A weight that does not fit is stored as the all-ones escape code, with its exact value in a
   sorted `(edge index, weight)` list (3,496 of 5,000,000 edges on seed 1).
3. **Renumber for locality.** Every worker grows breadth-first regions from unclaimed roots in its own
   slice of old IDs. It claims each vertex with a compare-and-swap of a provisional ID (worker and
   position) and notes its degree. The regions, one after another, give the new order. A parallel pass
   over equal pieces of that order then copies the rows from the old graph into the new CSR, translating
   heads through the claims. Software prefetch asks for rows a few vertices ahead. Without this step the
   run is 2.3× slower. Idea credit: Team7's public Track D renumbering (MIT).
4. **Reverse graph.** In-degrees are counted with atomics, then each edge is scattered through atomic
   cursors into a reverse CSR with the same packing.
5. **Queries.** Workers claim queries from an atomic counter; answers are stored by index, so the output
   keeps input order. `s == t` returns 0. A source with no out-edge, or a target with no in-edge (an empty
   reverse row), returns `-1` without a search. Otherwise the search runs forward from `s` and backward from
   `t`. It always grows the side with the smaller queue: choosing by the smaller key would let one side
   flood the graph when the other side's first edge weighs close to 2^32. The best path length `μ` is
   updated whenever an edge reaches a vertex the other side has labelled. The search stops once the two
   queue minima sum to at least `μ`, or when a side is exhausted. Heaps hold one `u64` key, the distance
   shifted above the vertex. Distances are `u32` in 1,024-vertex pages, assigned on first write and
   recycled after each query, so worker memory follows the largest search. A relaxation that does not fit
   `u32` is skipped. An answer below `u32::MAX` is still exact; otherwise the query reruns in one `u64`
   search shared by all workers behind a mutex (4 of 2,000 seed-1 answers exceed `u32`).

Memory: on Linux the process asks for 4 KB pages (`prctl(PR_SET_THP_DISABLE)`), because the reserved and
paged buffers are touched sparsely. With 2 MB pages one touched byte commits 2 MB, and the board measured
132 → 102 MB. `mallopt(M_MMAP_THRESHOLD, 256 KB)` returns large freed buffers to the kernel. The peak
(≈ 79 MB) is at the end of load placement, when the buckets and the first CSR are both live.

No routing index or answer cache is used, and nothing is computed ahead of the run. Output is byte-equal
to the Python reference on full seed 1 (SHA-256 `a23dbdf0…277e`) and on the sample, tiny and edge-case
datasets.

Release builds use fat LTO, one codegen unit and abort on panic. The Docker build adds
`target-cpu=native`.

Build: `cargo build --release --offline --locked`.
Run: `target/release/traverse-baseline --data /data --out /out`.
The container defaults to `/data` and `/out`. For a serial comparison, append `--threads 1`.

## Dependencies

None.

## What did not work

These are measured; details and evidence are in PERF.md.
- **Other priority queues:** three Dial bucket rings, plus a radix heap tried as a local prototype only
  (search 7–30% slower on an M1 Pro). The search is bound by memory latency, not by the heap.
- **Profile-guided optimization** made no difference.
- **Pruning pushes by `μ`** cut pushes 5.7% but no settles.
- **Smaller BFS "blobs"** for the renumbering gave no gain.
- **Huge pages only for the final graph arrays** were −4.7% on the VPS, but the board samples came out
  slower and used more memory.
- **Contracting two-way degree-2 vertices (31.8% of edges) and dead-end spurs** is exact and cut the search
  by 21%. Finding the chains cost more than that saved: total time was +17%.

## With another 24 hours

The board's time splits about 55% building and 45% searching. Closing the 2× gap to the leaders needs
the building phases to do less random-access work, not a faster search loop. The next steps would be:
- build the reverse graph without atomics, from per-worker inboxes bucketed by target block;
- run the prefix sums and large zero-fills in parallel;
- classify degree-2 chains inside an existing per-edge pass rather than a separate one, so the 21% smaller
  search pays off.
