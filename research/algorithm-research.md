# Track D algorithm and systems research (2026-11-01)

Researcher report, read-only. Baseline is `track-d/src/main.rs` on `exp/d-mu-prune` (841 lines, S=723).
Board: T 0.199–0.224 s, M 79.6 MB, 65.39 pts, rank 32/104 (live feed `generated_at` 2026-11-01T15:11:55Z).
Tags: **[F]** fact read from source/spec/feed. **[M]** measured, with whose measurement it is. **[I]** inference.
**[U]** untested estimate. Points: ΔP = 25·log10(T0/T1) + 10·log10(M0/M1) + 7.5·log10(S0/S1)
([scoring.py](https://github.com/hackathon-raptors/speed-demon-2026/blob/main/harness/scoring.py)).

## 1. Answer to the central question

One change will not give a structural 2×. No public solution near 0.1 s has one either. The fastest public
Track D code is HighVelocity (#8, best 0.110 s / 67.8 MB / S 783). Its development history shows the gap
is closed by stacking 5–8 mechanisms, each worth 5–15%. Three of them change what the hardware does, not
just constants:

1. **Memory-level parallelism (MLP) in the search.** A binary heap pops one vertex at a time, so
   at most one row miss is in flight. Draining a whole 128-wide distance bucket in FIFO order lets the
   search prefetch the row and label 4 entries ahead. **[M, HighVelocity]** That change cut query time
   268→230 thread-ms, and branch-free relaxation cut it 294→244.
2. **Graph reduction.** Bypassing out-degree-≤2 vertices that are not query endpoints shortens every
   search. **[M, HighVelocity]** Whole-process time fell 0.264→0.230 s (4 workers) and 0.274→0.238 s
   (12 workers); they won all 30 paired comparisons against thresholds 0 and 1.
3. **Box-specific memory behaviour.** The box is an i5-13500 with a 24 MB L3 **[F, FlyHigh README; harness
   README `--cpuset 12-19` for E-cores]**. Our two packed graphs (≈54 MB) do not fit in L3, and with THP
   disabled every random access also risks a dTLB walk. **[M, FlyHigh board]** Disabling THP for their
   whole process cost 0.196→0.223 s. **[M, ours]** Step 15 cost 0.243→0.253 s, a single run.

The phase split on the box is close to the VPS. **[M, FlyHigh exit-code builds on the box, 0.155 s total]**:
parse 15 ms (ends 21 ms after start), CSR 12, BFS 20, new ids + copy 15, reverse weights 17, queries ≈65.
Queries are therefore about 42% and construction about 55%. Our VPS split is load 0.10, renumber 0.115,
transpose 0.055, search ≈0.20 of 0.47 s, so the search is again ≈42%. **[I]** On the board we probably spend
≈85 ms searching and ≈110 ms building. Halving T means removing ≈50 ms from each side. The 8 hypotheses
below add up to roughly −25% T (≈0.155 s) if most succeed **[U]**. That is a realistic ceiling for the
23 hours before freeze, and it is not 0.1 s.

## 2. Public competitor code (leads, not evidence for our code)

The leaders' repositories are private or not indexed. GitHub commit search by board hash found nothing for
Lex, Cause Of Death, SweetDreams, Featherweight, Kirk Mates, Allegro, ALL STAR, AVENGERS, Cosmic, PeppaPig,
Team7, Glitter, NewtonRedHeat, ttimesai, Pal Ghevariya and Yasaswi P **[F]**. Three hashes matched:

| Team (rank, best) | Repo @ revision read | Lang/S |
|---|---|---|
| HighVelocity (#8, 0.1103 s, 67.8 MB) | [insightlabs38-pixel/sd-sm](https://github.com/insightlabs38-pixel/sd-sm/blob/7753f18622d0ff3b3da3c4434c135b9724d623e1/track-d/d_traverse.cpp) `7753f18`; dev repo [sd-td](https://github.com/insightlabs38-pixel/sd-td) `b564f89` | C++, 783 |
| Cache Me (#16, 0.1677 s, 70.1 MB) | [manusingh090/speed-demon-2026-entry](https://github.com/manusingh090/speed-demon-2026-entry/tree/3ca2a3e86335f3f5ff4e5a45f952f7c07c46cfd2/track-d) `3ca2a3e` | Rust, 516 |
| Team FlyHigh (#19, 0.1507 s, 83.0 MB) | [Harini-vm/speed-demon-harini](https://github.com/Harini-vm/speed-demon-harini/tree/6feb4ffbb33cefb4143bf0ef6c2c35ad26277cbc/track-d) `6feb4ff` | Rust, 660 |

How HighVelocity differs from us at code level **[F, d_traverse.cpp]**:
- **Search queue.** A ring of 2048 buckets, each 128 distance units wide (`Queue`, shift 7), plus a
  `priority_queue` for far keys and an occupancy bitmap. `Side::drain` expands a whole bucket in FIFO
  order. Entries pushed into the current bucket are appended and drained in the same pass, so the search
  is label-correcting inside a bucket and stale entries are skipped (`dist != d`). Each step prefetches
  the row and label of entry k+4. Relaxation is branch-free (`*x = take ? nd : old; push(nd, v, take)`),
  and the meeting update runs unconditionally. Sides are balanced by settled count (`fs <= bs`).
  Stopping rule: `f + b >= best` on bucket lower bounds. Labels are u64 in 128-vertex pages.
- **Graph layout.** No offsets array. Each vertex has 4 inline u32 slots (16 B), and a vertex with more
  than 4 edges keeps 3 inline plus a spill code (`Rows`). **[M, sd-td]** −3% search time. **[M, FlyHigh on
  the box]** The same idea with 8-byte entries was worse: 168 ms / 108 MiB against 155 ms.
- **Fused build.** One pass does degree-2 bypass, BFS regions and gather (`Grower`). Claims are plain
  stores into a shared bitmap, with a private first-check bitmap and duplicates tolerated. The `where`
  map is written in a separate scatter pass.
- **Transpose without atomics** (`add_reverse`). Each worker owns an ID slice, counts local in-edges
  directly and passes foreign in-edges through per-pair inboxes. **[M, sd-td]** 79→57 thread-ms.
- **Parsing.** SSE2 `cmplt '0'` gives a 64-bit separator mask, followed by SWAR decimal conversion. Bins
  use 6-byte entries (`Entry {u32 key; u16 w}`). **[M, sd-td]** parse 120→97 thread-ms.
- **Build workers.** 4 build workers and 12 query workers (`min(4u, threads)`). **[M, HighVelocity board]**
  An earlier 4-build cap coincided with 0.122→0.170 s on the box, and their notes contradict each other,
  so we should not copy this cap.
- **THP.** `MADV_NOHUGEPAGE` only on sparse arrays (`Array(n, sparse=true)`), so dense graph arrays keep
  THP. `std::exit(0)` skips destructors.
- **Rejected by them with measurements:** an exact binary heap inside each bucket (+44% wall); ALT (cost
  model: 2 landmarks ≈ 4 SSSPs ≈ the whole query phase); CH/witness contraction; 3-slot rows (+8–11%);
  forward probe 0/32/128/256 (neutral); query-order locality. Sources:
  [sd-td research](https://github.com/insightlabs38-pixel/sd-td/blob/b564f89aea5334b3a2b00367e66c4657875e6cd2/research/d-architecture-fused.md)
  and [sd-td PERF.md](https://github.com/insightlabs38-pixel/sd-td/blob/b564f89aea5334b3a2b00367e66c4657875e6cd2/track-d/PERF.md).

What FlyHigh does that nobody else does **[F, README]**: **there is no reverse graph.** Each edge is 8 B
(head, weight, reverse weight or "absent"). The ≈144k one-way in-edges sit in a side list in blocks of 64
heads, guarded by a bitmask. **[M, FlyHigh box]** Queries took 80 ms with two graphs and ≈56–65 ms with one
structure, and the transpose (33 ms of 196 with atomics) became a 17 ms reverse-weight pass. The board
change was only 0.196→0.191 s in a single run, so this is a lead **[I]**. They also release THP selectively
(off while buckets fill, on for the graph arrays), sort queries by source in runs of 4, and use a radix
heap with a bitmask.

Cache Me **[F/M, PERF.md]**: 6-byte bucket records. One pool of threads started once and held at barriers.
Their reason: each phase spawned 12 threads, and new stack mappings waited behind page faults and unmaps,
so the last thread started 1–18 ms late. Laptop median 165.6→155.7 ms, and the board went
0.183/0.196→0.172/0.168 s. A 4-ary heap equalled a radix heap. Unmapping the input early **hurt** on the
box (0.172→0.190 s).

Already common to all of us: mmap with bucketed CSR, BFS-region renumbering, packed u32 edges with a heavy
side list, paged labels, bidirectional search with queue-size or settled balancing, `mallopt`.

## 3. Facts about our current build relevant to systems costs

- **[F]** There are **84 thread spawns in 7 `thread::scope`s per run**: parse ≤12, placement ≤12, BFS 12,
  copy 12, transpose count 12 + scatter 12, queries 12. Each spawn mmaps a 2 MiB stack plus a guard page.
- **[F]** Several steps run serially on the main thread between scopes:
  - `claims` is built as a `(0..n).map(AtomicU32::new(0)).collect()`, 7 MB written by one thread.
  - The transpose builds `degrees` and `cursor` the same way (7 MB each) and runs its prefix sum serially.
  - The reorder computes `new_offsets` and `node_base` serially.
  - **[M, ours, step 22]** The transpose spends 16 ms of 51–62 ms in "prefix and allocation".
- **[F]** With THP disabled for the whole process, the graphs searched (forward and reverse, ≈54 MB in
  total) are backed by 4 KB pages. **[I]** Golden Cove's STLB holds 2048 entries, which covers about 8 MB
  of 4 KB pages, so almost every random row or label access in the search also misses the TLB.
- **[F]** The memory peak is at the end of placement (78.8 MB), when the 8-byte bucket records (≈40 MB)
  and the first CSR are live together. The renumber copy (74.7 MB) is next.

## 4. Ranked hypotheses (central estimates are [U] unless noted)

| # | Hypothesis | Class | ΔT | ΔM | ΔS | ΔP | Hours | ΔP/h |
|---|---|---|---:|---:|---:|---:|---:|---:|
| 1 | THP back on for the dense final graphs only | minimal-risk / memory-systems | −5% | ≈0 | +8 | +0.52 | 1.5 | **0.35** |
| 2 | 6-byte bucket records (u16 source offset + u32 packed) | memory layout / construction | 0 | −6% | +4 | +0.25 | 2 | 0.13 |
| 3 | FIFO bucket-drain queue with 4-ahead prefetch and branch-free relaxation | algorithmic (MLP) | −8% | 0..+3% | +40 | +0.70 | 6 | 0.12 |
| 4 | Balance sides by settled count instead of queue length | query-distribution, minimal | −1% | 0 | +2 | +0.10 | 1 | 0.10 |
| 5 | Exact degree-2 bypass contraction inside renumbering | algorithmic (graph reduction) | −10% | −2% | +90 | +0.85 | 9 | 0.09 |
| 6 | Region-owned transpose without atomics; parallel prefix; no cursor array | construction | −3% | 0 | +20 | +0.25 | 3 | 0.08 |
| 7 | One persistent worker pool with barriers (84 → 12 spawns) | systems | −3% | 0 | +15 | +0.26 | 4 | 0.06 |
| 8 | SIMD separator mask in the parser (reuse Track A's AVX2 scan) | parsing | −1.5% | 0 | +20 | +0.08 | 2 | 0.04 |

None of these have been tried in this repository. #3 is not the rejected Dial queue (see below).
Expected sums are not additive; #3 and #5 both shrink search work.

### H1. Huge pages for the dense search graphs only (minimal risk)
- **Mechanism.** Turn THP back on just while the dense, fully written arrays are allocated and first
  touched: `new_offsets`, `new_edges`, and the transpose's `offsets` and `edges`. Call
  `prctl(PR_SET_THP_DISABLE, 0)` and also `madvise(…, MADV_HUGEPAGE)` on page-rounded ranges, because the
  VPS runs THP in `madvise` mode. Set the flag back to 1 before the query workers create `Side` pages.
  THP is checked at fault time **[I, kernel `hugepage_vma_check`]**, so pages already mapped huge stay
  huge. This targets dTLB walks in the search, which is about 42% of T, and in the transpose scatter.
- **Difference from step 15.** Step 15 turned THP off for everything to cut memory. The memory cost came
  from *sparse* buffers: reserved region buffers and distance pages that have since been removed. Dense
  arrays are fully written, and THP maps only aligned 2 MB extents inside a VMA, so they gain no resident
  slack **[I]**. FlyHigh does exactly this on the box (see §2).
- **Estimates.** T −3..−8% (central −5%). M ≈0, because the peak stays in the load phase where THP remains
  off. S +6..+10. No dependencies.
- **Correctness.** No logic changes. `madvise` must receive a page-aligned start and a length rounded
  down. Ignore failures. Keep THP off while the BFS `order`/`degrees` Vecs grow by doubling.
- **Experiment.** Change only this. On the VPS with `madvise` mode, run baseline and candidate alternately
  with identical builds, 12 workers, ≥10 pairs. Record `AnonHugePages` from `/proc/self/smaps_rollup`
  just before the search and cgroup `memory.peak`. Then do 2 board runs.
- **Rejection rule.** Reject if `AnonHugePages` is 0, if the VPS search phase is not faster in ≥7/10
  pairs, or if peak M rises by more than 1 MB.

### H2. 6-byte bucket records to lower the load peak (memory layout)
- **Mechanism.** Buckets currently hold `(u32 source, u32 packed)`. The source only needs `part_bits`
  bits (15 at full scale), so store a `#[repr(C, packed)] (u16 offset_in_part, u32 packed)` record and cap
  `part_bits` at 16. Heavy weights stay in the existing side Vec. This removes 2 B × 5M = 10 MB at the
  measured peak (end of placement). **[I]** The peak then moves to the renumber copy (74.7 MB), so the net
  is ≈ −5 MB.
- **Precedent.** Cache Me and HighVelocity both use 6-byte entries **[F]**. It is not part of the step-19
  memory diet, which went from 12 to 8 bytes.
- **Estimates.** M −6% (79.6 → ≈75), T ±1% (fewer bytes written while parsing), S +4.
- **Correctness.** Graphs with more than 2^16 × parts vertices need the `part_bits ≤ 16` cap to still
  cover all IDs, so `parts` grows. Packed fields must be copied, never borrowed. Test `n` that is not a
  power of two, plus the 1-vertex and no-edge fixtures.
- **Experiment and rejection.** Measure the VPS cgroup peak with phase marks, 6 pairs. Reject if the peak
  falls less than 3 MB or if T rises more than 2%.

### H3. FIFO bucket drain with lookahead prefetch (algorithmic, memory-level parallelism)
- **Mechanism.** Replace each side's `BinaryHeap<u64>` with a 2048-bucket ring that is 2^7 distance units
  wide, plus a far `BinaryHeap` and a `[u64; 32]` occupancy mask. To advance one side, drain its whole
  current bucket in FIFO order:
  - for entry k, prefetch `offsets[node_{k+4}]`, its first edge, and its label slot;
  - skip stale entries;
  - relax branch-free, with an unconditional meeting update;
  - append improvements, which may land in the bucket being drained.
  The stop test `lb_f + lb_b >= μ` uses bucket lower bounds (`cur << 7`). This targets the serial miss
  chain of heap pops. **[M, HighVelocity]** FIFO drain + lookahead: 268→230 thread-ms; branch-free:
  294→244 thread-ms; an exact heap inside each bucket: +44% wall.
- **Why the rejected Dial (step 21) is not evidence against this.** That version
  ([`dial-main.rs`](../tests/performance/2026-11-01-traverse-dial-queue/vec-buckets/dial-main.rs)) used
  width-1 buckets with a cursor that stepped one label at a time (≈120 empty probes per settle at a
  median weight of 120). It popped one entry per call, alternated sides per settle, and prefetched only
  the next entry's row. It reduced queue cost, which Cache Me and FlyHigh also found does not matter, and
  it added no MLP. Its memory rise came from 4096 persistent `Vec` buckets per side. This proposal keeps
  2048 and resets only the lengths.
- **Estimates.** Search −15..−25% → T −6..−11% (central −8%). M +0..+2 MB, from bucket capacity of
  12 workers × 2 sides × occupied buckets. S +30..+50.
- **Correctness.**
  - Labels are u32. Keys at or above `cur + 2048·128` go to the far heap.
  - Zero-weight edges re-enter the current bucket and must be drained in the same pass (loop while
    `k < len`).
  - The u32-overflow flag and `u64` fallback stay unchanged.
  - μ is updated only with real path sums.
  - The stop test uses lower bounds that never exceed any queued key, which keeps it exact. Once a
    bucket is fully drained, every label below `(cur+1)·128` is final (Δ-stepping light phase argument,
    [Meyer & Sanders 2003](https://doi.org/10.1016/S0196-6774(03)00076-2)).
  - Tests: the existing all-pairs random-graph reference (weights 0 / small / ~2^31 / ~2^32), zero-weight
    chains longer than 128, islands, and weights of exactly 128·2048.
- **Experiment.** Change only the queue and drain, and keep the queue-size balancing so H4 stays
  separable. Count expansions and stale skips at 1 thread on seed 1. Run VPS pairs at 6 and 12 workers.
- **Rejection rule.** Reject if the search phase is not ≥8% faster at 12 workers in ≥7/10 pairs, or if
  the peak rises by more than 3 MB.

### H4. Settled-count side balancing (query-distribution specific, minimal)
- **Mechanism.** Grow the side that has settled fewer vertices, instead of the side with fewer queued
  entries. Both target ranks and source ranks are drawn per query (log-uniform rank 2^1..2^17). For a
  symmetric road graph, the optimal split equalises settled work, not frontier size.
- **Evidence.** **[M, sd-td]** 31-pair CI: 0.2522→0.2490 s at 6 workers (26/31 wins) and 0.2589→0.2554 s
  at 12 workers (23/31). Strict alternation was worse.
- **Islands and floods.** An exhausted side still ends the loop. A near-2^32 first edge does not flood
  the graph, because the counts force alternation **[I]**.
- **Estimates.** T −0.5..−1.5%, S +2. This is below the board's single-run noise (±6%), so only many VPS
  pairs can judge it. Reject unless ≥14/20 pairs win.

### H5. Exact degree-2 bypass contraction (graph reduction)
- **Mechanism.** At full scale the average out-degree is 2.87 **[F, 5M/1.74M]**. A vertex y with
  out-degree ≤2 that is not a query endpoint, after excluding the edge back to the predecessor and any
  self-loop, has one forced continuation. Every path through y continues to that single other head, so
  the edge u→y can be replaced by u→next with weight w(u,y)+w(y,next), following the chain until a
  survivor is reached.
- **Evidence.** **[M, sd-td]** −13% wall at 4 and 12 workers; 12-worker peak RSS 77→69 MB. Search graph
  ≈1.2M of 1.74M vertices; 5.38M settles against our 7.57M pops.
- **How it fits our code.** During the BFS, do not enqueue bypassable vertices. In the copy, resolve each
  head through its chain, up to 64 hops, keeping a u64 sum and sending it to `heavy` if needed. If y turns
  out to have two non-predecessor heads, the walk must stop at y and y must survive. HighVelocity claims
  y lazily in that case. In our two-phase design, the safe rule is to mark y bypassable only if *every*
  in-neighbour is one of its ≤2 out-heads, which is symmetric two-way chains only. That rule is simpler,
  and it is weaker **[U: share of vertices unknown; measure first]**.
- **Estimates.** T −6..−13%, M −2..−5% (smaller final graphs; the load peak is unchanged unless H2 lands),
  S +70..+110, 8–10 h.
- **Correctness.** Query endpoints must never be bypassed, and the −1 checks must read survivor rows.
  Also cover: zero-weight chains, cycles made only of bypassable vertices (a dead loop must become a
  self-loop or vanish), parallel edges into a chain, and a chain sum above 2^32.
- **Experiment.** First add a counter-only build that reports the bypassable fraction under the strict
  rule on seeds 1, 7 and 42 (≈1 h). Implement only if that fraction is ≥20%. Reject if VPS T is not ≥6%
  faster in ≥7/10 pairs.

### H6. Region-owned transpose without atomics (construction)
- **Mechanism.** After renumbering, worker w's region is a contiguous ID range, and most edges stay
  inside it **[I]**. Make the transpose owner-computes, as HighVelocity's `add_reverse` does:
  - each worker scans its own rows and counts in-range targets with plain increments;
  - cross-range edges go to small per-pair inboxes;
  - the prefix sum is parallel over owner ranges;
  - slots are filled by a decrementing in-place cursor, so there is no separate 7 MB `cursor`;
  - arrays are allocated with `vec![0; n]` (calloc, first-touched by their owners) instead of a serial
    `collect` of atomics.
  This removes 2×5M contended `fetch_add`s and ≈16 ms of serial work (VPS).
- **Difference from step 22.** Step 22 moved the counting into the renumber copy and kept atomics. This
  removes the atomics and the serial pieces.
- **Estimates.** Transpose −30..−50% → T −2..−5%, S +15..+25, M unchanged.
- **Correctness.** The reverse rows must hold the same multiset as before; the `heavy` index must match
  the final slot. Owner ranges come from `node_base`, and an empty region is allowed. Reject if the
  transpose phase is not ≥25% faster.

### H7. One persistent worker pool (systems)
- **Mechanism.** One `thread::scope` with 12 workers and `std::sync::Barrier` between phases. Serial steps
  (prefix sums, `node_base`) run on worker 0 between barriers. This replaces 84 spawns and 7 joins with
  12 and 6 barrier waits, and it avoids stack-mapping `mmap` calls queueing behind page-fault
  `mmap_lock` holders.
- **Evidence.** **[M, Cache Me]** −6% laptop; board 0.18–0.20 → 0.17 s, confounded by other changes.
- **Uncertainty.** Kernels ≥6.4 use per-VMA fault locks, which shrinks the mechanism **[I]**. The box
  kernel is unknown.
- **Estimates.** T −1..−5%, S +10..+20.
- **Experiment.** First add only per-thread start timestamps relative to the spawn (cheap), and implement
  only if the median start skew is ≥2 ms per phase.

### H8. SIMD separator mask in the parser (parsing)
- **Mechanism.** One 64-byte block gives a separator bitmask, and the number boundaries come from
  `tzcnt`. The existing SWAR digit combine stays. The Track A AVX2 newline scan (commit `6d5802b`) is
  reusable. **[M, sd-td]** 120→97 thread-ms; **[M, FlyHigh]** 49.5→43 ms locally.
- **Estimates.** The parse is ≈10% of box T **[I]**, so T −1..−2%. Low priority, because S and risk
  roughly cancel the gain.

## 5. Investigated and not recommended

- **ALT / landmarks.** One SSSP over the main body costs ≈1.7M settles ≈ 0.23 of our whole search work
  (7.57M pops), on one thread. ALT needs forward and reverse SSSPs per landmark. Even 4 landmarks,
  computed in parallel, add ≈8 SSSP-equivalents ≈ 1.8× the query phase's work before any query runs.
  Typical road-network ALT savings are 3–10× fewer settles at 16 landmarks, which cannot pay back at this
  query volume ([Goldberg & Harrelson, SODA 2005](https://www.microsoft.com/en-us/research/wp-content/uploads/2005/01/soda05.pdf)).
  **[I]** HighVelocity reached the same conclusion **[F]**.
- **Contraction hierarchies or partial CH.** Witness searches cost about 0.5 µs or more per contracted
  vertex. For 600k vertices that is ≥0.3 thread-s, close to our whole search
  ([Geisberger et al. 2008](https://doi.org/10.1007/978-3-540-68552-4_24)). Only witness-free degree-2
  reduction (H5) pays.
- **Radix heap / 4-ary / pairing heap.** **[M]** Cache Me, FlyHigh and HighVelocity all found these equal
  to a binary heap. Queue arithmetic is not the cost; misses are. See H3.
- **Building the reverse during load instead of transposing.** **[M, FlyHigh box]** Bucketing by head was
  −4% T but +16% M. The reverse must use renumbered IDs anyway. FlyHigh's alternative, one 8-byte
  structure for both directions, is a full rewrite (≥10 h): high upside on the box's 24 MB L3, too risky
  for the freeze.
- **4-slot inline rows.** Mixed evidence: −3% at HighVelocity, worse at FlyHigh. Memory is equal.
- **`exit(0)` / leaking graphs.** The kernel tear-down is still timed until exit, and Cache Me measured
  early unmapping as worse. Expected gain ≈0.
- **Forward-only probe for short queries.** **[M, sd-td]** Probe 0/32/128/256 was neutral.
- **Query ordering for L3 sharing.** Tried here and by HighVelocity; not promoted by either.

## 6. Missing measurements that would change the ranking

1. `AnonHugePages` and dTLB miss counts (`perf stat -e dTLB-load-misses`) during search, with THP on and
   off, on the VPS. Decides H1.
2. The bypassable-vertex fraction under the strict rule. Decides H5.
3. Per-thread phase start skew. Decides H7.
4. Search expansions plus stale skips per query for H3 against the heap, at 1 thread.

## 7. Evidence paths

Ours: `track-d/src/main.rs`; `track-d/PERF.md` steps 15, 19, 21–26; `tests/performance/2026-11-01-traverse-{dial-queue,transpose,search-tail}/`.
Official: [spec](https://github.com/hackathon-raptors/speed-demon-2026/blob/main/specs/d-traverse.md), [harness](https://github.com/hackathon-raptors/speed-demon-2026/blob/main/harness/README.md), [board.json](https://s3.amazonaws.com/raptors.dev/speeddemon.dev/2026/board/board.json) (fetched 2026-11-01T15:11:55Z).
Competitor clones (untrusted, not built or run): `/private/tmp/claude-501/-Users-pavelkushneryk-Documents-vsprojects-kaggle-notebooks-speed-demon/125bdc10-ed9d-4c4f-ac31-9d79ac9935d2/scratchpad/research-dl/repos/` (`sd-sm`, `sd-td`, `cacheme`, `flyhigh`, `upstream`).
