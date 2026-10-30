# Track A: performance log

Every step from the reference to your final version, each with the live-board run ID that measured it. Judges check that the run IDs exist and that each change moved the way you say it did.

| Step | Change | Run ID | T (s) | M (MB) | Points |
|---|---|---|---|---|---|
| 0 | The reference, unchanged | | 32.05 | 228 | 0 |
| 1 | Rust baseline port (`55522c4`) | run-735e9cb7363e | 1.397731 | 62.058496 | 38.980249 |
| 2 | Reuse line buffer; iterate fields without a temporary Vec (`3897216`) | run-aa9a80757414 | 1.087126 | 61.673472 | 40.980844 |
| 3 (local candidate) | Inline hour keys; borrowed endpoint lookup | Pending | | | |
| 4 (no measured win) | Buffered byte parser (`a2217a2`); Linux comparison archived | Pending | | | |
| 5 (local candidate) | Intern endpoints; numeric hour/endpoint IDs and vector groups | Pending | | | |

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

## Step 4 — buffered byte parser, Linux check 2026-10-30

Measured current Track A at repository revision `f70b655` against the exact archived
pre-parser source. Both versions already include the same group keys,
`sort_unstable`, release profile and lockfile, isolating the parser change.
Native Linux x86-64 VPS, pinned CPU 2; identical pinned Rust build toolchain.

Two batches, each one warmup plus five measured runs per variant, alternating
order and dropping min/max: 1.270541 -> 1.313733 s (+3.40% time), then
1.269154 -> 1.283111 s (+1.10%). The repeat was triggered by >5% spread.
Measured spreads remain 15.6–23.8%: no end-to-end speedup is demonstrated;
the exact size of any regression is uncertain. No memory benefit established.
These native-host times are not official board measurements or container-limit checks.

All nine official-data/edge cases matched Python byte-for-byte, including
tiny/sample seeds 1/7/42 and full seed 1; published sample/full hashes matched.
Both batches' warmups and timed runs matched the full reference output.
Six optimized parser regression tests passed on Linux, including buffer split,
LF/CRLF, numeric, long-record and read-failure cases.

The parser remains implemented but is not credited as a speed optimization.
No rollback, commit or push was part of this check. Raw results, exact source
snapshots, hashes, compiler/CPU details, runner and reproduction procedure:
[`tests/performance/2026-10-30-ingest-byte-parser-linux`](../tests/performance/2026-10-30-ingest-byte-parser-linux/README.md).

## Step 5 — endpoint interning and numeric groups, 2026-10-30

Keep the parser, build profile and latency sorting unchanged. A global endpoint
dictionary stores each endpoint once; a separate hour dictionary assigns stable
numeric IDs. Each hour owns a vector indexed by endpoint ID, extended lazily.
IDs do not move when new endpoints/hours appear. Sort unique hours and endpoints
for output rather than sorting all composite group keys; omit empty combinations.
No fixed endpoint count, catalogue, date range or dependency was introduced.

Against the parser at `f70b655`, two native Linux x86-64 VPS batches gave
1.291992 -> 1.216720 s (5.83% less time) and 1.314827 -> 1.265930 s
(3.72% less time). Each batch: warmup plus five measured executions per version,
alternating order, CPU 2, identical release builds, mean after removing min/max.
Spreads were 23.5%/27.6% then 9.2%/15.8%; this is a small observed improvement,
not an established precise speedup or official score. No memory benefit established.

Both batches matched the preserved Python output on all nine official-data/edge
cases, including tiny/sample seeds 1/7/42 and full seed 1. Reference/input/output
hashes were checked before reusing the prior Linux reference cache. All warmups
and timed runs matched the full output bytes and published SHA-256.
Seven regression tests passed locally and on Linux, including endpoint discovery
after earlier hours, revisiting existing IDs, sparse groups and buffer boundaries.
Formatting, Clippy and the local release build passed.

The first attempt at batch 2 failed before any measurement due to an incorrect
reference-cache path; its log is retained. The retry used the original verified
reference cache. No failed data was included in either timing summary.
Raw logs, exact before/after source and manifests, hashes, CPU/compiler details,
runner, failure and retry procedure are archived in
[`tests/performance/2026-10-30-ingest-dense-groups`](../tests/performance/2026-10-30-ingest-dense-groups/README.md).
