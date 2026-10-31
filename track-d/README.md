# Track D: Traverse

The task, the input files and the exact output format are in the [spec](https://github.com/hackathon-raptors/speed-demon-2026/blob/main/specs/d-traverse.md).

## Approach

Rust implementation of exact point-to-point shortest paths with bidirectional
Dijkstra. Load a CSR graph with `u32` offsets, retaining every input edge
(duplicates included), and build its reverse CSR in parallel. Each edge is
packed into one `u32` (endpoint in the low bits needed for `n - 1`, weight above);
weights too large for the remaining bits keep an escape code and their exact value
in a sorted `(edge index, weight)` list. Each query
searches forward from the source and backward from the target, always growing
the side with the smaller queue (choosing by smaller key would let one side
flood the graph when the other side's first edge weighs close to 2^32). The
best path length `μ` is updated whenever an edge reaches a vertex the other side
has labelled; the search stops once the two queue minima sum to at least `μ` or
a side is exhausted. Distances are `u32` in 1,024-vertex pages assigned on first
write and recycled after each query, so worker memory follows the largest search
rather than the vertex count. Relaxations that do not fit `u32` are skipped; a
found distance below `u32::MAX` is still exact. Otherwise, if anything was
skipped, the query reruns in one `u64` search shared by all workers behind a
mutex (4 of 2,000 seed-1 answers exceed `u32`).
Zero weights, parallel edges, duplicates and isolated nodes are supported.
Before searching, return zero for identical endpoints and reject queries with
no outgoing edge at the source or no incoming edge at the target (an empty row
of the reverse graph).
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
Each scoped worker parses its chunk once and buckets every edge by the part that
owns its source (a power-of-two block of vertex IDs, about eight per worker).
Each part then counts and places its own edges into its own `split_at_mut` slices
of the CSR: a cache-sized region, plain writes, no atomics. The temporary
buckets hold one `(u, v, w)` triple per edge and are freed after placement, below
the search-phase memory peak. The mapping is released after loading. Every edge
is retained. Loading uses the same worker limit as query processing.

Plain `u v w` records are parsed with one 8-byte load per number (SWAR: length
from the first byte with bit 4 clear, a nibble check for digits, three
multiplies); any other shape falls back to the strict parser. Graph fields are
parsed directly from borrowed ASCII byte slices; queries retain
a reusable line buffer and share the checked `u32` parser. No temporary strings
or field vectors. Overflow, field counts, vertex bounds, LF/CRLF and final lines
without a newline are handled. The std-only mapping wrapper targets 64-bit Unix
(the Linux competition target and macOS development host); input files remain
immutable during loading, as with the read-only competition data mount.

Before searching, relabel vertices for locality. Every worker grows breadth-first
regions from unclaimed roots in its own slice of old IDs and claims each vertex
with a compare-and-swap of a provisional ID (worker and position). Edges are
written with provisional heads during the walk, so the old graph is read once;
a parallel pass concatenates the regions into the new CSR through disjoint
`split_at_mut` slices. Region buffers reserve the whole graph up front: only
written pages are charged and they never reallocate. All vertices and duplicate
edges remain; exact distances and query order are preserved. Empty queries skip
the rebuild. Idea credit: Team7's public Track D renumbering (MIT).
On the six-vCPU VPS, renumbering takes 0.11 s instead of 0.42 s for the serial
BFS order, with unchanged search time. See PERF.md for measurements.

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

Locate the new memory peak (loading or renumbering) and shrink its transient
buffers, then compare 6 versus 12 workers on the competition machine using time
and cgroup peak memory.
