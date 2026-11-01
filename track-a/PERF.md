# Track A: performance log

Every step from the reference to your final version, each with the live-board run ID that measured it. Judges check that the run IDs exist and that each change moved the way you say it did.

| Step | Change | Run ID | T (s) | M (MB) | Points |
|---|---|---|---|---|---|
| 0 | The reference, unchanged | — | 32.05 | 228 | 0 |
| 1 | Rust baseline port (`55522c4`) | run-735e9cb7363e | 1.397731 | 62.058496 | 38.980249 |
| 2 | Reuse line buffer; iterate fields without a temporary Vec (`3897216`) | run-aa9a80757414 | 1.087126 | 61.673472 | 40.980844 |
| 3 | Inline hour keys; borrowed endpoint lookup (`ff39a9a`) | Not in public history | 0.938535 | 50.634752 | 42.883192 |
| 4 | Buffered byte parser (no board time win) (`a2217a2`) | Not in public history | 0.959642 | 49.434624 | 41.171803 |
| 5 | Intern endpoints; numeric hour/endpoint IDs and vector groups (`531def7`) | run-12ff8c772294 | 0.876075 | 35.188736 | 43.339268 |
| 6 | Hour ID cache + mmap + parallel chunked ingestion (`69aefa2`) | run-6d2d50ab6cf7 | 0.187192 | 53.919744 | 56.776445 |
| 7 | Fixed-layout parser, custom endpoint table, per-hour sorted keys (`14001f9`) | run-7893403a45a6 | 0.072554 | 62.013440 | 66.208615 |
| 8 | SWAR hour/latency, selection quantiles, streamed output (`70c6a36`) | run-2f867ed017e2 | 0.049657 | 46.960640 | 70.622489 |
| 9 | Exact-size hour-bucketed chunks, hash-only endpoint table, in-worker writes (`01bf7d2`) | run-3dde37561ccb | 0.044817 | 46.551040 | 71.672304 |

Official feed generated at **2026-10-31 15:19:48 UTC (16:19:48 Europe/Warsaw)**; history
snapshot generated at 15:19:40 UTC. All 18 published A/D attempts passed.
The table uses the first board measurement of each implementation, not a VPS
or synthetic result. MB means decimal 1,000,000 bytes. These are single live-board
runs; they do not isolate noise or replace final holdout scoring.
`Not in public history` means the result is verified by commit/time, but its Run
ID is omitted from the public compact history. IDs are recorded only when found
in a saved or current full feed; none are inferred.
Raw evidence, all 18 attempts (including unchanged-track reruns), sources and
provenance: [board results](../tests/performance/2026-10-31-board-results/README.md).

Current best: step 5, **0.876075 s / 35.188736 MB / 43.339268 points**.
Latest rerun of the same Track A source at `b8e5bc1`: 0.906580 s / 35.127296 MB /
42.975236 points, `run-b6806a903087`; this is not a new A optimization.
Step 3 reduced time 13.67% and added 1.902348 points versus step 2. Step 4
increased time 2.25% and lost 1.711389 points versus step 3, despite lower memory.
Step 5 reduced time 8.71% and added 2.167465 points versus step 4.

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

## Step 3 — group key allocation, 2026-10-30

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
That VPS experiment did not measure memory or source-size score. The later
official board measurement is recorded in the table above.

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


## Step 6 — most recent hour ID cache, 2026-11-01 (local, kept)

Ingestion is ~92% of processing in a coarse VPS phase diagnostic. A one-entry
`(hour, ID)` cache replaces repeated dictionary lookups with exact comparison;
misses use the unchanged lookup. Arbitrary record ordering remains correct.
Eight Rust source lines added (approximate S including Dockerfile 183 → 191).
No parser, quantile, dependency or compiler change.

Native Linux x86-64 VPS, CPU 2, pinned offline Rust release builds, warm input,
alternating AB/BA order. Full seed 1: 1.18864 → 1.05518 s (−11.23%, ten measured
runs each); repeat 1.23853 → 1.12672 s (−9.03%, six each). Mean after dropping
one min/max per version. Spreads 24.2%/11.4% and 8.7%/19.4%, so the precise
size remains uncertain. Sample seeds 7/42 improved 11.6%/12.8%. Synthetic
alternating-hour input regressed 9.7%, then 4.0% in repeat; shuffled input moved
−2.8% (noise), constant-hour input −16.5%. Kept for observed locality across
seeds, with the miss-heavy tradeoff recorded. No universal speedup claim.

Separate peak RSS: 26.28–26.50 MB before, 26.35–26.50 MB after (three each),
no material change; not cgroup memory.peak. Initial driver RSS inherited its
own high-water mark and is excluded. At unchanged M, observed T and approximate
S imply K ×1.0546 / ×1.0417. No official score or board Run ID yet.

All twelve official/synthetic cases match Python; full output retains SHA-256
`495ca09682b9673ef836a506496177680bb4a3af54255174befddb1f83481cd3`.
Every benchmark output is checked. Nine optimized Rust tests, formatting and
Clippy pass. Added tests cover cache hits/misses/revisits and invalid input
preserving state; three independent design reviews approved. No commit/push.
[Ten-hypothesis ranking](../docs/optimization-plateau-2026-11-01.md) and
[complete experiment evidence](../tests/performance/2026-10-31-ingest-hour-cache/README.md).

## Step 7 — mmap + parallel chunked ingestion, 2026-11-01 (local macOS check, not yet on board)

Plan: [top-10 plan](../docs/track-a-top10-plan-2026-11-01.md). Input is mapped read-only and
split into up to 12 newline-aligned chunks; scoped threads aggregate each chunk into private
groups with the unchanged per-record code. Endpoint and hour lists are merged by sorted union and
every hour is finalized in parallel (concatenate the workers' latencies for each group, sort,
format); hour texts are written in order and the process exits without freeing. THP off and
`mallopt` as in Track D. `--threads N` overrides the worker count.

Local Apple-silicon check only (full seed 1, warm cache): 0.14–0.19 s at 6–10 threads,
`user` 0.56–0.65 s. Output sha256 `495ca096…81cd3` identical at 1/2/6/10 threads; tiny/sample
seeds 1/7/42, edge cases and empty input equal the Python reference. Ten Rust tests pass
(new: chunk coverage, ordered parallel results), Clippy clean. Anon memory about 40 MB (RSS
including the mapped input is 280 MB). VPS sweep, cgroup memory and the board run ID are still
to be recorded; no board claim is made.

### A7 VPS perf profiling — 2026-11-01

Profiled current `69aefa2355f6166a7e20b83fd5bb9b02eaa28e34`, source SHA-256
`7a81b23a5c4ce5d98da7d3a978fe2c2e33661a4b07808a773725399cb60cd286`,
without changing implementation. Shared six-vCPU Haswell KVM, affinity 0–5,
pinned offline Rust builds, warm full seed 1. Five unchanged-release runs/count,
alternating forward/reverse worker order; trimmed means: 1 worker **1.193228 s**,
2 **0.670827 s**, 6 **0.294095 s**, 12 **0.275626 s**. Whole process including
output; container startup excluded. Six workers scale about 4.06× over one;
six-versus-twelve is inconclusive amid 11–24% spreads and VPS oversubscription.
This is within-A7 scaling, not a paired comparison with the serial cache source.

Hardware PMU unavailable; used software perf counters and CPU-clock user
sampling at 499 Hz over 15 runs each at one/six workers. Initial DWARF stacks
were incomplete; retained as excluded evidence, followed by diagnostic debug=1
builds with frame pointers/unwind tables. Accepted flamegraphs contain 9,044 /
10,114 solution samples, zero lost/empty callchains. At six workers, observed
inclusive CPU shares: ingestion 81.3%, parsing 35.3%, UTF-8 validation 8.6%,
SipHash 12.3%, sorting 4.2%, formatting 3.1%. Categories overlap and represent
user CPU, not wall-time phases. Frame-pointer builds change code generation;
release timing above was measured separately with unchanged flags.

Ten Rust tests and 18 exact reference comparisons pass (nine cases, one/six
workers), with every timed/profiled output checked. Full hash unchanged.
Separate diagnostic RSS 269,536 / 272,904 KiB includes mapped input; no cgroup
peak or canonical S measurement, so no K/board claim. Next recommended isolated
experiment: fixed-layout scalar parser; no implementation or worker default
changed. [Raw evidence, commands, limitations and interactive flamegraphs](../tests/performance/2026-11-01-ingest-perf-flamegraphs/README.md).

### A7 native hardware profiling on rl-lyceum — 2026-11-01

Copied the unchanged `69aefa2355f6166a7e20b83fd5bb9b02eaa28e34` source
(SHA-256 `7a81b23a5c4ce5d98da7d3a978fe2c2e33661a4b07808a773725399cb60cd286`)
into `/home/rabat50/speed-demon-track-a-Eh6ThXY0` on `rl-lyceum`. Built and ran
natively without Docker using existing root Rust 1.99.0. i7-7700, four cores /
eight threads, affinity 0–7, kernel 4.15.0-142, glibc 2.23, perf 4.15.18.
Ten Rust tests and 36 exact reference comparisons passed (nine cases at
1/4/8/12 workers); full output hash unchanged. Source and existing work preserved.

Five uninstrumented warm full-seed-1 runs/count, alternating forward/reverse
1/2/4/8/12 order: medians **0.739274 / 0.361952 / 0.221070 / 0.178939 /
0.230746 s**. Spawn-to-exit includes output; comparison excluded. Shared host,
no CPU/frequency isolation; substantial outliers. Eight workers fastest observed;
no default change or official performance claim.

`sudo perf stat -r 3` succeeds for cycles, instructions, branches/misses,
cache references/misses, L1 load/miss and dTLB misses, plus software counters.
All selected counters counted. One/eight-worker user IPC **2.21 / 1.38**, branch
misses **2.08% / 2.08%**. Root counter runs are separate from unprivileged timing.
Single max-RSS observations 269,408 / 270,928 / 273,276 KiB at 1/4/8 workers
include mapped input, not cgroup peak. No K estimate.

Separate debug=1/frame-pointer build: `cycles:u`, requested 499 Hz, 25 runs at
one/eight workers, every output hash checked. Zero reported lost samples;
8,243 / 14,571 solution samples; unknown frames affect 2.35% / 10.29% of stacks,
so attribution is limited. Eight-worker flat physical self shares include
SipHash write 8.84%, HashMap get 8.80%, UTF-8 validation 8.65%, quicksort 3.99%.
Inlining folds much parsing into worker wrappers. No optimization attempted.
[Commands, raw evidence, conditions and readable flamegraphs](../tests/performance/2026-11-01-track-a-rl-lyceum/README.md).

## Steps 6–9 summary — 2026-11-01 (board results)

Board S rose 183 → 287 → 310 → 410 → 423 (formatted lines). Rank 102 → 91 → 72 → 66 → 64 of 111;
the top-10 cutoff was about 84.4 points (T ≈ 0.0215 s, M ≈ 19 MB, S ≈ 334).
Native Linux x86 (rl-lyceum, single thread) for step 9: 268 ms task-clock, 1.95 G instructions
(2.19 IPC), 1.9% branch misses; about 74% of instructions are the parse phase. Memory did not
fall with exact-size chunks (46.6 MB): the 10 MB output page cache and 24.8 MB of 8-byte keys
dominate. Endpoints are identified by a 64-bit hash of all bytes without a byte comparison
(measured about 10% of single-thread parse); the plan is in
[top-10 plan](../docs/track-a-top10-plan-2026-11-01.md). Next ideas, untested: 6-byte records,
O_DIRECT output, SIMD endpoint scan, trimming S.
