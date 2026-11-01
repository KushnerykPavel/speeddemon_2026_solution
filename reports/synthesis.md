# Track A review synthesis (2026-11-01)

Inputs: `reports/historian.md`, `reports/profiler.md`, `reports/researcher.md` (written independently).
No code changed in this phase. Nothing pushed or submitted.

## Facts that replace earlier claims
- Best board entry: `run-20d64f688eec`, 78.333 pt, a rerun of the `ea2ee34` source (docs-only commit `be5ed24`;
  tag `best-board-run-20d64f688eec`). Same source first scored 77.236: board T noise is about 9-10%.
- `6d5802b` (AVX2 + haswell): `run-9e5d2dfb4e65`, 78.103 pt, T 0.031695, M 20.56 MB, S 554. Not shown to beat
  the rerun (-0.23 pt). Counters on rl-lyceum do show -6.6% user cycles, -3.6% task-clock (profiler).
- 77.258 / `5878e1d` is not in the feed or any saved JSON: unverified.
- Score levers (scoring.py, profiler): 1 ms T = 0.35 pt, 1 MB M = 0.22 pt, 1 source line = 0.0059 pt
  (earlier docs said 0.014: wrong). S trimming is a weak lever.
- Top-10 cutoff 84.74 pt needs about -37% T; all identified ideas together are +2..+3.5 pt (estimates).

## Bottleneck model (VPS wall, medians; profiler [M])
T = 31.7 ms on the board: parse 57-62%, hours 24-25%, text 7-9%, exec+exit ~4%, fixed/serial ~6 ms.
Kernel ~15% of CPU (10.7k page faults); text phase 37% kernel. Memory: two equal peaks (hours 18.1 MB,
end of text 18.6 MB), fixing one alone saves ~0. Parse is already near the measured IPC/instruction floor of
this design (interleave, two-word hash, AES hash all falsified). The hours phase is the least-optimised phase
and both the profiler and the researcher independently point at its selection step.

## Ranked hypotheses (estimates, not measurements)
| # | Hypothesis | Predicted | Conf. | Effort | Correctness risk |
|---|---|---|---|---|---|
| 1 | Hours selection: take p50 first, then select p95/p99 inside the upper part; small groups sorted in one pass. Compares ~7.6n -> ~4.7n. | -1..-2 ms, +0.4..+0.7 pt | medium | ~1 h, ~0 lines | low: exact percentiles, testable vs reference |
| 2 | Fuse hours and text: format each hour straight into the mapped output; drop `Vec<Row>`, size pre-pass, barrier, one spawn. | +0.5..+1.2 pt | low-medium | 3-4 h | medium: output offsets need earlier hours' sizes |
| 3 | Rely on the spec's well-formed input (drop status/digit/CR validation, 30-45 instr/record, 30-40 lines). | +0.5..+1 pt | low | 2 h plus a spec reading | high: current tests/edge-cases require rejecting malformed input; only valid if the spec says so |

Not recommended: histogram select, AES/CRC hash (falsified), thread reuse, more S trimming.

## Falsification tests and thresholds (hypothesis 1)
Baseline `6d5802b`. Candidate differs only in the selection order.
- Correctness: 7 unit tests; byte-identical output on all datasets incl. the non-public seeds 20261101 and
  987654321 at 1/2/3/6/12 threads. Any difference: revert.
- Keep if: rl-lyceum full-run instructions (1 thread) at least 2% lower than baseline AND 8-thread task-clock
  paired median (>= 10 alternating runs) not worse. Revert otherwise.
- Do not judge on VPS wall time (paired-difference CI about +-12 ms) nor on a single board run (T noise ~10%).

## Recommendation
Exactly one next experiment: hypothesis 1 (selection order). If it fails, STOP: remaining ideas need either a
spec change (3) or a multi-hour rewrite (2) for under +1 pt each, and top 10 is out of reach.
Board submission only on an explicit user instruction, with at least two board runs of the same commit to
average out the ~10% noise.

## Outcome of the recommended experiment (hypothesis 1): REJECTED
Candidate: select p50 first, then p95 and p99 inside the part above the previous index (8 changed lines,
no new lines). Baseline `6d5802b`, both built with `-C target-cpu=haswell` on rl-lyceum, full seed 1, 1 thread,
`perf stat`, 2 alternating rounds:

| | instructions | cycles |
|---|---:|---:|
| base | 1,196.2 M / 1,205.7 M | 674.0 M / 670.5 M |
| cand | 1,209.1 M / 1,205.9 M | 676.0 M / 671.6 M |

Output hash on the full set unchanged (`495ca09682b9...`), 7 unit tests passed. Pre-registered keep threshold
was at least 2% fewer instructions; the result is +0.1% to +1.1%, so the hypothesis is falsified: the selection compare
count is not where the hours phase spends its instructions (the counting sort and gather dominate), or
introselect's pivot behaviour differs from the 7.6n/4.7n model. The 8-thread paired timing was not needed
and its script failed on output permissions; no conclusion drawn from it. Reverted; source is `6d5802b`.

Per the synthesis rule: next step is STOP unless the user accepts a larger, riskier change (hypotheses 2 or 3).

## Outcome of hypothesis 2 (fuse hours and text): REJECTED
Candidate: each hour formats its text into a thread-local buffer straight after selection, publishes the exact
size, waits (yield) for earlier hours' sizes and copies into the output mapped up front at an upper bound
(`HOURS * sum(88 + name length)`, sparse), file cut to the real size at the end. Dropped the `Row` list, the size
formula, the barrier, the second thread spawn and the final `release`/`drop(states)`. -19 source lines
(527 vs 546 counted lines in main.rs), 7 tests pass, all 9 datasets byte-identical at 1/3/12 threads.
VPS docker A/B, 3 alternating rounds x 9 runs, base `6d5802b` vs fused:
T trimmed means 0.0942/0.0847/0.0883 s vs 0.0902/0.0820/0.0913 s (noise-level, ~-1%).
M max: 19.82/19.51/19.88 MB vs 27.00/27.26/27.33 MB (+7.4 MB, about -1.4 pt at the board's M weight).
Cause: output pages are now charged while the parsed records are still held (blocks are only released
when their hours finish), so the two memory peaks add up instead of following each other. The profiler's note
that the peaks are near equal predicted that gains from fusing would not show in M; here it made M worse.
Rejected by the rule "M up more than 1 MB without a matching T gain". Patch kept at `$TMPDIR/fuse.patch`, source reverted to `6d5802b`.

## Outcome of hypothesis 3 (rely on well-formed input): ACCEPTED as candidate, not on the board
Spec `a-ingest.md` section 2: "Every line ends with a single LF, including the last. No CR, no empty lines ...
a solution may assume well-formed input", ts exactly 27 bytes inside the 72-hour window, status 100-599 three
digits, latency 0..4294967295 without leading zeros. Removed: CR handling, minimum line length, hour/month range
check, method-length check, latency digit-count range and per-digit validation, status validation, endpoint
non-empty check, `BAD`. Kept for memory safety: slot table bound ("too many endpoints"), bounded digit loop.
Unit test for malformed records and CRLF / missing-final-LF variants removed (out of contract).
Source: main.rs counted lines 546 -> 513 (-33). 6 tests pass, clippy clean, all 9 datasets byte-identical at
1/3/12 threads (full seed 1 sha256 `495ca09682b9...`).
rl-lyceum `perf stat`, full seed 1, 1 thread, haswell build, fresh builds (an earlier pair of runs reused a stale
cargo build because rsync preserved mtimes; discarded): base 1,205.4 M instr / 167.1 ms vs lean 1,100.0 M (-8.7%) / 162.3 ms (-3%).
VPS docker 3x9 runs: T 0.0897/0.0985/0.0835 vs 0.0860/0.0918/0.0860 s (noise), M 19.26/19.68/19.45 vs 18.98/19.57/19.08 MB.
Risk: behaviour on malformed input is now undefined (wrong output or abort, no memory-unsafe slices); the spec lets us assume it.

## Tail-copy removal (zero-padded mapping): neutral, committed locally only
`map_file` reserves an anonymous zero-filled region `SLACK` bytes (rounded to pages) longer than the file and maps
the file over its start (MAP_FIXED), so the parser may read `SLACK` bytes past the end; `summarize` no longer
copies the last lines into a padded buffer or special-cases a tail chunk. main.rs counted lines 513 -> 508 (-5).
6 tests pass (test `leak` pads like the mapping), clippy clean, 9 datasets byte-identical at 1/3/12 threads.
VPS docker A/B 3x9 runs, base `807ceec` vs candidate: T 0.0877/0.0805/0.0865 vs 0.0932/0.0892/0.0850 s,
M 19.71/19.29/19.56 vs 19.82/19.39/19.72 MB: no gain visible. Worth about +0.03 pt through S; not pushed.
