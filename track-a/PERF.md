# Track A: performance log

Every step from the reference to your final version, each with the live-board run ID that measured it. Judges check that the run IDs exist and that each change moved the way you say it did.

| Step | Change | Run ID | T (s) | M (MB) | Points |
|---|---|---|---|---|---|
| 0 | The reference, unchanged | | 32.05 | 228 | 0 |
| 1 | Rust baseline port (`55522c4`) | run-735e9cb7363e | 1.397731 | 62.058496 | 38.980249 |
| 2 | Reuse line buffer; iterate fields without a temporary Vec (`3897216`) | run-aa9a80757414 | 1.087126 | 61.673472 | 40.980844 |
| 3 (local candidate) | Inline hour keys; borrowed endpoint lookup | Pending | | | |

The Rust baseline preserves grouping and sorting; fixed-width UTC timestamps
identify the hour without a date dependency. No external dependencies.
VPS tiny/sample/full outputs match Python byte-for-byte. Validation evidence
is in `../tests/BASELINE_VALIDATION.md`.

## Step 2 — 2026-10-30

Replace `BufReader::lines()` with `read_line()` and one reusable `String`;
replace `split(' ').collect::<Vec<_>>()` with an iterator. Preserve the exact
five-field check, LF/CRLF handling and final line without a newline.
Grouping, key allocation, percentile sorting and output formatting are unchanged.

Live board: time fell by 22.22% (1.2857x speedup), memory fell by 385,024 bytes,
and the score increased by 2.000595 points. S increased from 69 to 87; D stayed 1.
K increased from 6.020118 to 6.601109. Both runs passed all gates.
MB in the table means decimal 1,000,000 bytes. Board feed snapshot:
2026-10-30 15:32:11 UTC (16:32:11 Europe/Warsaw).
These are live-board runs, not the final holdout scoring protocol.

Local VPS comparison on full seed 1: 2.130127492 s before vs 1.669489579 s after,
1.2759x speedup / 21.62% less time. Five measured runs per version after warmup,
alternating order, pinned to logical CPU 2; mean of the middle three.
Process startup/exit included, Docker startup excluded. No local memory
measurement; noticeable run-to-run variation on the shared VPS.

Output matched byte-for-byte on every VPS run, with official full seed-1 SHA-256
`495ca09682b9673ef836a506496177680bb4a3af54255174befddb1f83481cd3`.
Local release build, formatting and Clippy passed; Python comparisons passed
for existing edge cases, empty input, CRLF and no final newline. Missing and
extra fields were rejected. The optimized version did not rerun the entire
tiny/sample multi-seed matrix during this step.

All raw timings, conditions, source hashes, exact timing runner, board evidence
and reproduction instructions are preserved in
[`tests/performance/2026-10-30-ingest-buffer`](../tests/performance/2026-10-30-ingest-buffer/README.md).

## Step 3 — group key allocation, 2026-10-30 (local candidate)

Replace `HashMap<(String, String), Group>` with an hour map keyed by `[u8; 13]`,
each value holding an endpoint map. Copy the hour inline; look up existing
endpoints by borrowed `&str`. Allocate an endpoint string only on a new group.
All calendar hours remain data-derived; no date-range or endpoint catalogue
is hard-coded. Flatten the maps and retain sorting by hour then endpoint bytes.
Percentile sorting, parsing and output formatting are otherwise unchanged.

Paired VPS comparison against `3897216`, full seed 1:
1.641332929 s before vs 1.295608003 s after, 1.26684x speedup / 21.06% less time.
Same protocol as step 2: five measured runs per version after warmup,
alternating order, CPU 2, process time including startup/exit, excluding Docker
startup. Run-to-run variation remains noticeable on the shared VPS.
Do not compare absolute times across the two separate experiments.
Memory and source-size score were not measured; board run ID and score pending.

All 12 VPS executions matched each other byte-for-byte and the official full
seed-1 SHA-256. Local Python comparisons passed existing edge cases, empty,
CRLF, no final newline and three synthetic 20,000-row inputs (seeds 1/7/42;
these are not the official generator matrix). Release build, formatting and
Clippy passed. No unsafe code or dependencies were added.

Raw timings, exact runner, source hashes and the candidate patch are archived in
[`tests/performance/2026-10-30-ingest-group-keys`](../tests/performance/2026-10-30-ingest-group-keys/README.md).
