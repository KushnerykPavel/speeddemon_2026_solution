# Track A: VPS and Lyceum profile review and optimization plan

Research date: 2026-11-01. Research only; no solution changes, builds, benchmarks,
remote commands, submissions or pushes. Independent historian, profiler and
researcher reports were reconciled. Existing untracked evidence was preserved.

## Decision

The archived profiles describe an older implementation. Do not repeat fixed-layout
parsing, SipHash replacement, UTF-8 removal, SWAR parsing, selection quantiles or
streamed output as new proposals: those changes are already committed.

First restore exact endpoint equality, recover or collect a current-source profile,
and measure current cgroup memory. Then prioritize low-cost source simplification
and worker/chunk sizing; the first substantial storage experiment should be six-byte
hour-bucketed records. SIMD is conditional on a fresh parsing profile. Defer direct
output I/O until output-cache attribution is measured.

## Evidence identity and strength

| Item | Identity | Interpretation |
|---|---|---|
| Both archived profile bundles | `69aefa2355f6166a7e20b83fd5bb9b02eaa28e34`; source SHA-256 `7a81b23a5c4ce5d98da7d3a978fe2c2e33661a4b07808a773725399cb60cd286` | Historical implementation, called A7 in profile notes but step 6 in PERF |
| Current implementation | `01bf7d29d8785606c183983e926997f7d08b4655`; source SHA-256 `98af63ebf587195ac266d11ae909a0ff498057be07433e51138d16c298ebd4fc` | Hash-only endpoint lookup, SWAR parsing, exact-size hour buckets, selection, ordered worker writes |
| Inspection HEAD | `5a3d992de37a6336e19ccbba94bde1897fa46f3c` | Documentation commit; same implementation source |

Sources: [VPS profile provenance](../tests/performance/2026-11-01-ingest-perf-flamegraphs/README.md),
[Lyceum provenance](../tests/performance/2026-11-01-track-a-rl-lyceum/README.md),
[current source](../track-a/src/main.rs), and Git commits above.

The best recorded board result is current `01bf7d2`: **0.044817 s, 46.551040 MB,
S=423, 71.672304 points**, `run-3dde37561ccb`. This exists in
[PERF](../track-a/PERF.md), but matching raw board JSON was not found in the local
archive. An attempted refresh of the official S3 board/history feeds was inaccessible
through the web reader. Treat these figures as logged results, not independently
reconfirmed board evidence. The older `531def7` result, 0.876075 s / 35.188736 MB /
43.339268 points, has [preserved first-party JSON](../tests/performance/2026-10-31-board-results/normalized-results.json).

PERF's latest 268 ms task-clock, 1.95 billion instructions, 74% parsing, 24.8 MB
keys and ~10 MB output-cache statements have no matching archived current-source raw
profile in the inspected evidence. They are leads to verify. The old “Current best:
step 5” label, Track A README's full-sort description, and the earlier top-10 roadmap
are stale. Source revisions take precedence over stage names.

## What the saved profiles establish

Both hosts used warm full seed 1, release O3/fat LTO/one codegen unit, a warmup and
five measurements per worker count, alternating forward/reverse count order.
Executions were serial. Timing covers process spawn through exit and output writing;
build, container launch and output comparison are excluded.

| Host | Hardware | Workers → seconds | Statistic |
|---|---|---|---|
| VPS | Shared six-vCPU Haswell-class KVM, affinity 0–5 | 1 → 1.193228; 2 → 0.670827; 6 → 0.294095; 12 → 0.275626 | Mean of middle three |
| rl-lyceum | i7-7700, four cores/eight logical CPUs, affinity 0–7 | 1 → 0.739274; 2 → 0.361952; 4 → 0.221070; 8 → 0.178939; 12 → 0.230746 | Median |

These measure scaling of one historical implementation. They do not isolate a new
optimization. VPS six versus twelve is inconclusive amid roughly 11–24% spreads;
twelve workers oversubscribe six virtual CPUs. Lyceum eight workers was fastest
observed, but its range was 0.164–0.305 s. Neither sweep sets the optimum for current
code or the scoring machine. CPU, compiler and privilege differences prevent using
cross-host absolute times as an optimization comparison.

Sources: [VPS raw summary](../tests/performance/2026-11-01-ingest-perf-flamegraphs/vps/summary.json),
[Lyceum raw summary](../tests/performance/2026-11-01-track-a-rl-lyceum/native/results/summary.json).

VPS six-worker inclusive solution CPU samples show ingestion **81.32%**, parsing
**35.26%**, UTF-8 **8.55%**, SipHash **12.28%**, sorting **4.23%** and formatting
**3.13%**. These categories overlap; UTF-8 belongs within parsing. Lyceum eight-worker
flat self reports show SipHash write **8.84%**, endpoint HashMap get **8.80%**, UTF-8
**8.65%**, quicksort **3.99%** and memcmp **3.62%**. Different denominators and
inclusive/self attribution mean these percentages cannot be added together.
[Sample analysis](../tests/performance/2026-11-01-ingest-perf-flamegraphs/profile-analysis.json),
[Lyceum flat report](../tests/performance/2026-11-01-track-a-rl-lyceum/native/results/workers-8-report.txt).

The accepted sampling builds add frame pointers and unwind tables and change code
generation; their timings are not release comparisons. VPS repaired stacks have no
unknown folded frames, whereas 10.29% of Lyceum eight-worker solution stacks contain
an unknown frame. Original VPS DWARF flamegraphs with broken callchains were excluded.
[Capture limitations](../tests/performance/2026-11-01-ingest-perf-flamegraphs/README.md),
[Lyceum sample quality](../tests/performance/2026-11-01-track-a-rl-lyceum/profile-analysis.json).

Lyceum one/eight-worker instructions stay near 5.89/5.93 billion while aggregate
cycles rise 2.66 → 4.28 billion and IPC falls 2.21 → 1.38. Branch miss rates remain
2.08%. This supports scaling inefficiency without establishing its cause. Generic
cache misses/references of 24.4% are not 24.4% of all loads; L1 load misses were 2.37%.
VPS PMU counters were unavailable. Neither host proves a memory-bandwidth bottleneck.
[Counter evidence](../tests/performance/2026-11-01-track-a-rl-lyceum/README.md).

Historical RSS observations around 269–273 thousand KiB include mapped input and
are not scored memory. No current cgroup peak reduction follows from those numbers.
Sorting's historical 4.23% CPU share gives only a 1.044× ideal aggregate CPU ceiling
if sorting vanished, not a current elapsed-time forecast.

## Already attempted: avoid duplicate experiments

| Mechanism | Evidence and decision |
|---|---|
| Buffered byte parser | No demonstrated native Linux time win; batches +3.4% and +1.1% amid noise; board worse than predecessor. Kept, not credited as a speed optimization. |
| Endpoint interning | Accepted; board 0.876075 s; local 3.7–5.8% improvement was noisy. |
| Most-recent-hour cache | Kept; paired full-input VPS improvement 9–11%, but alternating-hour synthetic input regressed 4–10%. Generic fallback is necessary. |
| Parallel mmap ingestion | Implemented at `69aefa2`; archived scaling and correctness evidence. |
| Fixed parser/custom endpoint table | Implemented at `14001f9`, bundled with per-hour sorted keys. |
| SWAR/selection/streaming | Implemented at `70c6a36`; no isolated mechanism attribution. |
| Exact-size buckets/hash-only lookup/worker writes | Implemented together at `01bf7d2`; no archived controlled local A/B. |

Sources: [byte-parser experiment](../tests/performance/2026-10-30-ingest-byte-parser-linux/README.md),
[endpoint interning](../tests/performance/2026-10-30-ingest-dense-groups/README.md),
[hour-cache experiment](../tests/performance/2026-10-31-ingest-hour-cache/README.md),
[PERF history](../track-a/PERF.md) and the corresponding Git commits.

## Correctness gate

`State::endpoint` returns an existing ID when its stored 64-bit hash matches, without
checking the endpoint bytes. Distinct endpoints with an equal hash would be merged
locally and in global remapping. Restore byte equality after the hash match and
continue probing on unequal names. This is an algorithmic correctness gap; no
concrete colliding generated input was demonstrated in this review.
[Implementation](../track-a/src/main.rs).

The [official contract](https://raw.githubusercontent.com/hackathon-raptors/speed-demon-2026/main/specs/a-ingest.md)
requires exact byte identity, full u32 latencies, nearest-rank percentiles, duplicate
preservation and ordered output. It permits fixed ASCII/timestamp parsing and a
72-hour index. Its 1,842-endpoint catalogue is not permission to hardcode names or
answers. There is no promised out-of-order bound: closing an hour on first observing
a later hour is invalid.

Validate the restored lookup with a direct table test inserting different names
under the same supplied hash, plus integration reference comparisons. Keep the
current revision recoverable, but use the corrected version as the baseline for
performance acceptance; do not compare faster incorrect behavior with exact code.

## Ranked research hypotheses

The following is a **conditional planning model**, not measured benefit or a
probability-calibrated forecast. It uses the logged current K≈27.13, S=423 and
M=46.551040 MB, unchanged dependencies, and uncapped memory/source factors.
Each row is independent; do not add the gains. Replace the anchor after restoring
correctness and measuring it.

| Rank | Isolated hypothesis | Central after/before T, M, S assumptions | Hours including validation | Conditional ΔK/hour |
|---|---|---|---:|---:|
| 1 | Simplify existing code without changing work | 1.00, 1.00, 0.92 | 0.75 | 0.455 |
| 2 | Tune workers and chunk multiplier | 0.96, 1.00, 1.00 | 1.5 | 0.373 |
| 3 | Store bucketed records in six bytes | 0.98, 0.867, 1.07 | 4 | 0.196 |
| 4 | SIMD endpoint delimiter discovery | 0.94, 1.00, 1.12 | 4 | 0.095 |
| 5 | Avoid output page-cache charging, only if proven | 1.05, 0.783, 1.10 | 4 | 0.070 |

The formula is
`K_after/K_before = T_ratio^(-0.50) * M_ratio^(-0.20) * S_ratio^(-0.15)`.
Official caps still apply. Code is counted after formatting; merging statements is
not a source-size optimization. Ranking is sensitive to the assumed effort and S;
near ties are not meaningful. New runtime dependencies are excluded from these plans.
[Official scoring implementation](https://raw.githubusercontent.com/hackathon-raptors/speed-demon-2026/main/harness/scoring.py),
[official source counting and measurement](https://raw.githubusercontent.com/hackathon-raptors/speed-demon-2026/main/harness/README.md).

1. **Source simplification:** time-box a review of redundant adapters, duplicated
   remapping and temporary state, retaining bounds and error handling. An 8% S cut
   alone yields about 1.26% K, or 0.27 speed points. This target is unproven; stop
   if no real simplification exists. Canonically count formatted S, verify outputs,
   and reject time/memory regressions that erase the size gain.
2. **Workers/chunks:** current code uses `workers * 32` chunks. Sweep current-source
   worker counts matching real topology, then a small chunk-multiplier grid one
   variable at a time. Record per-worker completion and phase wall times to
   distinguish overhead from imbalance. Reject a default justified only by VPS
   oversubscription or one noisy fastest run. No dependency or algorithm change.
3. **Six-byte records:** current stored keys use eight bytes per request. Once
   bucketed by hour, the hour is implicit; retain a full u32 latency and a u16
   endpoint/error tag. Use an actual six-byte representation or parallel u32/u16
   arrays; a naturally aligned struct may still occupy eight bytes. Pack during
   the existing bucket scatter, preserving the parser as the isolated baseline.
   At 3.1 million records, retained payload falls 24.8 → 18.6 MB: **6.2 MB**, about
   13.3% of logged total M if the entire saving reaches the high-water mark.
   Measure transient source/destination coexistence, decoding cost and both scatter
   passes. Check ID capacity explicitly; preserve all u32 latencies and error bits.
   A 7% S increase permits only about 3.8% slower T under this central M assumption
   before K stops improving. Reject if measured K fails to improve beyond noise.
4. **SIMD endpoint scan:** the current baseline already scans eight bytes at a time.
   Test delimiter discovery alone against that baseline, preserving hashing and
   complete equality, with a scalar tail/CPU fallback. Test 1/7/8/9/15/16/17/31/32/
   33/125-byte endpoints and mapping/chunk tails. Reject if the relevant fresh
   phase is small or dispatch/source costs erase the gain. Do not describe a
   full-hash replacement as the same experiment.
5. **Output memory:** first establish whether output file pages contribute to the
   measured cgroup peak while keys remain resident. Direct I/O is lower priority:
   alignment, final-tail handling, filesystem support and blocking writes add
   complexity and can lose runtime. The table optimistically assumes the entire
   10.113 MB output charge disappears; this is an upper scenario, not an observed
   saving. Generic cache advice is not a guarantee of eviction or a lower past
   peak. Require exact file length/hash and a buffered fallback before considering
   acceptance. No claim that O_DIRECT is currently beneficial.

Primary implementation references for the conditional SIMD and output proposals:
[Rust AVX2 mask intrinsic](https://doc.rust-lang.org/core/arch/x86_64/fn._mm256_movemask_epi8.html),
[Linux direct-I/O constraints](https://man7.org/linux/man-pages/man2/open.2.html),
and [kernel cgroup memory accounting](https://docs.kernel.org/admin-guide/cgroup-v2.html).
These document mechanisms and limits, not a demonstrated gain for this program.

## Next authorized experiment protocol

The recommended next action is a **measurement and correctness round**, followed
by the worker/chunk experiment; packed storage is the next substantial change.

1. Recover current-source raw Lyceum evidence if it survives; otherwise collect
   fresh profiles through the existing workflow. For VPS execution use the
   `my-vps-code-check` skill and resolve the actual project path from evidence.
   This research did not contact either host.
2. Record source/manifest/input/binary hashes, compiler, flags, CPU/topology,
   affinity, privilege, warmup and exact timed boundary. Keep the original source
   and correctness-restored baseline distinct.
3. Separately measure release end-to-end T and diagnostic phase wall times:
   parse, hour count/scatter, endpoint union/remap, finalize, ordered output, and
   tail waiting. Aggregate CPU samples are not elapsed-time fractions.
4. On Lyceum collect release counters at one and eight workers and a separate
   frame-pointer attribution build. On VPS collect software counters if PMU is
   still unavailable. Keep unprivileged timing distinct from privileged counters.
5. Measure a fresh per-run cgroup peak plus anonymous/file/kernel breakdown at
   phase boundaries; a boundary snapshot may miss the peak. Use the official
   harness where supported. Record RSS separately, never as scored M.
6. For each later candidate: identical inputs/build conditions, serial alternating
   AB/BA runs, at least five measurements/variant after warmup; compare output on
   every run. Repeat only for new failures or unresolved noise. Validate tiny and
   sample seeds 1/7/42, multiple full seeds where feasible, empty input, all
   percentile boundaries, duplicate/max-u32 values, sparse hours and collisions.
7. Count canonical formatted S and D; calculate K using comparable measured T/M.
   Save raw evidence under `tests/performance/`, update PERF with source identity,
   limits and accepted/rejected/inconclusive status. Effects within noise remain
   inconclusive. After three unsuccessful experiments, revisit the bottleneck
   model with fresh independent reports before another optimization.

No new speedup, memory improvement or optimal worker default is established by
this research. Board submission and pushing remain separate explicit actions.
