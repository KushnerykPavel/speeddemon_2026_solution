# Track D: current state (2026-11-01, 15:20 UTC)

Code freeze: 2026-11-02 15:00 UTC. Tags: **F** fact from a file or the official feed, **M** measured
(environment named), **H** hypothesis, **U** unknown.

## 1. What the task scores

- **F** K = (Tref/T)^0.50 · min(Mref/M, 50)^0.20 · min(Sref/S, 1)^0.15 · (Dref/D)^0.15, and points =
  50·log10 K ([scoring.py](https://github.com/hackathon-raptors/speed-demon-2026/blob/main/harness/scoring.py)).
- Sensitivities (arithmetic, not measured): −10% T = +1.14 points, −10% M = +0.46, −10% S = +0.34.
  Halving T = +7.5 points.
- **F** Live board: one run per push. Final scoring: a hidden seed, 5 runs, mean of the middle 3.
- **F** `overall()` is the geometric mean of a team's best **three** track totals and returns `None` with
  fewer than three tracks. The feed shows our Tracks B and C as unchanged starters ("not run"). This is
  outside Track D, but it may matter more to the team score than any Track D change. Ask the user whether
  the target is Track D points or the overall score.
- **F** Track total = 0.65 · machine points + 0.35 · judge score. Judges check PERF.md run IDs.

## 2. Baseline identity

- **F** `track-d/src/main.rs`, `Cargo.toml`, `Cargo.lock` and `Dockerfile` are byte-identical from
  `21df4fd` (step 21) to HEAD `539b15f`. The BFS blobs from `cae88c9` were reverted in `740ed03`.
- **F** Uncommitted: `track-d/PERF.md` adds steps 25 (μ-prune, rejected) and 26 (no-reorder control,
  rejected). Their evidence dirs are untracked. They must be preserved.
- **F** Best D board run: `run-8040ef5bc533`, commit `14001f9` (a Track A commit; D code is the `21df4fd`
  code). T 0.198594 s, M 79.65 MB, S 723, D 1, K 20.316, **65.39 points**, rank 32 of 104 (feed
  2026-11-01 15:04:53 UTC). The latest D run (`run-3ce388bd55f8`, `807ceec`) scored 65.38.
- **F** The prompt's "77.26" is **not** a Track D number. In the saved feeds it appears only as another
  team's Track A points. Our Track A best is 79.03. The Track D plateau is 65.0–65.4.

## 3. Board noise (measured from the official feed)

The board re-ran the unchanged D binary 21 times between 2026-10-31 21:44 and 2026-11-01 14:03 UTC,
because pushes for Track A trigger D runs as well.

| Metric | Mean | SD | Min | Max |
|---|---:|---:|---:|---:|
| T (s) | 0.2069 | 0.0069 (CV 3.3%) | 0.1986 | 0.2241 |
| M (MB) | 79.62 | 0.21 | | |
| Points | 64.95 | 0.35 | | |

- **M** A single board run cannot resolve a time change below about 7–10%. A bootstrap of the final protocol
  (5 runs, middle 3) from these samples gives SD ≈ 1.6%. A real 4% gain is invisible on the live board
  but counts in the final score.
- **F** Our "best" 65.39 is the luckiest of 21 samples. The expected final score of the current binary is
  about 64.9–65.0, before the effect of a different seed.

## 4. Gap to the leaders (feed 2026-11-01 15:04 UTC)

| Team | T (s) | M (MB) | S | Points |
|---|---:|---:|---:|---:|
| Lex | 0.0987 | 62.1 | 828 | 73.62 |
| Cause Of Death | 0.1184 | 65.7 | 424 | 73.58 |
| SweetDreams | 0.1088 | 65.3 | 574 | 73.54 |
| 10th (AVENGERS) | 0.1210 | 68.3 | 618 | 71.95 |
| **Kushneryk** | **0.1986** | **79.65** | **723** | **65.39** |

Decomposing our 6–8 point gap to the top 10 (arithmetic): T 0.199 → 0.12 is +5.5 points, M 80 → 65 is
+0.9, S 723 → 550 is +0.9. **This is a 2× time problem.** Micro-optimizations of 3–5% (steps 20–26)
have not changed the board result, and they cannot close this gap.

## 5. Current implementation (`track-d/src/main.rs`, 841 lines, S = 723)

Pipeline of five phases with seven fork/join barriers (parse, place, BFS claim, row copy, transpose
count, transpose scatter, search), plus serial prefix sums, the heavy-list sort and zero-fills between
them (corrected after `historian.md`):

1. `map_file` maps `graph.txt` (92 MB). `line_chunks` splits it into ≤ 12 line-aligned chunks.
2. `build_graph` uses a SWAR parse (`fast_record`) and buckets each edge by its source block into
   8-byte records. Then each part counts and places its edges into the CSR. Edges are packed `u32`s:
   the head in the low ⌈log2 n⌉ = 21 bits, the weight above it; escaped weights go to a sorted `heavy`
   list.
3. `reorder_graph` claims vertices with a CAS while each worker grows BFS regions in its own ID slice.
   The regions, concatenated, give the new order. A parallel copy writes the new CSR, translating
   heads through the claims.
4. `transpose` counts in-degrees with atomics, takes a serial prefix sum, then scatters edges through
   atomic cursors.
5. `answer_queries`: 12 workers take queries from an atomic counter. Each query runs bidirectional
   Dijkstra with a `BinaryHeap<Reverse<u64>>` (key = distance << 32 | node). Distances are `u32`, stored
   in 1,024-vertex pages assigned on first touch. Each step grows the side with the smaller queue; the
   search stops when `top_f + top_b ≥ μ`. If a sum overflowed and no answer below `u32::MAX` was found,
   the query reruns in a shared `u64` search under a mutex. The 4 seed-1 answers above 2^32 rerun there.
6. Degree rejection, the `s == t` shortcut, and output through `BufWriter`.

Build: fat LTO, `codegen-units=1`, `panic=abort`. The Dockerfile sets `target-cpu=native`. Zero
dependencies. `prctl(PR_SET_THP_DISABLE)` and `mallopt(M_MMAP_THRESHOLD, 256 KB)` are set.

## 6. Measured performance model (VPS, 6-vCPU Haswell KVM; not the board)

| Phase | Time (s) | Source |
|---|---:|---|
| Load: parse + bucket + place | ≈ 0.09–0.10 | PERF step 19 phase medians (12 workers) |
| Renumber (BFS + copy) | ≈ 0.11 | step 19 |
| Transpose | 0.051–0.062 | step 22 (count ≈ 15 ms, prefix and allocation ≈ 16 ms, scatter ≈ 25 ms) |
| Search (excluding transpose) | ≈ 0.20–0.21 | step 19 search 0.268 includes transpose |
| Whole process | ≈ 0.47 (12 w), 0.49 (6 w) | steps 25–26 |

- **M** Search: 7.57M pops and 8.66M pushes per full seed-1 run (step 25 counters, one thread). The 100
  longest queries account for half of the search work. The makespan is within 1–4% of perfect balance
  (step 22).
- **M** Disabling renumbering makes the run 2.3× slower (step 26). Locality is the dominant search factor.
- **M** cgroup peak 79.0 MB at the end of load placement; the renumbering copy reaches 74.7 MB (step 19).
- **U** The board's phase split. The board is about 2.3× faster than the VPS overall (0.207 s vs 0.47 s),
  but no phase-level board timing exists. It is not known whether the board ratio of search to
  preprocessing matches the VPS.
- **U** Hardware counters (cache misses, IPC). The evidence so far has wall-clock phase timers and
  `cpu-clock` samples only.

## 7. Experiment history (summary; full audit in `historian.md`)

Accepted board gains (step → board time): mmap and parallel CSR (1.63 → 1.14 s), BFS renumbering
(→ 0.64), `u32` distances (→ 0.59, memory), parallel region renumbering (→ 0.47), bidirectional search
(→ 0.31), bucketed single pass (→ 0.24), packed edges and THP disable (memory 142 → 102 MB), `u64` heap
key (→ 0.22), prefetch (→ 0.206), memory diet (102 → 80 MB), code-size pass (S 875 → 722).

Rejected or with no effect: serial two-pass CSR (step 7), Dial queues ×3 (step 21), transpose counting
and buffer reuse (step 22), query ordering (step 22, already balanced), BFS blobs 512/128/64 (step 23,
board samples 0.198 and 0.223 s), PGO (step 24), μ-pruning of pushes (step 25), no-reorder control
(step 26). The SWAR parser's VPS gain (−15%) did not show on the board.

Since step 19 (board 0.195 s) no change has moved the board time outside its noise band.

## 8. What is unknown and needed before choosing

1. **U** The board-proportional phase split: does search or preprocessing dominate on 6 P-cores/12
   threads?
2. **U** Whether a structural change (a different pipeline or algorithm) explains the leaders' 0.10 s.
   Competitor code is the strongest lead (researcher report).
3. **H** The four barriers plus the separate transpose waste a large part of the parallel capacity
   (serial prefix sums, the heavy-list sort, thread spawns, first-touch faults). Not measured.
