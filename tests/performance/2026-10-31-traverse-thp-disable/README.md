# Track D: disable transparent huge pages for the process

Baseline: `1767ec0` (`before-main.rs`). Candidate: `thp-main.rs` (scored
source): `main` first calls `prctl(PR_SET_THP_DISABLE, 1)` on Linux, so the
process gets 4 KB pages. Failure is ignored (it only costs memory).

## Why

The step-14 board run saved 6.5% memory where the VPS saved 25.6%. Our design
reserves large buffers and touches them sparsely (renumbering regions, paged
search distances), which is cheap only with 4 KB pages. Public PERF.md logs of
other teams report that the scoring box backs anonymous memory with 2 MB
transparent huge pages (Cache Me: 102 → 64 MiB with packed edges and THP off;
FlyHigh: −12 MB on Track A, no change but slower on Track D).

## What the VPS can and cannot show

VPS THP setting: `always [madvise] never`, so plain allocations already use 4 KB
pages and the memory effect is invisible here. This run only checks that the
Linux build works, outputs stay exact and time does not move. Same protocol as
earlier steps (CPU set 0-5, one warmup + five rotating runs, trimmed mean of the
middle three, cgroup v2 `memory.peak`); all 24 outputs byte-equal to Python.

| Variant-workers | T (s) | Spread | cgroup peak (MB) |
|---|---:|---:|---:|
| `1767ec0`-6 | 0.5566 | 16.4% | 96.5 |
| `1767ec0`-12 | 0.5720 | 20.5% | 101.4 |
| THP off-6 | 0.5474 | 15.6% | 96.6 |
| THP off-12 | 0.5632 | 10.1% | 101.4 |

The board run is the measurement for this step. Raw: `runs.json`, `summary.json`.
