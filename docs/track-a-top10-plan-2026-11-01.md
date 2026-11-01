# Track A: top-10 plan (A6, A7 detailed)

Date: 2026-11-01. Board snapshot 2026-11-01 05:44 UTC, Track A, 111 teams.

## Gap

| | T (s) | M (MB) | S | Points |
|---|---:|---:|---:|---:|
| #1 Cause Of Death | 0.0161 | 17.3 | 346 | 87.76 |
| **#10 Cache Me (cutoff)** | **0.0215** | **18.9** | **334** | **84.30** |
| Us (step 5) | 0.876 | 35.2 | 183 | 43.34 (rank 102) |

Points = 50·log10(K). Cutoff K ≈ 48.5, ours 7.36. Need ~40x less time, M ≤ ~20 MB, S ≈ 300–350.
Top teams' techniques are inferred from these numbers only; their code was not seen.

## Lessons from Track D that apply

- Structural changes paid (parallel mmap load, layout, memory diet). Micro-tweaks (SWAR, blobs, PGO, Dial) did not show on the board.
- Board uses 2 MB THP: `prctl(PR_SET_THP_DISABLE)` and `mallopt(M_MMAP_THRESHOLD)` keep M honest.
- 12 workers (6 P-cores, SMT). Board noise about ±4%; resample before judging gains under 10%.
- S matters: ~+0.6 points for 150 lines (D step 20).

## Roadmap

| Step | Change | Target T |
|---|---|---|
| A6 | Last-hour ID cache (already measured on VPS, −9..−11%) | 0.8 s |
| A7 | mmap + 12 newline-aligned chunks, thread-local groups, parallel merge/finalize | ~0.10 s |
| A8 | Fixed-layout parser: hour index from bytes, skip method, backwards latency | ~0.04 s |
| A9 | Custom endpoint table (first 16 bytes + length hash, full compare) | ~0.03 s |
| A10 | Memory layout: pairs vs count/scatter arena (M ≤ 20 MB) | M ≤ 20 MB |
| A11 | Parallel select/format, hand itoa, one write | ~0.022 s |
| A12 | Tune workers/chunks, resample, trim S | ≤ 0.02 s |

Each step: byte-identical `summary.txt` (full seed 1 sha256
`495ca09682b9673ef836a506496177680bb4a3af54255174befddb1f83481cd3`), tiny/sample seeds 1/7/42,
edge cases, then a board run and a PERF.md row with the run ID.

## A6: hour ID cache

One-entry `(hour, id)` cache before the hour dictionary. Fallback is the unchanged lookup, so
arbitrary record order stays correct. Already implemented and VPS-measured (full seed 1
1.189 → 1.055 s, repeat 1.239 → 1.127 s; synthetic alternating-hour input +4..+10%).
Evidence: `tests/performance/2026-10-31-ingest-hour-cache/`. Status: in working tree, not
committed, no board run yet. A8 replaces it with a direct hour index; A6 only provides a
board-measured baseline and a safe intermediate commit.

## A7: parallel mmap ingestion

Pipeline:

1. `tune_memory()` (THP off, mallopt) on Linux.
2. `mmap` `requests.log` read-only, private; the data mount is read-only so it cannot change.
3. Split into up to 12 chunks at newline boundaries (`line_chunks`, same as D). Empty file: no chunks.
4. Scoped threads each run the existing per-record code (`add_record`) over one chunk into a
   private `Groups` (own endpoint dictionary, hour dictionary, rows). Errors: first error wins.
5. Global sorted endpoint list = union of worker dictionaries; `remap[worker][rank]` = local
   endpoint id (or none). Global sorted hour list = union of worker hours.
6. Finalize in parallel over hours (atomic counter): for each endpoint rank, concatenate that
   group's latencies from every worker into one reusable buffer, `sort_unstable`, compute
   count/p50/p95/p99/errors/ppm, format into the hour's byte buffer. No serial merge.
7. Write hour buffers in order, `exit(0)` immediately (skip drops and munmap).

Semantics preserved: LF/CRLF, final line without newline, blank line = error, five fields,
overflow checks, empty groups omitted, byte order of endpoints, nearest-rank percentiles.
`--threads N` (capped at 12) for worker sweeps.

Known costs accepted for A7 (fixed in A10): per-worker `Vec` growth plus the concatenation buffer
raises M above the serial 35 MB; per-record cost is unchanged (A8/A9).

Acceptance: sha256 equal on full seed 1; sample/tiny seeds 1/7/42 and edge cases equal to the
Python reference; Rust tests pass for chunk coverage, errors from any chunk, and equality of
output across worker counts 1/2/3/12; VPS sweep at 1/2/6/12 workers before the board run.

Risks: chunk boundary splits, endpoint present in only some workers, hour present in only some
workers, thread error propagation, memory peak from per-worker doubling.
