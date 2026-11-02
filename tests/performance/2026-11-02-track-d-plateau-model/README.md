# Track D: proxy calibration and phase diff against competitor binaries (measurement, 2026-11-02)

Experiment 0 of [research/plateau-2026-11-02.md](../../../research/plateau-2026-11-02.md). Measurement only; the
scored code is unchanged (`track-d/src/main.rs` sha256 `459900905ca4…`, last changed in `740ed03`, functionally `21df4fd`).

## Sources
Public competitor solutions, all MIT-licensed (each repo's `LICENSE`, copyright Hackathon Raptors CIC template).
They were built and run here for benchmarking only; none of their code is in the scored solution.

| Team (board best) | Repo @ commit | Build (each team's own Dockerfile flags) |
|---|---|---|
| HighVelocity (0.1103 s) | `insightlabs38-pixel/sd-sm` @ `7753f18` | `gcc:15`, `g++ -O3 -std=c++17 -pthread -flto -static -s` (no `-march=native`) |
| Team FlyHigh (0.1507 s) | `Harini-vm/speed-demon-harini` @ `6feb4ff` | rust 1-slim-trixie, `target-cpu=native`, fat LTO |
| Cache Me (0.1677 s) | `manusingh090/speed-demon-2026-entry` @ `3ca2a3e` | rust 1-slim-trixie, `target-cpu=native`, LTO |
| ours (best 0.1986 s, mean of 21 runs 0.2069 s) | `track-d/` at HEAD | as submitted |

Measurement shims, which do not affect the algorithms:
- FlyHigh and Cache Me read the thread count only from `available_parallelism` (6 under CPU set 0–5). One line
  adds an `SD_THREADS` override so they can run at 12; HighVelocity already reads `SD_THREADS`.
- Diagnostic builds (`phases/`, `nb/`, `np/`, `cnt/`) add stderr phase marks (wall time and process CPU from
  `getrusage`), counters, or ablations. The timed calibration binaries (`calibration/`) have only the thread shim.
- Every timed run was checked against the Python full-seed-1 output (`a23dbdf0…`). In `calibration/`, all four
  binaries matched on all ten validation datasets (`correctness.json`).

The competitors' source trees are not copied into this repository. Fetch them at the commits above; the
diagnostic changes are fully described here: a one-line `SD_THREADS` override, stderr phase marks
(`getrusage`) after each phase, `if (false && …)` on the bypass test and on the `LOOK` prefetch, and atomic
counters in `drain`.

Environment: `my-vps`, 6 Haswell vCPUs (KVM), Docker CPU set 0-5, 8 GB cgroup, read-only container without network
and with all capabilities dropped, prewarmed input, `measure.rs` wrapper (wall time, cgroup `memory.peak`, rusage).
Order rotates each round; round 0 is discarded.

## 1. Calibration (`calibration/summary.json`, 10 rounds per worker count)
| | ours | Cache Me | FlyHigh | HighVelocity |
|---|---:|---:|---:|---:|
| VPS 6 workers, trimmed mean (s) | 0.504 | 0.412 | 0.413 | 0.290 |
| VPS 6 workers, ratio to ours | 1 | **0.82** | **0.82** | **0.58** |
| VPS 12 workers, ratio to ours | 1 | 0.85 | 0.87 | 0.63 |
| Board best, ratio to ours | 1 | **0.84** | **0.76** | **0.56** |
| user CPU (s, 6 w) | 2.21 | 1.82 | 1.80 | 1.28 |
| sys CPU (s, 6 w) | 0.25 | 0.18 | 0.24 | 0.17 |
| minor faults (6 w) | 47.3k | 34.9k | 53.6k | 39.2k |
| cgroup peak (MB, 6 w) | 78.5 | 64.7 | 81.8 | 64.0 |

- **The VPS reproduces the board's ratios for large code differences.** It matched Cache Me and HighVelocity
  within 2–4 points at 6 workers. The plateau is not caused by a blind proxy. The earlier VPS-to-board misses
  (SWAR, THP) were effects below board noise.
- FlyHigh gains about 7 points more on the board than on the VPS (0.76 against 0.82). That is a weak hint of a
  box-specific benefit from its THP and single-structure layout. It is one best run, so it is not conclusive.
- **The gap is user CPU work, not OS overhead.** HighVelocity's wall ratio (0.58) equals its CPU ratio
  (1.45 / 2.46 = 0.59). The sys-time difference from ours is 0.08 CPU-s, less than 9% of the CPU gap. FlyHigh
  has more faults and sys time than we do and is still faster. Approach C (faults, spawns) is rejected as a
  primary mechanism.

## 2. Phase split, ours vs HighVelocity (`phases/phases-summary.json`, 8 rounds, 6 workers; wall / CPU ms)
| Our phase (HV equivalent) | ours | HV | HV, bypass off (`nb/`) | gap ours − HV with bypass off (wall) |
|---|---|---|---|---:|
| parse + place (parse + rows) | 105 / 458 | 83 / 398 | 85 / 380 | +20 |
| renumber (bypass + BFS + gather) | 137 / 624 | 95 / 440 | 105 / 496 | +32 |
| reverse graph | 62 / 233 | 28 / 132 | 36 / 173 | +26 |
| search | 196 / 1113 | 94 / 527 | 120 / 690 | **+76** |
| total (s) | 0.522 | 0.307 | 0.349 | |

12 workers gives the same picture (ours 0.508 s, HV 0.316 s). HighVelocity's bypass (out-degree ≤ 2, not a query
endpoint; classified during row placement from out-degree alone) is worth −10% of its total time (0.349 → 0.315 s,
same run). Its search drops 120 → 99 ms.

## 3. Search engine dissection
- **Same work** (`cnt/count.txt`, HV with bypass off, full seed 1): 7,523,556 expansions, 8,189,955 bucket
  entries (stale included), 22,158,390 relaxations, 9,382,499 improving pushes. Ours (step 25 counters):
  7,573,787 settles and 8,655,173 pushes. HV's balancing, stop rule and forward probe do **not** reduce work.
- **Cheaper per expansion:** search CPU per expansion is 147 ns for ours, 84–92 ns for HV (two sessions) and 97 ns for HV
  without its 4-ahead prefetch: 1.5–1.75× at equal work, compared across sessions.
- **Prefetch ablation** (`np/phases-np-summary.json`, HV with bypass off, 8 rounds): with the `LOOK = 4` prefetch
  of row and label disabled, search goes 109 / 631 → 126 / 730 ms (+16% CPU). Memory-level parallelism is part of
  the engine's advantage, ≈20% of it ((730 − 631) / (1113 − 631)). The remaining 1.5× (730 vs 1113 CPU-ms at equal work) is not yet
  attributed. The candidates, all present in HV and absent in ours, are:
  - the 2048 × 128-wide bucket ring with FIFO drain instead of a `BinaryHeap<u64>`;
  - 4-slot inline rows (16 B per vertex, one line per row, no offsets array);
  - branch-free relaxation;
  - u64 labels in 128-vertex pages.
- Run-to-run noise: HV with bypass off measured 120 / 690 and 109 / 631 ms search in two sessions (≈9%), so
  differences under about 10% between sessions are not meaningful. Comparisons within one session alternate order.
