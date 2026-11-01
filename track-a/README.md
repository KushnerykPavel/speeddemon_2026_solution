# Track A: Ingest

The task, the input files and the exact output format are in the [spec](https://github.com/hackathon-raptors/speed-demon-2026/blob/main/specs/a-ingest.md).

## Approach

Rust port of the organisers' Python reference. The log is mapped read-only and split into up
to 12 newline-aligned chunks. Scoped std threads parse their chunk byte-wise (no temporary
strings), intern endpoints, assign numeric hour IDs (with a most-recent-hour cache) and keep
per-hour vectors indexed by endpoint ID. After the join, endpoint names and hours are merged by
sorted union and each hour is finalized on a worker: that hour's latencies from all chunks are
concatenated per endpoint, sorted, and nearest-rank percentiles and integer error rates are
formatted. Hour texts are written in order and the process exits without freeing. Linux THP is
disabled and glibc's mmap threshold fixed so memory returns to the kernel. LF, CRLF and a final
record without newline are supported; no endpoint catalogue or calendar range is hard-coded.
No selection algorithm or SIMD yet. The spec guarantees fixed-width valid UTC timestamps.

Build: `cargo build --release --offline --locked`.
Run: `target/release/ingest-baseline --data /data --out /out` (`--threads N` overrides the worker count, capped at 12).
The container defaults to `/data` and `/out`.

## Dependencies

None.

## With another 24 hours

Fixed-layout parsing with a direct hour index, a custom endpoint table, a contiguous latency
arena with exact selection and hand-written number formatting (see the top-10 plan in `docs/`).
