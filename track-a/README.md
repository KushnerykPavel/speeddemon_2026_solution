# Track A: Ingest

The task, the input files and the exact output format are in the [spec](https://github.com/hackathon-raptors/speed-demon-2026/blob/main/specs/a-ingest.md).

## Approach

Rust port of the organisers' Python reference. Scan byte records directly in a
64 KiB read buffer using `fill_buf`/`consume`; copy only records crossing buffer
boundaries into one reusable byte vector. Find field separators without temporary
strings and parse decimal status/latency with overflow checks. Support LF, CRLF,
and a final record without a newline. Intern each distinct endpoint once and assign
stable numeric IDs to endpoints and UTC hours. Store groups in per-hour vectors
indexed by endpoint ID; grow each row independently as new endpoints arrive.
Sort only the unique hours and endpoint names for output, skipping empty groups.
Sort each group's latencies and emit exact nearest-rank percentiles and integer
error rates. No endpoint catalogue or fixed calendar range is hard-coded.
No parallelism, memory mapping or selection algorithm yet. The spec guarantees
fixed-width valid UTC timestamps, so no date dependency is needed.

Build: `cargo build --release --offline --locked`.
Run: `target/release/ingest-baseline --data /data --out /out`.
The container defaults to `/data` and `/out`.

## Dependencies

None.

## With another 24 hours

Measure parsing, grouping and sorting separately; consider parallel parsing,
a contiguous latency arena and exact selection for large groups.
