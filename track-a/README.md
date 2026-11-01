# Track A: Ingest

The task, the input files and the exact output format are in the [spec](https://github.com/hackathon-raptors/speed-demon-2026/blob/main/specs/a-ingest.md).

## Result

Board best: `run-7fbd8d58705c`, 0.029519 s, 20.2 MB, S 516, no dependencies, 79.18 points. Board runs
of one binary vary by about 10% in time. Every step, its run ID and the
rejected experiments are in [PERF.md](PERF.md).

## Approach

Rust, standard library only, one pass over the mapped log, then two short parallel phases. The design
follows Cache Me's public Track A solution (techniques only, rewritten).

1. **Parse.** `requests.log` is mapped read-only and split into newline-aligned pieces, one per thread
   (up to 12). The file is mapped over a zero-filled reservation, so the parser may read past the end.
   Line ends are found 64 bytes at a time: two 32-byte AVX2 compares, with SSE2 and scalar
   fallbacks. Each line is parsed backwards from its end, latency, status and endpoint, with fixed-layout
   offsets and word-sized loads. The spec guarantees well-formed input, so no per-line validation is done.
   The hour comes from the fixed-width timestamp (72 hours, a direct index), with a cache for runs of the
   same hour. Endpoints go into one lock-free table shared by all threads. The slot is claimed by a 64-bit
   hash of the whole name, and the slot number is the name's id. Each record becomes one `u32`: latency
   above a 13-bit key (12-bit slot, 5xx flag); latencies of 2^19 or more go on a side list. Records are
   appended to 16 KB blocks per hour. Parsed input pages are dropped with `madvise(MADV_DONTNEED)`, and
   blocks are handed back as soon as their hours are finished.
2. **Hours.** Each hour runs on a worker:
   - a counting sort of its records by endpoint rank (byte order, the output order);
   - three selections per group for the nearest-rank p50, p95 and p99;
   - integer error rates.
   
   Each group's numbers are kept together with the exact size of their text. The records are freed before
   any output page is charged.
3. **Text.** The output file is sized from the exact line sizes and mapped. Every thread formats its hours
   straight into its own disjoint range. The process exits without freeing.

Memory: Linux transparent huge pages are disabled, and glibc's mmap threshold is fixed so freed blocks
return to the kernel. The board's `/out` is a tmpfs, so the 10 MB output counts towards the peak; that
is why records are freed first.

The Docker build uses `-C target-cpu=haswell`, so the binary needs AVX2/BMI2 (any x86 since 2013).

Build: `cargo build --release --offline --locked`.
Run: `target/release/ingest-baseline --data /data --out /out` (`--threads N` overrides the worker count,
capped at 12). The container defaults to `/data` and `/out`.

## Dependencies

None.

## What did not work

These were measured; details are in PERF.md.
- Three-cursor interleaving: +14% instructions.
- Selecting p50 first: no fewer instructions.
- Fusing the hour and text phases: +7.4 MB.
- A native-CPU build before the AVX2 path: mixed sign within noise.

## With another 24 hours

The leaders run in about 0.016 s and 16–17 MB with S 315–371. Our parse is 265 instructions per record,
spread over hashing, lookup, number parsing and grouping, with no single hot loop. The next step would be
a shorter parse path per record: fewer passes over each line, and a hash computed from the words already
loaded. It would need fewer source lines as well.
