# Track D historian report (2026-11-01, read-only)

Tags: **F** fact from a file/feed (path given) · **M** measured (environment named) · **I** inference · **U** unknown.
Feed copy: `$SCRATCH/board/kush.json` (generated 2026-11-01T14:03Z; 46 D runs, all `ok`), where
`$SCRATCH=/private/tmp/claude-501/-Users-pavelkushneryk-Documents-vsprojects-kaggle-notebooks-speed-demon/125bdc10-ed9d-4c4f-ac31-9d79ac9935d2/scratchpad`.
That history has no run IDs; run IDs below come from PERF.md, evidence READMEs, or the parent's feed check.

## 0. State at start

- **F** Branch `exp/d-mu-prune`, HEAD `539b15f`. Only `track-d/PERF.md` is modified (+18 lines: steps 25, 26).
  Untracked D evidence: `tests/performance/2026-11-01-traverse-{mu-prune,no-reorder}`. The other untracked
  `2026-11-01-*` dirs (ingest-perf-flamegraphs, track-a-*) are Track A and out of scope.
- **F** Scored source identity: `track-d/src/main.rs` sha256 `459900905ca4c8b9…09b`, `Cargo.toml` `ca61ff94…3f20`,
  `Dockerfile` `70afec48…bc6a`. These are byte-identical from `21df4fd` to HEAD. The mu-prune and no-reorder
  evidence records the same `before-main.rs` hash (`459900905ca4…`).
- **F** `git stash@{1}` "track-d optimizations (pre-baseline revert)" (2026-10-30, +138/−61 in main.rs) is an
  old pre-baseline stash. It is not part of any measured lineage. **U** whether it holds anything not later
  re-implemented. It is probably superseded (I).

## 1. Best verified board version vs best local candidate

| | Identity | Run | T (s) | M (MB) | S | Points | Evidence |
|---|---|---|---:|---:|---:|---:|---|
| Board best (D) | code `21df4fd`, pushed as `14001f9` | run-8040ef5bc533 (parent feed check) | 0.198594 | 79.65 | 723 | 65.392 | kush.json `c=14001f916f75` |
| Same binary, 21 board samples | `21df4fd` code | 21 runs, 2026-10-31 21:44 → 11-01 14:03 | mean 0.2069, sd 0.0069, median 0.2061, range 0.1986–0.2241 | 79.62 ± 0.21 | 723 | mean ≈ 64.95 | kush.json (**M**, board) |
| Best local candidate | **none beats HEAD**. The VPS baseline is the HEAD binary | — | 0.469–0.477 (12 w), 0.476–0.492 (6 w) | 78.7–78.9 cgroup peak | — | — | mu-prune/no-reorder `summary.json` (**M**, VPS) |

- **M** Bootstrap of the final protocol (middle 3 of 5) from the 21 samples: 5–95% band 0.2014–0.2124 s,
  median 0.2061 s. **I** The expected final score of this binary is ≈ 64.9–65.0 (same-seed assumption).
  65.39 is the best of 21 draws, not the expected value.
- **F** PERF.md's bolded team best (65.160080, `run-abe73b97cc7d`) is stale. `run-8040ef5bc533` (65.39) and the
  other 19 reruns are not in PERF.md. The PERF table also mixes "first measurement" rows with re-sample rows.

## 2. Attempted mechanisms: status, scope, environment, reason

Noise yardsticks: **M** VPS max/min spreads were 6–36% per arm (trimmed means of 5–8 rounds), so the VPS
resolves roughly ≥5–8% whole-run effects. **M** On the board, one run has CV 3.3%. The difference of two single
runs therefore has sd ≈ 4.7%, and a 95% band of about ±9%. Memory noise is small on both (board sd 0.27%;
the VPS cgroup peak is stable to within ~1 MB). Memory effects are therefore resolvable where time effects are not.

### 2a. Accepted (board-confirmed or kept)
| Step | Mechanism | Board ΔT (single run) | Distinguishable? |
|---|---|---:|---|
| 6 | parallel queries | −76.5% | yes |
| 8 | mmap + parallel CSR | −29.7% | yes |
| 9 | BFS renumbering | −44.3% | yes |
| 10 | u32 distances | −7.9% (M −30%) | T no (PERF says so); M yes |
| 11 | parallel region renumbering | −19.4% | yes (assumes CV holds at 0.47 s: **U**) |
| 12 | bidirectional + paged distances | −34.6% | yes |
| 13 | bucketed single-pass build | −22.8% | yes (~4.8σ) |
| 14 | packed u32 edges | T +1.5%, M −6.5% | T noise; M yes |
| 15 | THP disable | T +4.5%, M −23.2% | T noise; M yes |
| 16 | u64 heap key | −12.5% | borderline (~2.7σ); real direction, magnitude unknown |
| 17 | prefetch | −7.2% | no (~1.5σ) |
| 18 | SWAR parse | +2.1% | no |
| 19 | memory diet + mallopt | T −5.2%, M −21.6% | T noise; M yes |
| 20 | code size 875→722 | T +8.7%, then −4.0% on re-sample | T noise; S certain |
| 21 | `enumerate()` restored in `Graph::row` | +1.4% | noise (no VPS A/B recorded for this alone: **M missing**) |

- **M** Collectively, steps 16–21 are real: board 0.2534 (step 15, one sample) → 0.2069 (21-sample mean), −18%.
  Individual attributions within that chain are not resolved on the board. They rest on the VPS phase timers.

### 2b. Rejected / inconclusive: mechanism vs noise (Q1)
| Step | Variant | Scope / env | Result | Spread | Class |
|---|---|---|---|---|---|
| 3 | build profile alone | VPS, seed 1, CPU 2 | −5.1% | 28% | **Inconclusive, noise** (kept anyway, joint with CSR) |
| 4 | checked byte parser | board, ~8 s regime | +19.4% vs prior run; same-source rerun 7.80 s | — | Board regression on one sample. Superseded by steps 8/18 |
| 7 | serial two-pass CSR | VPS, seed 1, 1/6/12 w + phase timers | +4.3/+10.6/+18.0%, repeat +12.1%; scatter 0.73→1.33 s | 7–10% | **Mechanism** (two batches, phase timer explains it) |
| 9 | DFS order | VPS | −17.2% vs BFS −21.8% | 5–7% | Mechanism, single batch; BFS chosen |
| 10/11/12 | per-worker u64 / growing region buffers / dense bidir arrays | VPS cgroup | M +28% / +20% / +97% | memory stable | **Mechanism (memory)**; replaced by kept variants |
| 12 | grow the side with the smaller key | text only | "floods the graph" when one side's first edge is ≈2^32 | — | **Missing evidence** (no numbers saved) |
| 19 | search-state trimming / free bucket parts / claim prefetch / no mallopt | VPS 12 w | +11% (search +30 ms) / +6–8.5% with no peak gain / renumber +9 ms / 98 MB peak | 7–20% | trimming: mechanism-ish; freepart, claimpf: noise; no-mallopt: mechanism (memory) |
| 21 | Dial ring 4096 / 8192 / pooled+bitmap | VPS 6 and 12 w, seed 1 | 12 w: +11% / +13% / +1.3%; 6 w: −1.1% / +7.7% / +1.3%; search medians higher in every arm; 12 w peaks 86.8 / 89.7 / 84.8 vs 78.7 MB | 8.6–30.7% | **Mechanism on memory** (buckets keep stale entries until the query ends). Time loss is mechanism-ish for the 8k ring, noise for the pooled ring |
| 22 | query reordering | VPS per-query timing, 8 runs | makespan within 1–4% of ideal | — | **Mechanism ceiling**: ≤1–4% available |
| 22 | edgescan (count in-degrees from packed edges) | VPS | +1.0 / +2.9%, count −2 ms | U (not saved in README) | **Noise** |
| 22 | recycle (atomic count in copy + buffer reuse) | VPS | +2.1 / +7.6%, peak 85.0 MB (+8.2%), renumber +31 ms | U | **Mechanism**: atomic contention plus a longer old-graph lifetime |
| 23 | blobs 512 | VPS 12 w + board ×2 | VPS search 258.5 vs 259.5 ms; board 0.1983 / 0.2225 (mean 0.2104 vs 0.2069 = +1.7%, ~0.7σ) | 26% / board CV 3.3% | **Noise / no effect** |
| 23 | blobs 128 / 64 | VPS 12 w | +7.5 / +15.7%; search +14 / +19%; peak 91.5 / 104.5 MB | 14–16% | **Mechanism** (search phase and memory both worse) |
| 24 | PGO (trained without seed 1) | VPS 1/3/6/12 threads | 0.0 / +0.1 / +2.8 / −3.5% | 6.5–36% | **Noise / null**. The 1-thread arm (lowest noise) shows 0.0% |
| 25 | μ-prune of pushes | VPS 6/12 w + exact counters | pushes −5.7%, pops +0.3%; T +0.6 / −0.2%; 3/8 and 5/8 rounds faster | 9.9–34.9% | **Mechanism ceiling** (counters bound the gain at ≲ the push share). Time effect is noise |
| 26 | no renumbering (control) | VPS 1/6/12 w | +130 / +124 / +133%, 0/8 rounds faster; peak 229.5 MB at 12 w | 8 rounds | **Mechanism**: locality dominates |

Untracked Track-A dirs and A's plateau experiments (hour cache, native CPU) are not Track D evidence.

## 3. VPS → board transfer (Q2)

| Mechanism | VPS whole-run | Board single-run | Board effect resolvable? |
|---|---|---|---|
| SWAR (18) | −15.0% (6 w), −10.6% (12 w); load-phase −23 to −29% | +2.1% | No. Even a true −5% board effect would be invisible |
| heap key (16) | −4.9 / −4.2% (spreads 8–19%) | −12.5% | Borderline (~2.7σ). The step-15 base is a single, possibly high draw |
| memory diet (19) | T +3.6%, M −22% | T −5.2%, M −21.6% | T no; M yes, and M transferred exactly |
| THP disable (15) | T, M unchanged (VPS THP=madvise) | M −23.2%, T +4.5% | M yes; T no |
| code size (20) | −5.9 / −2.8% | +8.7%, then −4.0% re-sample | No (two samples of one binary differ by 4%) |
| prefetch (17) | −10.9 / −12.5% | −7.2% | No (~1.5σ) |

- **I** Of the five transfer "contradictions" in PERF.md, only memory effects and heap key (borderline) are
  distinguishable from board noise. The SWAR "no transfer" and diet "opposite sign" findings are both
  consistent with noise. They are not evidence of misweighting.
- **F/M** One systematic misweighting is proven: memory before step 15. The board backed anonymous memory with
  2 MB THP; the VPS uses THP only on madvise. Since step 15, the board peak (79.2–79.9 MB) matches the VPS
  (78.7–79.0 MB), so memory now transfers.
- **I/U** Possible time misweighting, unproven: (a) the VPS has 6 vCPUs and no SMT, so its 6-worker and
  12-worker arms run the same CPU capacity, while the board has 12 hardware threads. 6 vs 12 workers has never
  been measured on the board (README's own "24 hours" item). (b) `lscpu` on the VPS reports L2 24 MiB and L3
  96 MiB (KVM-synthesized, `mu-prune/cpu.txt`). Cache-sensitive mechanisms (blobs, renumbering granularity) may
  rank differently on the board. (c) The board is 2.3× faster whole-run (0.207 vs 0.47 s), but there is **no
  board phase split** (**U**). The weak hint from heap key (board gain > VPS) and SWAR (board gain < VPS) is
  that search may be a larger share on the board than load. That hint is within noise.
- **I** Final scoring averages the middle 3 of 5 runs (sd ≈ 1.6%), so a real 3–5% gain counts there even though
  one board run cannot show it. The board can confirm only ≳10% changes, or smaller ones with ≥4–5 samples per arm.

## 4. Shared assumptions of steps 20–26 (Q3)

- **F** Every experiment from step 20 on kept the same pipeline, with **7 scoped fork/join barriers plus serial
  sections** (`track-d/src/main.rs`): parse chunks (`parse_chunks`) → place parts (`build_graph`) → BFS claim
  regions → parallel row copy (`reorder_graph`) → transpose count → serial prefix sum and atomic allocation →
  transpose scatter (`transpose`) → search (`answer_queries`). There is also a serial query parse, a serial
  prefix sum over 1.74M offsets in transpose, a serial `heavy` sort, and `(0..n).map(AtomicU32::new)`
  zero-fills. (research/current-state.md says "four strict barriers"; the code has seven.)
- Shared assumptions, none of which has been questioned since step 13 (**I**):
  1. Exact plain bidirectional Dijkstra over the **whole** renumbered graph and a **full** reverse CSR. No
     goal direction (ALT/landmarks), no index (CH/arc flags). The README states "No routing index" as a design
     choice. **U** whether the leaders use one.
  2. Phase order is fixed and nothing overlaps: transpose runs only after the copy, and search starts only after
     both.
  3. 12 workers by default and THP disabled process-wide, both set once and never re-tested on the board after
     the code changed. Step 15's T +4.5% (possible 4 KB-page TLB cost on the random-access graph) was never
     resolved.
  4. The VPS whole-run trimmed mean is the decision metric, although its resolution (≥5–8%) exceeds every
     effect tested in steps 19–25 except the losses (Dial, blobs 128/64, recycle). The exceptions are earlier
     steps: prefetch (−11/−12%) and SWAR (−15/−11%) did exceed it.
  5. Micro-mechanisms come from competitors' PERF claims (Dial, blobs, PGO). Two of three failed here
     because the competitors' baselines were weaker (`dial-queue/README.md`).
- **F** Rusage (mu-prune, VPS 12 w): user 2.18 s, sys 0.24 s, wall 0.477 s. About 16% of 6-vCPU capacity is
  idle (serial or imbalanced phases), and sys plus page faults are ~10% of CPU. **I** That bounds what
  barrier removal alone can buy on the VPS at roughly ≤10–15%.
- **M** Total search work at 1 thread: 7.57M pops, 8.66M pushes per 2,000 queries (~3,800 settles/query). The
  top 100 queries are half of search work (search-tail). The leaders' ~0.10–0.12 s is ≈2× our mean 0.207 s.
  **I** A 2× gap is not consistent with any single micro-mechanism tried. It requires either a far cheaper
  preprocessing path or less search work per query.

### Untested or abandoned ideas (each with the changed assumption that would justify it)
| Idea | Status | Changed assumption / why now |
|---|---|---|
| Board A/B of 6 vs 12 workers (or 6+SMT pinning) | never measured on the board | The VPS cannot test SMT. A cheap 1-line change, but it needs ≥4 board samples per arm |
| Selective huge pages for **dense** arrays (CSR, reverse CSR, offsets, claims) with sparse pages kept at 4 KB | never tried | Step 15 disabled THP globally to fix memory charged by *sparse* pages. Dense arrays are fully touched, so 2 MB pages cost ≤ ~2 MB each. **U**: `PR_SET_THP_DISABLE` also blocks `MADV_HUGEPAGE` on older kernels, so this needs a per-region `MADV_NOHUGEPAGE` design instead. The VPS (THP=madvise) can test the time effect |
| Atomics-free transpose (bucket by target block, as step 13 did for sources) or reverse built inside the renumber copy without atomics | untried. "Recycle" used atomics and failed on contention | Transpose is 51–62 ms (~11% VPS). The rejected variant's failure cause was atomics and buffer lifetime, not the fusion idea |
| Parallel first-touch of reverse storage (plateau #3) | untried | First touch is 20 ms per 28 MB single-thread vs 6 ms on 6 threads (`transpose/page-fault-bench.rs`). Estimated ≤3%: noise-level |
| Radix heap or 4-ary heap (plateau #6) | untried. Dial (a different structure) failed | Distinct from Dial: it holds no stale buckets. Low prior: heap ops are ~16M per run and the run is miss-bound (I) |
| Cheaper locality order (e.g., BFS within load-time source parts, skipping the global claim pass) | untried | Step 26 shows the order matters (2.3×); renumbering costs ≈0.11 s of 0.47 s VPS |
| Overlapping phases / persistent thread pool | untried | 16% idle capacity (rusage). Ceiling ≲10–15% (I) |
| Goal-directed search (ALT) or a light index | never evaluated | Must pay back inside one run: each landmark costs a full SSSP (~1.74M settles ≈ 23% of all query work). Prior is negative (I) unless the leaders show it |
| Board phase profile (instrumented diagnostic push, if board logs are visible) | never done | Removes the largest **U** (board phase split). **U** whether board logs expose stderr (step 14 cites other teams' "public board logs") |
| Revisiting rejected Dial / PGO / blob-512 / μ-prune / two-pass | — | No changed assumption found. Do not repeat |

## 5. Correctness record (Q4)

- **F** No recorded wrong answer in any D experiment or board run. All 46 D board runs `ok=true` (kush.json).
  Every evidence README reports Python byte-equality on full seed 1 (SHA `a23dbdf0…277e`) and, since step 9,
  on the ten-dataset matrix.
- Near-misses and risks, from the files:
  1. **F** Step 10/12: the u32 narrow pass is exact only below `u32::MAX`. 4/2,000 seed-1 answers exceed u32 and
     must take the shared u64 rerun. Any new queue or key must preserve the overflow flag (`PERF.md` §10, §12).
  2. **F** Mutants caught: stop at the first meeting (3 tests), always accept the narrow answer (4), no page clear
     on reset (6), Dial far labels in the ring (4), μ-prune `2*bound` (unit), swapped heap-key halves (2).
     The suite discriminates the known failure modes.
  3. **F** Step 12: choosing the side by smaller key floods the graph. This is a performance pathology near 2^32
     weights. Correctness held, but no measurements were saved.
  4. **F** Leader doc warns that `(distance,node)` lexicographic radix keys break with zero weights. This matters
     for any radix-heap experiment.
  5. **F** Step 20: the VPS-timed build (`measured-slim-main.rs`) differs from the scored file (offset
     construction). This is a provenance gap, not a correctness gap.
  6. **F** Test rot: `diet-tests.rs` no longer compiles against current `main.rs` (mu-prune README). The newer
     all-pairs u64 reference test (300 random graphs) is the live safety net.
  7. **F** `reorder_graph` skips renumbering when `nodes ≥ 2^27` (provisional-ID bits). This is correct but
     ~2.3× slower on such inputs. The hidden seed has the documented size, so it is not relevant (I).

## 6. Contradictions affecting the next experiment

1. **F** `track-d/README.md` is stale on 5 points. It says buckets hold `(u,v,w)` triples (they are 8-byte records
   since step 19). It says region buffers hold edges with provisional heads (step 19: order-then-copy). It says
   "the mapping is released after loading" (step 20 keeps it). It quotes D7-era timings. Its "24 hours" section
   predates the memory work. Judges read it.
2. **F** PERF.md's best label (65.16) is below the board best (65.39, `run-8040ef5bc533`, absent from PERF).
3. **F** Step 21 says "same line count", but board S went 722 → 723 (kush.json).
4. **F** The plateau doc (`docs/optimization-plateau-2026-11-01.md`) forecast D-PGO at −5% central. The
   measurement was null. Its D #3 (first touch) and #6 (radix) remain untested estimates, not evidence.
5. **I** Step 18's note ("parser kept for now ... candidate for removal") was never resolved. The SWAR path
   costs S (~38 lines between steps 17 and 18). Its board time effect is unresolved; its VPS load gain is real.
   Removing it trades a measured VPS load gain for about +0.15 points of S (7.5·log10(821/783)). That needs a board multi-sample
   check, not one run.
