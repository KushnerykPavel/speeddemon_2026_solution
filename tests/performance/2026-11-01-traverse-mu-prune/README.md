# Track D: prune pushes by the best path found (rejected) + rusage prelude

Candidate `prune-main.rs` (sha256 `f4e0df0f15b3…`) vs baseline `before-main.rs` (= `track-d/src/main.rs` at
`539b15f`, sha256 `459900905ca4…`, functionally `21df4fd`). One change: in `Side::settle_next`, a label is not
pushed when `candidate + other.top() >= best` (the meeting update runs first; `best` passed from `Search::run`).

Environment: `my-vps`, 6 Haswell vCPUs, CPU set 0-5, pinned Rust image, offline `target-cpu=native` release build,
full official seed 1 (output SHA `a23dbdf0…277e`), prewarmed input, 8 GB cgroup, `measure.rs` wrapper
(wall incl. start/exit, cgroup v2 `memory.peak`, now also rusage user/sys/minor faults). Timing: nine alternating-order
rounds per worker count, round 0 discarded, trimmed mean of the remaining eight.

## Correctness
- All-pairs vs independent u64 Dijkstra, 300 random graphs, weights 0/tiny/1e3/~2^31/~2^32, 1 and 3 workers:
  passes for candidate and baseline; mutant (`2 * bound`) fails (`unit-*.txt`).
- 204 Python-reference cases x thread counts, 8 invalid-input, 3 invalid-thread checks: both binaries pass.
- Candidate output byte-equals baseline on all ten validation datasets (`equality-and-unit.log`).
  (The repo's older `diet-tests.rs` no longer compiles against current `main.rs`; not used.)

## Deterministic counters (1 thread, full seed 1, exact)
| | pushes | pops |
|---|---:|---:|
| baseline | 8,655,173 | 7,573,787 |
| prune | 8,158,649 (-5.7%) | 7,594,477 (+0.3%) |

Pruning removes only 5.7% of pushes and no settles: most pushes happen before the first meeting, and the search
is dominated by settling ~3,800 vertices/query, which the stop rule already bounds.

## Timing (trimmed mean, `summary.json`, `runs.jsonl`)
| Workers | baseline (s) | prune (s) | change | paired median | prune faster | spreads b/p |
|---|---:|---:|---:|---:|---:|---|
| 12 | 0.4772 | 0.4800 | +0.6% | +1.3% | 3/8 | 24.8% / 9.9% |
| 6 | 0.4915 | 0.4903 | -0.2% | -1.6% | 5/8 | 9.9% / 34.9% |
Cgroup peak 78.9 / 78.8 MB (12), 78.0 / 78.4 MB (6): unchanged. Source +9 lines.

**Decision: rejected** (rejection rule: search gain < 2%; push reduction < ~6%; time inside spread). Not boarded.

## Prelude finding (rusage, baseline, 12 workers)
user 2.18 s, sys 0.24 s (~10% of CPU), 48k minor faults (~190 MB touched), wall 0.477 s on 6 vCPUs, so about 16%
of the 6-vCPU capacity is idle (serial or imbalanced phases) and sys+fault time is below the 20% threshold that
would promote allocation/page-fault mechanisms. Remaining ranking stands: transpose serial parts, renumber-off
control, fused distance pages.
