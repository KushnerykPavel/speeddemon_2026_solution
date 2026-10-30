# Track A: Ingest

The task, the input files and the exact output format are in the [spec](https://github.com/hackathon-raptors/speed-demon-2026/blob/main/specs/a-ingest.md).

## Approach

Rust baseline port of the organisers' Python reference. Read and split each log
line, group latencies by UTC hour and endpoint in a hash map, sort each group's
latencies, and emit nearest-rank percentiles and integer error rates. Sort the
group keys before writing output. No parallelism, custom parser or selection
algorithm yet. Since the spec guarantees fixed-width valid UTC timestamps, the
first thirteen timestamp bytes identify the hour without a date dependency.

Build: `cargo build --release --offline --locked`.
Run: `target/release/ingest-baseline --data /data --out /out`.
The container defaults to `/data` and `/out`.

## Dependencies

None.

## With another 24 hours

Measure parsing, grouping and sorting separately before changing the baseline.
