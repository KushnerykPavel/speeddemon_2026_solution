# Track D plateau recovery (2026-11-01, 15:40 UTC)

Inputs: [current-state.md](current-state.md), [historian.md](historian.md), [profiler.md](profiler.md),
[algorithm-research.md](algorithm-research.md), plus three new parent diagnostics (§2). No production
code changed, nothing committed or pushed. Tags: **F** fact, **M** measured (environment named), **I**
inference, **U** unknown or estimate.

## 1. Executive diagnosis

1. **The plateau is real, but the reported number is wrong.** "77.26" is not Track D. Our Track D best
   is `run-8040ef5bc533`, 65.39 points (T 0.1986 s, M 79.65 MB, S 723), rank 32/104. It is the luckiest
   of **21 board runs of one unchanged binary**: T 0.2069 ± 0.0069 s (CV 3.3%), 64.95 ± 0.35 points.
   **F** (official feed, 2026-11-01 15:04 UTC).
2. **We have been tuning below the board's resolution.** Two single board runs of the same binary differ
   by about ±9% (95%). No change since step 19 (board 0.195 s) has had a board-visible effect. Steps
   20–26 (code size, Dial ×3, blobs, PGO, μ-prune) each targeted 0–5%. The final protocol (5 runs, middle
   3) has an SD of about 1.6%, so a real 4% gain does count. But single board pushes were treated as
   verdicts. Two of them were luck: the "SWAR did not transfer" result and the "memory diet got faster".
3. **The pipeline's structure was never questioned.** Every experiment since step 19 kept the same
   pipeline. It parses, places, claims by BFS, copies, counts the transpose, scatters the transpose and
   searches: 7 fork/join barriers, 84 thread spawns. It searches the full 1.74M-vertex graph and a full
   reverse copy, with THP off for the whole process.
4. **The gap is a 2× time problem, and no single mechanism closes it.** The leaders run at 0.10–0.12 s.
   Our preprocessing alone (≈0.08–0.11 s on the board, inferred) is as long as their whole run. The best
   public competitor, HighVelocity (0.110 s), closed the gap by stacking 5–8 measured changes worth 5–15%
   each. The largest is degree-2 bypass (−13%, 30/30 paired wins). **M** (their repo), **I** for us.
5. **Realistic ceiling before freeze:** stacking the plausible changes gives ≈ −20 to −25% T. That is
   ≈0.155–0.165 s, about +2.5–3 points, and roughly rank 15–20. It does not reach the top 10 or 0.1 s.

## 2. Performance model

### Phase split

Mac medians are the new parent diagnostic: 5 rounds, full seed 1, phase-timer build in the scratchpad,
same source as `21df4fd`. Output hash `a23dbdf0…` in every run.

| Phase | Mac 1 T (ms) | Mac 8 T (ms) | Mac 12 T (ms) | 12 T speedup | VPS 12 w (ms) | FlyHigh on the box (their code) |
|---|---:|---:|---:|---:|---:|---|
| Parse + bucket | 96.8 | 15.3 | 18.6 | 5.2× | ≈88–98 incl. place | parse 15 |
| Place CSR | 21.9 | 6.0 | 5.9 | 3.7× | (above) | CSR 12 |
| BFS claim | 115.0 | 18.1 | 18.1 | 6.4× | 116–121 renumber | BFS 20 |
| New offsets (serial) | 4.5 | 4.3 | 4.1 | 1.1× | (above) | |
| Row copy | 122.4 | 27.6 | 18.7 | 6.5× | (above) | copy 15 |
| Transpose (count / prefix / scatter) | 15.9 / 6.9 / 22.2 | 3.5 / 7.5 / 6.1 | 3.8 / 6.9 / 4.9 | 2.9× | 55 | reverse 17 |
| Search | 572.5 | 84.2 | **68.2** | 8.4× | ≈200 | ≈65 |
| **Total** | 994 | 185 | **163** | 6.1× | ≈470 | 155 |

The Mac is an M1 Pro with 8 P-cores and 2 E-cores, no THP and no prefetch instruction. At 12 threads
(0.163 s) it is closer to the board (0.207 s mean) than the VPS (0.47 s, 6 Haswell vCPUs) is.

Points from the profiler and researcher reports:

- **Working model for the board (I):** about 55% preprocessing (≈0.11 s) and 45% search (≈0.09 s).
  Four sources agree: the Mac (58/42), the VPS (58/42), FlyHigh's on-box timers (55/42) and the
  profiler's proportional scenario. Its 73% bound (search scaling perfectly, nothing else) is an upper
  limit only.
- **Serial fraction is unresolved.** Code that really runs serially (new offsets plus the transpose
  prefix and allocation) is 11 ms, about 7% on the Mac. The profiler's Amdahl fit on the VPS gives an
  effective 27% serial share (12 workers on 6 vCPUs, rusage 80–83% CPU). The fit includes bandwidth
  limits and barrier waits. Neither machine is the board. Parallelizing the prefix sums is worth at
  most about 5%.
- **Search (M, VPS evidence):** 7.57M pops and 8.66M pushes. The top 5% of queries take 52–57% of the
  search work and the top 10% take 74–81%. Cost is about 90 ns per pop at 1 thread on the Mac and about
  140 ns at 6 workers on the VPS. It looks memory-latency bound, as several results show:
  - no renumbering means 2.7× the CPU for the same pops;
  - prefetch gave −7 to −13%;
  - every queue change was null or worse (§3).
  
  Hardware counters do not exist for any Track D run (the VPS has none), so dTLB and LLC misses are
  **U**.
- **Board box (F, FlyHigh README and the harness):** i5-13500, timed on its 6 P-cores and 12 threads,
  with a 24 MB L3. Our forward and reverse CSR are about 54 MB on 4 KB pages, so they fit neither the L3
  nor the STLB reach (about 8 MB).
- **Memory (M):** the peak is 78.8 MB at the end of placement (buckets plus the CSR), with the renumber
  copy next at 74.7 MB. Getting to 65 MB needs both of those cut and is worth only about +0.9 points.

### New diagnostics from this round

1. **Radix-heap prototype (Mac, scratchpad):** exact output; search +7–15% at 1 thread and +10–30% at
   12. It is the fifth queue variant to lose. Cache Me, FlyHigh and HighVelocity all measured heaps as
   equal. **The queue is closed.**
2. **Degree-2 census (seed 1, Python over the input):** 397,682 vertices (22.8%) are two-way degree-2,
   meaning in = out = {a, b} with a ≠ b, ignoring self-loops, and are not query endpoints. They carry
   1,590,728 edge lines (31.8%). Another 100,900 vertices are (1,1) dead-end spurs with in = out = {a},
   about 200k more edges. One-way chain vertices: 6. **M**
3. **Hardware counters:** the VPS reports "The cycles event is not supported". **F**

## 3. Failed-experiment analysis: what to stop trying

| Mechanism | Status | Why |
|---|---|---|
| Priority-queue swaps (Dial ×3, radix, PGO-tuned heap paths) | Stop | The queue is not the bottleneck. Five local variants lost; three competitors saw no difference. |
| μ-prune of pushes | Stop | Pushes fell 5.7% and pops were unchanged, so there is no work to save. |
| Blobs 512/128/64 | Stop | Search unchanged or slower, memory up. |
| PGO | Stop | Null at 1 thread; needs a binary artifact. |
| Transpose fused into the copy with atomics, or buffer recycling | Stop as built | Atomic contention, peak 85 MB. Revisit only without atomics (H6). |
| No renumbering | Settled | +130%. Locality is the dominant search factor. |
| Single board push as an A/B | Stop | Below resolution. Use the resample protocol (§6). |

Shared assumption of steps 20–26: search the graph exactly as given, and shave constants per pop or per
phase. The work itself (pops, edges per phase) never changed. The only competitor mechanism with a
>10% measured effect reduces work (degree-2 bypass).

## 4. Ranked hypotheses

Estimates are **U** unless marked. ΔP = board points from the T, M and S changes. Hours include VPS
validation.

| ID | Hypothesis | Bottleneck | Est. ΔT | ΔM | ΔS | ΔP | Conf. | Hours | Tier | Conflicts with past failures? |
|---|---|---|---:|---:|---:|---:|---|---:|---|---|
| H1 | **Degree-2 bypass (and dead-end spur removal) during renumbering** | search work, copy and transpose work | −10 to −15% | −5 to −10% | +60–90 | +1.0 to +1.8 | medium (31.8% of edges measured; HighVelocity −13%) | 7–9 | 2 | No; new mechanism |
| H2 | **THP back on for the dense final CSR arrays only** | dTLB walks in search, copy, transpose | −3 to −8% | 0 to +1 MB | +10 | +0.3 to +0.9 | medium-low (FlyHigh board +14% with THP off; ours +4%, one run) | 1.5–2 | 1 | Refines step 15: the sparse buffers stay on 4 KB pages |
| H3 | Parallel prefix sums; fuse `degrees` and `cursor`; no serial 7 MB atomic collects | serial sections (11 ms Mac) | −3 to −5% | −7 MB transient (not at the peak) | +5–15 | +0.3 to +0.5 | medium | 2 | 1 | No |
| H4 | Transpose without atomics: per-worker inboxes bucketed by target block (HighVelocity 79 → 57 thread-ms) | transpose | −3 to −5% | 0 to +5 MB | +20 | +0.2 to +0.5 | medium-low | 3 | 2 | Differs from the step 22 failure (that one used atomics) |
| H5 | 6-byte load bucket records (u16 source offset + u32 packed) | memory peak | 0 | −6% (→ ≈75 MB, peak moves to the copy) | +10 | +0.25 | high for M | 2 | 1 | No |
| H6 | One persistent worker pool with barriers instead of 7 scopes | spawn overhead and cache warmth (Cache Me −6%, laptop) | −1 to −4% | 0 | +20 | ≈+0.2 | low | 3 | 2 | No |
| H7 | Grow the side that has settled fewer vertices (not the shorter queue) | search balance | −1% | 0 | +2 | ≈+0.1 | low | 1 | 1 | Variant of the side choice; the min-key flood risk does not apply |
| H8 | SSE separator scan in the parser | parse (≈11% of the Mac run) | −2 to −4% | 0 | +30 | ≈+0.2 | low (the SWAR VPS gain did not show on the board) | 3 | 2 | Partly; SWAR is already in |
| H9 | Restore a parallel BFS with provisional heads (the pre-step 19 one-read design) only if H1 rewrites the renumbering anyway | copy phase | −5% | +10 MB | 0 | ≈0 | low | (in H1) | 2 | Memory trade already measured in step 19 |
| H10 | S cut: remove the strict-parser fallback paths the spec makes unnecessary, as Track A step 807ceec did | S | 0 | 0 | −40 to −60 | +0.2 to +0.3 | high | 1 | 1 | No; check the "well-formed input" spec clause |
| X | ALT landmarks, contraction hierarchies, a full single-structure FlyHigh rewrite | — | — | — | — | negative or > 10 h | — | — | 3, rejected | ALT: 4 landmarks cost ≈1.8× the whole search phase |

Required categories: algorithmic H1; memory layout H2 and H5; parsing or construction H3, H4 and H8;
query-distribution specific H7; minimal-risk H2, H10 and H7.

Interactions: H1 shrinks the graphs, so the gains from H2–H4 shrink with it. H1 and H2 together are
not additive (once the graph is smaller, fewer pages miss the TLB). Each added line costs about 0.001
points per line (1% of S ≈ 0.03 points). Every hypothesis keeps D = 1.

## 5. Top three experiments

### E1 = H2: selective huge pages (recommended first, Tier 1)

- **Location:** `tune_memory` and `reorder_graph` (`track-d/src/main.rs:159–167, 461–476`), `transpose`
  (`:544–552`), `answer_queries` (`:779–786`).
- **Change:**
  1. Keep `PR_SET_THP_DISABLE = 1` through load and the BFS: the sparsely touched buckets, `order` and
     `degrees` stay on 4 KB pages.
  2. Before allocating `new_offsets`/`new_edges`, call `prctl(PR_SET_THP_DISABLE, 0)` and
     `madvise(MADV_HUGEPAGE)` on both allocations.
  3. Do the same for the transpose `offsets`/`edges`.
  4. Set the flag back to 1 before the search workers allocate their `Side` pages.

  THP is checked at fault time, and the arrays are written in full, so they gain no resident slack.
  `mallopt(256 KB)` keeps them in their own mmap VMAs.
- **Expected:** T −3 to −8% (mostly search, plus copy and transpose). M unchanged: the 78.8 MB load
  peak stays, with the copy peak at about 74.7 + ≤ 4 MB. Board ≈ +0.3–0.9 points.
- **Benchmark:**
  1. Worktree off `539b15f`. Check `/sys/kernel/mm/transparent_hugepage/enabled` on the VPS (expect
     `madvise`).
  2. A scratchpad diagnostic build prints `AnonHugePages` from `/proc/self/smaps_rollup` after the
     transpose; the timed binary stays clean.
  3. Existing VPS protocol: Docker, cpuset 0–5, cgroup `memory.peak`, 12 and 6 workers, ≥ 10
     alternating pairs, phase build for the search phase.
  4. Byte-equal outputs on full seed 1, sample seeds 1/7/42, tiny, edge cases, no-edges and no-queries,
     plus local full seed 2.
- **Stop or revert if:**
  - `AnonHugePages` is 0;
  - `memory.peak` rises by more than 1 MB;
  - the VPS search phase is not faster in ≥ 7/10 pairs.

  KVM nested paging may mute the VPS signal. If the VPS shows a null result with huge pages present,
  only the board resample (§6) can decide.

### E2 = H1: degree-2 bypass (largest work reduction, Tier 2)

- **Location:** `reorder_graph` (`:408–519`) and the query translation (`:515–517`). The search code
  does not change.
- **Change:**
  1. Mark a vertex *bypassable* if it is not a query endpoint and, ignoring self-loops, it has exactly
     two out-edges and two in-edges to and from two distinct neighbours {a, b}.
  2. From each kept vertex, walk every chain of bypassable vertices to the next kept vertex. Emit one
     edge per direction whose weight is the chain sum. Add a weight-0-safe check, and keep the chain
     when the sum is ≥ the packing escape, so `u32` exactness holds.
  3. Remove dead-end spurs: a vertex with in = out = {a} that is not an endpoint is never on a shortest
     path between other vertices.
  4. Bypassed vertices get no row in the new CSR. They need no ID; a sentinel works, and degree
     rejection is never reached because endpoints are kept.
  5. A ring made only of bypassable vertices (the 12-node island ring) is never entered from a kept
     vertex, so it is never walked. If it holds no endpoint, it is not needed.
- **Expected:** search pops and edges −20 to −30% (the census is measured; the effect on pops is
  estimated). The copy and transpose handle about 30% fewer edges. Measured on HighVelocity: −13% wall
  time in 30/30 pairs. Board ≈ +1.0–1.8 points, memory slightly lower, S +60–90.
- **Correctness obligations:**
  - Parallel edges or duplicates make a vertex non-bypassable (its in/out count is above 2).
  - Weight-0 chains stay exact.
  - Sums near 2^32 keep the chain.
  - Query endpoints are never removed.
  - The all-pairs reference test (`tests/performance/2026-11-01-traverse-mu-prune/prune-tests.rs`
    style) on random graphs with high degree-2 density, rings and spurs, and every fixture.
  - A mutant (dropping the escape check) must be caught.
- **Benchmark:** pop and push counters (deterministic, noise-free) first, then the VPS protocol as in
  E1, then multi-seed equality.
- **Stop if:**
  - pops fall by less than 10%;
  - the VPS total is not faster in ≥ 7/10 pairs;
  - any output differs;
  - implementation passes 6 h without exact equality.

### E3 = H3 + H10: serial-section and source-size cleanup (Tier 1, low risk)

- **Location:** `transpose` (`:540–552`): turn `degrees` into cursors in place with a parallel blocked
  prefix sum and derive `offsets` from the cursors after the scatter. Apply the same to `new_offsets`
  in `reorder_graph` (`:461–469`). For H10, remove the `parse_fields` fallback paths only where the spec
  §2 guarantees the input is well formed.
- **Expected:** −3 to −5% T and −40 to −60 S, ≈ +0.5–0.8 points, all of it small.
- **Benchmark:** the same VPS protocol plus the Mac phase build. Fixtures must still pass. Malformed
  input checks become intentionally out of scope only if the spec guarantee is quoted.
- **Stop if:** the transpose and renumber phases are not faster by ≥ 5 ms on the Mac at 12 threads, or
  S does not fall.

## 6. Recommended next experiment and board protocol

**Run E1 (selective THP) now.** It has the best value per hour: 1.5–2 h, about 10 lines, zero risk to
correctness, and direct board evidence that THP state moves time (FlyHigh +14%).
**Start E2 (degree-2 bypass) right after**, as the one Tier-2 bet with a measured work reduction behind
it. Its census (31.8% of edges) clears the 30% bar set before it was measured. E3 fills gaps while VPS
runs are queued.

**Board resample protocol (requires your push authorization):** a single board run cannot resolve the
3–8% effects expected here. Push the candidate once, then 2–3 record-only commits (≥ 300 s apart).
That gives 3–4 samples to compare against the 21 baseline samples (mean 0.2069 s, SD 0.0069). SE ≈ 1.7%,
so a 5% effect shows at about 3σ. Accept on the board only if the candidate's mean T is below the
baseline mean by ≥ 2 SE.

## 7. Final decision

**Continue with a high-confidence optimization (E1), then the measured-work-reduction bet (E2).** Keep
`21df4fd`-identical source recoverable (tag it) as the frozen fallback.

New evidence that would change this:

| If this happens | Then |
|---|---|
| `AnonHugePages` = 0, or the board resample shows no gain | Drop E1 and go straight to E2 |
| E2's pop counters fall < 10% | Stop E2; freeze after E3 |
| By about 2026-11-02 09:00 UTC no candidate has passed the VPS protocol and board resample | Freeze `21df4fd` code and spend the remaining time on documentation (below) |

## 8. Housekeeping the judges will see (independent of experiments)

- `track-d/PERF.md` names 65.16 as the best. The feed's best is `run-8040ef5bc533` (65.39), and the 21
  reruns of the same binary are not listed. Add a row and the noise note (64.95 ± 0.35 points).
- `track-d/README.md` is stale in 5 places (`historian.md`).
- Steps 25–26 in PERF.md and their evidence dirs are uncommitted. Commit them before any experiment
  lands (your call).
- `diet-tests.rs` in the step 19 evidence no longer compiles.

## 9. Outside Track D

`overall()` (harness/scoring.py) is the geometric mean of the **best three** track totals and is `None`
with fewer than three. Tracks B and C are unchanged starters that the board does not run. If the target
is the team's overall score rather than Track D points, a scored third track probably outweighs
everything in this report.

## 10. Experiment outcomes (2026-11-01, approved: E1 then E2, VPS only)

| Exp | Result | Evidence |
|---|---|---|
| E1 selective THP | **Accepted on the VPS, small.** 12 w −4.7% (9/10 pairs), 6 w −7.1% (8/10); peak unchanged; minor faults −24%; AnonHugePages 43–45 MB. Gain on the KVM guest is in the copy and transpose (faults), not the search; the board TLB effect is untested. +26 scored lines (≈ −0.12 points from S). Pre-registered search-phase criterion not met; accepted on total time. | `tests/performance/2026-11-01-traverse-thp-dense/`, PERF step 27, branch `exp/d-thp-dense` (uncommitted) |
| E2 degree-2 bypass | **Rejected.** Exact, and search −21% on the VPS, but classification and contraction cost more: old-ID classification ≈30 ms of random access on the Mac; post-renumber contraction 152 ms on the VPS (total +17%). | `tests/performance/2026-11-01-traverse-bypass/`, PERF step 28 |

Lessons from the experiments:
- On this graph, any pass that touches heads in the original random order costs about as much as the BFS itself.
- A work reduction pays only if it adds no per-edge random pass.
- The Mac-proxy and VPS directions agreed for E2. For E1 the Mac cannot test THP at all.

Updated decision:
1. **E1 is the only candidate.** Its expected board effect (−3 to −8%, estimate) is below single-run noise.
2. **Next step needs your push authorization.** Board resample: the candidate plus 2–3 record-only pushes, compared
   with the 21 baseline samples (mean 0.2069 s). Adopt only if the candidate's mean T is ≥ 2 SE below the baseline.
   The board's `defrag` setting is a risk to watch.
3. **Without board access, freeze `21df4fd` code** (or adopt E1 on VPS evidence alone; a judgment call, since the
   expected value is small and positive and correctness risk is nil).
4. **Remaining Tier-1 candidate:** E3 (parallel prefix sums and the S cut, about 2 h). After E2, one more unsuccessful
   experiment would trigger the AGENTS.md rule to rebuild the bottleneck model first.
