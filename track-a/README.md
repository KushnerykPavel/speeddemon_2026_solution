# Track A: Ingest

The task, the input files and the exact output format are in the [spec](https://github.com/hackathon-raptors/speed-demon-2026/blob/main/specs/a-ingest.md).

## Approach

Rust port of the organisers' Python reference. Reuse one line buffer and split
fields with an iterator. Group latencies in a hash map of UTC hours, each holding
an endpoint map. The hour key is thirteen inline timestamp bytes; endpoint strings
are allocated only for new groups, and existing groups are looked up using a
borrowed string. Sort each group's latencies and emit exact nearest-rank
percentiles and integer error rates. Sort the group keys before writing output.
No parallelism, custom parser or selection algorithm yet. The spec guarantees
fixed-width valid UTC timestamps, so no date dependency is needed.

Build: `cargo build --release --offline --locked`.
Run: `target/release/ingest-baseline --data /data --out /out`.
The container defaults to `/data` and `/out`.

## Dependencies

None.

## With another 24 hours

Measure parsing, grouping and sorting separately; compare `sort_unstable` with
the current stable sort, then consider exact selection for large groups.
