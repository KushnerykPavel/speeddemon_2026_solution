# Track D: huge pages for the dense final graph arrays (E1, VPS: accepted, small)

Hypothesis (research/plateau-recovery.md, H2/E1): the forward and reverse CSR are searched on 4 KB pages since
step 15 disabled THP for the whole process. Backing only the fully written final arrays with 2 MB pages cuts TLB
misses and page faults with no resident slack, while sparse buffers stay on 4 KB pages. Expected: T −3 to −8%,
M unchanged.

Baseline `before-main.rs` = `track-d/src/main.rs` at `539b15f` (sha256 `459900905ca4…`, functionally `21df4fd`).
Candidate `thp-main.rs` (sha256 `85d90829139b…`, +32 lines, `thp.patch`, branch `exp/d-thp-dense`, uncommitted):
- `allow_huge_pages(true)` (`prctl(PR_SET_THP_DISABLE, 0)`) after the renumbering BFS, `false` again after the
  transpose, before the search workers allocate their sparse distance pages;
- `madvise(MADV_HUGEPAGE)` on `new_offsets`, `new_edges` (renumber copy) and the transpose `offsets`, `edges`.

Environment: `my-vps`, 6 Haswell vCPUs (KVM), kernel 7.0.0-28, THP `enabled=madvise`, `defrag=madvise`
(`vps/thp-state.json`), rustc 1.99.1 pinned image, offline `target-cpu=native` release builds, Docker CPU set 0-5,
8 GB cgroup, `measure.rs` wrapper (wall, cgroup v2 `memory.peak`, rusage), full seed 1 prewarmed. Eleven
alternating rounds per worker count, round 0 discarded, trimmed mean of ten. `experiment.py` is the exact driver.

## Correctness
- All-pairs vs an independent `u64` Dijkstra, 300 random graphs (weights 0 … ~2^32): pass for both
  (`vps/unit-*.txt`).
- 204 Python-reference cases × 6 thread settings, 8 invalid inputs, 3 invalid thread options: pass for both.
- Byte-equal outputs on all ten validation datasets (full/sample/tiny seeds, edge cases, no-edges, no-queries);
  every timed run checked against the Python full-seed-1 output (`a23dbdf0…`). Local Mac: full seeds 1 and 2 equal.

## Mechanism check (diagnostic builds, 12 workers, five rounds after warmup)
| | AnonHugePages after transpose | renumber (ms) | transpose (ms) | search (ms) | total (ms) |
|---|---:|---:|---:|---:|---:|
| before | 0 | 113.9 | 63.6 | 194.5 | 468.3 |
| thp | 43–45 MB | 110.5 | 55.4 | 198.4 | 450.1 |

Huge pages are present. On this KVM guest the search phase does not change; the gain is in the transpose and copy
(fewer first-touch faults). TLB behaviour on the board's i5-13500 cannot be tested here.

## Timing (`vps/summary.json`, `vps/runs.jsonl`)
| Workers | before (s) | thp (s) | trimmed mean | paired median | thp faster | spreads b/t | peak MB b/t | minor faults b/t | sys s b/t |
|---|---:|---:|---:|---:|---:|---|---|---|---|
| 12 | 0.4828 | 0.4600 | −4.7% | −3.7% | 9/10 | 11% / 13% | 79.00 / 78.96 | 48.0k / 36.8k | 0.243 / 0.226 |
| 6 | 0.5014 | 0.4659 | −7.1% | −4.7% | 8/10 | 20% / 17% | 78.16 / 78.27 | 47.3k / 36.1k | 0.245 / 0.209 |

## Decision
The pre-registered criterion (search phase faster in ≥ 7/10 pairs) was **not met**: the search phase was unchanged
(194.5 vs 198.4 ms). Accepted on the total-time criterion instead, with the mechanism revised from TLB misses to
first-touch page faults in the copy and transpose. The TLB effect is untested. Source cost: +26 scored lines
(714 → 740), ≈ −0.12 points. Accepted on the VPS as a small gain: consistent sign (17/20 pairs, sign test p ≈ 0.001), −24% minor faults, peak
memory unchanged. The effect (−4 to −5%) is below single-board-run noise (±9%), so a board verdict needs the
resample protocol (candidate plus 2–3 record-only pushes vs the 21 baseline samples). Board risks to read
in the resample: with `defrag=madvise` the board may compact memory on these faults (no gain or a loss); under THP
`always` other allocations made inside the window (arena heaps, transpose counters) get huge pages too, so M must be
checked (reject if it rises by more than 1 MB).
Not submitted; no push.
