# Track D profiler report (2026-11-01)

Tags: **F** fact from a file or source; **M** measured (environment named); **I** inference or arithmetic;
**U** unknown. Nothing was run, built, or sent to the VPS for this report. The only new computations were
offline Python passes over archived evidence. The scripts are in the session scratchpad (`profiler/tail.py`,
`profiler/phases.py`).

## 0. Source identity and conditions

- **F** Current code: `track-d/src/main.rs` sha256 `459900905ca4…`, byte-equal to
  `tests/performance/2026-11-01-traverse-mu-prune/before-main.rs`, and functionally `21df4fd`
  (the BFS blobs were reverted in `740ed03`).
- **F** VPS (`my-vps`): six vCPUs reported as "Haswell, no TSX" under KVM, about 2.4 GHz (BogoMIPS 4788).
  Runs used CPU set 0-5, an 8 GB cgroup, rustc 1.99.1 in a pinned image, an offline
  `target-cpu=native` release build, full seed 1 with prewarmed input, and the `measure.rs` wrapper (wall
  time including process start and exit, cgroup v2 `memory.peak`, rusage). Sources:
  `mu-prune/cpu.txt`, `mu-prune/compiler.txt`, `transpose/edgescan/experiment.py`.
- **F** `lscpu` on the VPS reports 24 MiB L2 and 96 MiB L3 "(6 instances)". These are KVM CPUID values,
  not the physical cache. Step 23's argument that "the VPS cache is larger than the board's" therefore has no
  measured support (**I**).
- **F** Board: Core i5-13500. Runs use 6 P-cores with SMT (`--cpuset-cpus=0-11`), turbo off, governor
  `performance`, per `docs/optimization-strategy-2026-10-30.md:124` (tagged [V] there; the harness README
  was not re-read). P-core L2 is about 1.25 MB versus 256 KB on Haswell (**I**, vendor specification).
- **F** Board noise for the unchanged D binary over 21 runs: T mean 0.2069 s, SD 0.0069, minimum 0.1986.
  M 79.62 ± 0.21 MB (`research/current-state.md` §3).

## 1. Phase decomposition (Q1)

Phase timers come from separate profile builds that print `PHASES=` and `TRANSPOSE=` lines. Boundaries
(`memory-diet/profile-balanced-main.rs:973-994`):
- `graph` = mmap + parse + bucket + place.
- `reorder` = BFS + copy + query ID translation.
- `search` = `answer_queries`, transpose included.

Query parsing, `create_dir`, and output fall outside all three. **F** Provenance gap: the archived
`transpose/edgescan/profile-before-main.rs` has no `PHASES=` emitter, although the experiment script
requires one. The binary that ran is therefore not byte-archived. The same boundaries reproduce in three
other directories, so the split below stands (**I**).

VPS medians, 12 workers (oversubscribing 6 vCPUs), 5 rounds each (**M**):

| Phase | edgescan `before-12` (21df4fd) | blobs `before-12` | transpose/recycle `before-12` | Share of wall |
|---|---:|---:|---:|---:|
| Load (mmap, parse, bucket, place) | 0.088 | 0.098 | 0.094 | 19–20% |
| Renumber (BFS, prefix, copy) | 0.116 | 0.121 | 0.117 | 24–25% |
| Transpose: count / prefix+alloc / scatter | 0.015 / 0.016 / 0.024 = 0.055 | — | — | 11–12% |
| Search proper (search − transpose) | 0.198 | ≈0.205 | ≈0.195 | 41–42% |
| Unaccounted (start, queries, output, exit, munmap) | 0.008 | 0.008 | 0.008 | 1.7% |
| Wall (measure.rs) | 0.470 | 0.508 | 0.481 | |

- **M** At 6 workers: load 0.107, renumber 0.123, transpose 0.057, search proper 0.201, wall 0.492.
- **U** No archived timer separates parse from placement, or BFS from copy. The memphase marks carry no
  timestamps.
- **M** rusage (mu-prune and no-reorder baselines, 8 rounds each):

  | Threads | User CPU (s) | Sys CPU (s) | Wall (s) | Utilization | Minor faults |
  |---:|---:|---:|---:|---:|---:|
  | 1 | 2.07 | 0.17 | 2.25 | — | 43k |
  | 6 | 2.10 | 0.23 | 0.486 | 80% of 6 CPUs | 47k |
  | 12 | 2.13 | 0.23 | 0.476 | 83% of 6 CPUs | 48k |

  Total CPU grows only 4% from 1 to 6 threads. Sys time is about 10% of CPU.
- **I** Amdahl fit: 4.63× speedup on 6 CPUs implies an effective serial fraction of about 5.9% of
  single-thread work. That is about 0.13 s, or 27% of the 6-worker wall time. This "effective serial" time
  includes imbalance, barrier waits, and contention, not only literal serial code.
- **I** CPU split: search uses about 1.06 thread-seconds (tail data at 6 workers, one query per vCPU). The
  remaining ~1.28 CPU-s is preprocessing plus sys. Preprocessing therefore runs at about 73% of 6-CPU
  capacity (1.28 / (0.287 × 6)), against about 88% for search (1.06 / (0.20 × 6)). The idle time is in
  preprocessing.

Transfer to the board (**I**, two explicit scenarios; the board has only total T):
- **A, proportional.** Large VPS phase changes moved the board almost one-to-one: step 11 (renumber) VPS
  −23.6% vs board −19.4%; step 12 (search) −35.0% vs −34.6%; step 13 (load) −26.0% vs −22.8%. The board/VPS
  ratio was 0.82–0.99. Smaller steps transferred poorly: SWAR −10.6% vs +2.1%, prefetch −12.5% vs −7.2%.
  Scaling today's VPS shares to 0.207 s gives: load ≈0.040, renumber ≈0.051, transpose ≈0.024, search
  ≈0.086, other ≈0.004 s.
- **B, the requested bound.** Search scales perfectly to 12 hardware threads (2× the 6-vCPU thread count);
  every other phase gains only a per-core factor c. Solving (0.287 + 0.008 + 0.200/2)/c = 0.207 gives
  c ≈ 1.9. The board split is then preprocessing ≈0.150 s (73%), search ≈0.052 s (25%).
- **I** Conclusion common to both: load + renumber + transpose take 0.115–0.150 s on the board. Leaders
  finish the whole run in 0.10–0.12 s, so even a free search cannot reach them; preprocessing must shrink.
- **I** The VPS likely overstates the board's search share. Large queries touch 1–4 MB of graph rows plus
  distance pages, which fit a P-core's 1.25 MB L2 / 24 MB L3 better than Haswell's 256 KB L2.
- **I** Gain bounds, whole workload, scenario A:
  - Search −50% → T −21% → about +2.6 points.
  - Preprocessing −50% → T −29% → about +3.7 points.
  - Removing the serial transpose prefix+alloc (16 ms VPS) → T ≈ −3.4% → about +0.4 points.

## 2. Per-query search cost (Q2)

Source: `2026-11-01-traverse-search-tail/tail.jsonl.gz`, build `21df4fd` plus timers, 4 rounds each at 6
and 12 workers, and `mu-prune/counts.json` (1 thread, exact counts).

- **M** At 6 workers, summed per-query time is 0.97–1.19 s against a makespan of 164–201 ms. Idle inside
  search is 1.3–2.3% (3.4–3.9% at 12 workers).
- **M** Per-query time: median 28–33 µs, p90 1.5–1.8 ms, p99 6.4–7.9 ms, maximum 10.6–21.7 ms.
- **M** Concentration of work (consistent across runs; the same longest indices 782, 678, 763, 1658, 526
  every time):

  | Top share of queries | Share of search work |
  |---|---:|
  | 1% | 16–20% |
  | 5% | 52–57% |
  | 10% | 74–81% |
  | 50% | 99.1–99.6% |

  The 650 queries under 10 µs and the 1,311 under 100 µs carry under 2% of work together. The 275 queries
  over 1 ms carry 83% (6-worker medians).
- **M** The log2 histogram of query time holds 107–240 queries per bucket from 1 µs to 8 ms, nearly flat.
  This matches the spec's log-uniform rank octaves (about 122 queries per octave).
- **I** Rank mapping: about 3,800 pops per query on average, and the mean rank r is ≈12.3k, so pops are
  roughly 0.3·r. Queries over 1 ms are about the top 2 octaves (o = 15–16, r ≥ 32k): about 12–14% of
  queries holding about 85% of search work. No per-query pop counts exist (**U**), so this mapping is not
  measured.
- **M→I** Cost per pop: 1.06 thread-s / 7.57M pops ≈ **140 ns per pop** (VPS, all queries). Each pop
  averages 1.14 pushes and ≈2.9 relaxed edges, and each relaxation reads two distance pages. Pops include
  stale entries: the counter sits before the staleness check (`mu-prune/experiment.py:41`). Unique settles
  are **U**.
- **M** The 12-worker/6-worker per-query time ratio is 2.08 for the top 100 and 1.93 overall. On 6 vCPUs,
  12 workers only time-slice. **The VPS 12-worker data says nothing about board SMT scaling**, and
  12-worker per-query times are not cost measurements (invalid as costs).

Memory-latency bound or heap bound (**I**, no counters):
- Evidence for latency:
  - Disabling renumbering keeps identical pops and heap work, yet search user CPU rises from 2.08 to 5.75 s
    (+2.7×, ≈600 ns per pop). Locality dominates (`no-reorder`, **M**).
  - Prefetching the top-of-heap row cut search by 7–13% (step 17, **M**).
  - 140 ns per pop is about 3× an L1/L2-resident heap step. That gap is consistent with roughly one L3 or
    DRAM miss per pop on Haswell.
- Evidence that the heap matters but less: single-`u64` keys cut search 0.293 → 0.262 s (−11%, step 16,
  **M**). Dial queues were not faster (+1–13%, step 21), and pruning 5.7% of pushes changed nothing
  measurable (step 25).
- Evidence against branch layout: PGO showed no gain (step 24). The PERF claim that "the work is bound by
  cache misses" is an inference, not a measurement.
- **U** The cycle split between heap sifts, row loads, distance-page lookups, and dTLB walks.
  - dTLB: 54 MB of forward and reverse CSR on 4 KB pages (THP disabled) far exceeds the STLB reach
    (about 4–8 MB). Random row access is a plausible dTLB-miss source, untested (**I**).

## 3. Hardware counters (Q3)

- **F** No Track D evidence directory contains hardware counters. A search for perf output, `cpu-clock`,
  or cycles in `tests/performance/*traverse*` finds nothing. Track D has wall-clock phase timers, rusage
  (user, sys, minor faults, context switches), cgroup `memory.current`/`memory.peak` marks, and
  deterministic push and pop counters. It does not even have `cpu-clock` samples; those exist only for
  Track A (`2026-11-01-ingest-perf-flamegraphs`).
- **F** The VPS has no hardware PMU, only software events (`reports/profiler.md:13`).
- **F** rl-lyceum (i7-7700, 4 cores / 8 threads, kernel 4.15, native) has hardware counters
  (`reports/profiler.md:14`). It is the only SMT machine with counters on record.
- **I** Proxies:
  - On rl-lyceum: `perf stat -e cycles,instructions,LLC-load-misses,dTLB-load-misses,branch-misses,page-faults`
    on (a) the full run and (b) a `--threads` sweep 1/4/8. This yields SMT scaling plus instructions per
    cycle (IPC) and misses per pop.
  - On the VPS: cpu-clock sampling (`perf record -e cpu-clock`) for attribution by symbol. Also a
    locality knob (no-reorder is already one point) and an L1-resident microbenchmark (small queries only,
    which run at ≈1–2 µs each) to bound the pure heap cost per pop.

## 4. Where the CPU is idle (Q4)

- **M** About 17–20% of VPS CPU capacity is idle (utilization 0.80–0.83). Search is 97–99% balanced, so
  the idle time is in preprocessing (§1).
- **F** 83 thread spawns per run at 12 workers, across 7 scoped barriers (from source):

  | Phase | Spawns |
  |---|---:|
  | Parse chunks | 12 |
  | Placement (n = 1,740,512 → part_bits 15 → 54 parts, 5 per worker) | 11 |
  | BFS | 12 |
  | Copy | 12 |
  | Transpose (two scopes) | 24 |
  | Search | 12 |

  At 6 workers placement makes 6 spawns (27 parts).
- **I** At ~30–50 µs each, the spawns cost about 3–4 ms: not a bottleneck.
- **I** Placement imbalance: at 12 workers only 11 threads place. The critical path is 5 parts against an
  ideal of 4.43 (the last worker holds about 3.1 parts' worth of nodes), so placement is about 13% longer
  than balanced. The board's 12 hardware threads would leave one idle.
- **F→I** Serial sections on the main thread:
  - Transpose: `degrees` (7 MB of `AtomicU32::new(0)` via `collect`, a serial write plus first touch); the
    prefix sum into `offsets` (7 MB); `cursor` (7 MB). Measured prefix+alloc is 16 ms (**M**), which
    matches 3 × 7 MB at about 0.7 ms per MB of single-thread first touch (`page-fault-bench.rs`: 20 ms per
    28 MB).
  - Renumber: `claims` (7 MB serial) and `new_offsets` (a 1.74M push loop, 7 MB).
  - Together these are an estimated 10–15 ms (**I**, not timed).
  - Smaller items: `drop(graph)` (27 MB munmap), the `heavy` sort (3,496 entries), query parse and output
    (~8 ms including process start and exit, **M**).
  - **I** Identified serial total ≈ 35–45 ms of the ≈130 ms effective-serial budget. The remaining
    ≈85 ms is unattributed; candidates are BFS imbalance, CAS and atomic contention (claims, in-degree
    counting, scatter cursors), fault and mmap-lock contention, and bandwidth.
- **U** BFS load imbalance: no per-worker finish time or region size is archived. CAS claiming partly
  self-balances (a slow worker claims fewer vertices), but the tail is unmeasured.
- **I** Transpose's count and scatter are atomic random increments over 5M edges, 15 + 24 ms at 12 workers.
  The recycle variant showed this contention costs +31 ms when moved into the copy (step 22).

## 5. Peak memory (Q5)

- **M** cgroup marks from `memory-diet/memphase-balanced/rows.json` (12 workers, VPS, build `3b648eb`; the
  current code's peak is within ±0.5%, step 20):

  | Mark | MB |
  |---|---:|
  | start | 2.0 |
  | buckets_parsed | 50.5 |
  | **csr_placed** | **78.8 (peak)** |
  | graph_loaded | 33.5 |
  | order_built | 48.0 |
  | copied | 74.7 |
  | old_graph_dropped | 45.7 |
  | transposed | 58.7 |
  | searched | 31.8 |

  The board's 79.62 MB matches the VPS's 78.8–79.1 MB once THP is disabled (**F**, step 15).
- **I** Composition of the peak: buckets ≈5M × 8 B = 40 MB plus ≈5 MB of `Vec` growth slack (648 = 12 × 54 per-chunk
  bucket `Vec`s of ≈62 KB each, under the 256 KB `mmap` threshold, so they live in glibc arenas), plus the
  CSR (offsets 7.0 MB + edges 20 MB = 27 MB).
- **I** The copy mark is old CSR 27 + new 27 + claims 7 + orders 7 + new_offsets 7 = 75 MB. The transpose
  scatter's peak between marks is about 27 + 20 + 7 + 7 + base ≈ 63–66 MB.
- **U** Peaks inside phases (between marks) and inside search are unmeasured.
- **M/U** Freeing each part's buckets after placement ("freepart", 256 KB and 32 KB thresholds) did not
  move the peak (78.7 MB, +6–8.5% time), even though RSS fell 20 MB with the 32 KB threshold. Why is
  unexplained. Either the peak moved to an unmarked point of about 78.7 MB, or the cgroup charge was not
  released.
- **I** Cheapest path toward 65 MB:
  1. Cutting load alone is capped by the copy at ≈75 MB: −5%, ≈+0.23 points.
  2. Reaching about 65 also needs the copy phase cut by ≈10 MB, for example by not holding `claims`,
     `orders` and `new_offsets` alongside both CSRs.
  3. The cleanest load cut is to drop the buckets entirely: count degrees first, then place directly from a
     second SWAR parse. The load peak becomes about 35 MB, at the cost of one more parse pass (the parse
     share of ~0.09 s VPS load is not separated, **U**).
  4. Full 79.6 → 65 MB is worth only +0.88 points (0.2·log10). About 8% T is worth the same.

## 6. Comparisons: reliable, noisy, invalid, missing

- **Reliable (M):**
  - Deterministic push and pop counts.
  - No-reorder control (0/8 rounds faster, +124–133%).
  - Memory boundary marks (cgroup `memory.current`).
  - Phase shares reproduced across four directories.
  - Large-step VPS→board transfers (steps 11–13).
  - Board noise statistics over 21 runs.
- **Noisy (M):**
  - VPS totals: spreads of 10–35% swamp effects under about 5% (edgescan, μ-prune, PGO, blobs 512,
    Dial pooled +1.3%).
  - Single board runs: CV 3.3%, range 0.199–0.224 s. Steps 16, 18, 19 and 20 board deltas are inside or
    near this band.
- **Invalid as used (I):**
  - 12-worker per-query times on 6 vCPUs as cost.
  - `max_rss` (166 MB includes the file-backed mapping) as memory; cgroup peak is the board-relevant
    number.
  - KVM `lscpu` cache sizes.
  - "VPS cache larger than board" (step 23) and "bound by cache misses" (step 24) as findings rather than
    hypotheses.
  - VPS 12-versus-6-worker results as an SMT proxy.
- **Missing (U):**
  - Board phase split.
  - Single-thread per-phase times.
  - Sub-phase timers (parse/place, BFS/copy).
  - BFS per-worker end times.
  - Hardware counters.
  - Per-query pop counts.
  - Peaks inside phases.
  - SMT scaling on any machine for D.

## 7. Bottleneck hypotheses (ranked by estimated K per engineering hour)

**H1. Preprocessing has an effective-serial floor that does not shrink with 12 threads.**
- Mechanism: serial 7 MB zero-and-collect plus prefix passes (claims, `new_offsets`, degrees, offsets,
  cursor), 7 barriers, 11-of-12 placement, BFS tail, atomic contention.
- Falsifiable prediction: with `--threads` 1 versus 6, transpose and renumber speed up < 3.5×, while
  search speeds up ≥ 5×. If every phase reaches ≥ 4.5×, H1 is false.
- Experiment: the existing `PHASES=`/`TRANSPOSE=` build plus two timers (parse|place, BFS|copy) and
  per-worker BFS end times, at 1/6 threads.
- Fix candidates if confirmed: parallel prefix sums, zeroed `Vec` allocated via calloc instead of a
  serial `map().collect()`, and placement split into 12 groups.
- Correctness risk: low. Exact offsets are testable by the existing all-pairs and loader tests.
- **Estimate:** −15–30 ms VPS, board −5–10% T, +0.5–1.1 points, about +5–15 lines of source (S cost
  ≈ −0.05 points per 10 lines).

**H2. The search is memory-latency and dTLB bound on the top two octaves.**
- Mechanism: about 1 L3/DRAM miss per pop, each relaxation touching two distance pages (forward and
  backward) in separate arrays, and 4 KB pages over 54 MB of CSR.
- Prediction: on rl-lyceum, LLC misses and dTLB misses per pop are each ≥ 0.5 for queries over 1 ms and
  near 0 for queries under 100 µs. Cost per pop rises with query size.
- Experiments:
  - (a) Measure only: perf counters plus per-query pop counters.
  - (b) One change: interleave the forward and backward `u32` distances in one shared page, as `[f, b]`
    pairs, so both lookups hit one cache line.
  - (c) One change: `MADV_HUGEPAGE` on the two CSR edge arrays only. The VPS runs THP in `madvise` mode,
    so this is testable there. It requires replacing the process-wide `PR_SET_THP_DISABLE` with targeted
    advice or `MADV_NOHUGEPAGE` on sparse buffers. Memory risk: ≤ 2 MB rounding per array, but step 14
    showed what THP on sparse buffers costs.
- Correctness risk: (b) touches page reset and overflow paths; (c) none.
- **Estimate:** search −5–15% → board T −2–6%.

**H3. Fault and allocation churn: about 190 MB first-touched in 4 KB pages (48k minor faults) and
sys ≈ 10% of CPU.**
- Mechanism: every phase allocates fresh 7–27 MB buffers and frees them. With THP off, each 4 KB fault
  costs about 0.7 µs per KB single-threaded, and faults on a single address space contend.
- Prediction: sys time scales with first-touched bytes. Reusing the dropped old CSR for the transpose
  without atomics in the copy (unlike recycle) cuts minor faults by ≥ 6k and sys by ≥ 20 ms CPU.
- Experiment: rusage minor faults and sys per variant on the VPS.
- Note: the board kernel's fault cost is **U**, and KVM inflates it.
- Correctness risk: low.
- **Estimate:** ≤ 2–4% T. Partly overlaps H1's allocation items.

## 8. Smallest missing measurement that would change the ranking

Run the existing phase-timer build (PHASES + TRANSPOSE) with two extra timers (end of parse, end of BFS)
and per-worker BFS end timestamps, at `--threads 1` and `--threads 6` on the VPS. Use 3–5 alternating
rounds and archive the exact source. Per-phase parallel efficiency decides between "fix preprocessing
scaling" (H1/H3) and "speed up search" (H2), and fixes the board-split scenario (A versus B). It reuses
`transpose/edgescan/experiment.py` unchanged apart from the thread list. If rl-lyceum is available, the
same binary at 1/4/8 threads under `perf stat` also answers SMT scaling and H2's miss rates in one run.
