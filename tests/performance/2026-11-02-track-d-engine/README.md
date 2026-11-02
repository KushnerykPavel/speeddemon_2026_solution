# Track D: bucket-ring search engine (2026-11-02)

Hypothesis: [research/plateau-2026-11-02.md](../../../research/plateau-2026-11-02.md) §6, rank 1. Step 29
([plateau model](../2026-11-02-track-d-plateau-model/README.md)) measured HighVelocity's search doing the same work
as ours (7.52M expansions vs 7.57M) at 1.5–1.75× lower CPU per expansion. About 20% of that advantage came from a
4-ahead prefetch; the rest was not attributed. This is our own implementation of the idea (no code copied). The
ideas reused are the bucket-ring FIFO drain, prefetching ahead of the queue, branch-free relaxation and blank-page
label reads. Idea credit: HighVelocity (MIT).

Pre-registered rule: search CPU per expansion on the VPS ≤ 110 ns (from 142–151) is the target; > 130 ns rejects.

## Mechanism
- Each side queues `(vertex, distance)` entries in a ring of 2,048 buckets, each 128 distance units wide. Keys
  beyond the ring wait in a small `far` heap.
- A bucket is drained in insertion order, so the row and label of the entry 4 places ahead are prefetched.
- Inside a bucket a vertex may be expanded before its final label. An improvement is re-queued into the same
  bucket, so the bucket is label-correcting. After a drain, every label below the bucket's end is final.
- The search stops when the two sides' bucket lower bounds sum to ≥ μ, or when a side is exhausted. Each step
  drains the side that has expanded fewer vertices.
- Relaxation writes the label and the queue entry unconditionally and advances the length only on improvement.
  The label page for an unassigned block is a shared blank page, so label reads do not branch.
- Overflowing `u32` sums are skipped and flagged as before. The `u64` rerun is now a one-way Dijkstra with
  zero-initialized (untouched, so not resident) labels, serialized by a mutex. It replaces the generic heap search.

## Versions (all byte-equal; source in this directory)
| Version | Queue storage | S (rustfmt, non-blank non-comment) | Mac CPU (s) | Notes |
|---|---|---:|---|---|
| before `before-main.rs` | binary heap | 714 | 1.03–1.15 | `539b15f` |
| v1 `engine-v1-main.rs` | `Vec<(u32, u32)>` per bucket | 910 | 0.76–0.86 | old generic heap search kept for `u64` |
| v2 `engine-v2-main.rs` | shared arena of 256-entry chunks | 814 | 0.78–0.86 | compact `u64` fallback |
| v3 `engine-v3-main.rs` | `Vec<u32>` per bucket (vertex + low 7 distance bits), capacity capped at 1,024 after a drain | 788 | 0.73–0.83 | |
| v3b `engine-main.rs` | as v3, cap 256 | 788 | 0.70–0.77 | candidate |

Retained queue memory per side, measured on the Mac at 12 workers: v1 0.55 MB, v2 64 KB, v3 0.12–0.16 MB,
v3b 0.06–0.08 MB.

## Correctness
- `engine-tests.rs`: all-pairs comparison against an independent `u64` Dijkstra.
  - 400 random graphs, up to 300 vertices, more than 1M pairs, at 1 and 3 workers.
  - Weight mixes: 0, < 5, < 130, < 1000, < 400k / 600k (ring wrap-around and the far heap), near 2^31, and near
    2^32 mixed with small weights.
  - Passes for every version, on the Mac and on the VPS (`vps-*/unit-*.txt`).
  - v2 also passes with CHUNK = 1 and 2 (stresses chunk chaining).
- Mutants, all caught (Mac):
  - no requeue inside a bucket;
  - optimistic lower bound (`(current + 1) << 7`);
  - meeting through an unlabelled vertex;
  - far heap never pulled (v1: hangs);
  - lost chunk link (v2);
  - `u64` fallback disabled;
  - dropped low distance bits (v3).
- VPS (each version): 204 Python-reference cases × 6 thread settings, plus invalid-input and invalid-thread
  checks, pass. Byte-equal to `before` on all ten validation datasets. Every timed run is checked against the
  Python full-seed-1 output (`a23dbdf0…`). Local Mac: full seeds 1 and 2 equal.

## VPS timing
`experiment.py`: 6 Haswell vCPUs (KVM), Docker CPU set 0-5, 8 GB cgroup, full seed 1 prewarmed, 11 alternating
rounds, round 0 discarded, trimmed mean of 10, cgroup `memory.peak`. Diagnostic phase builds (`diag_patch.py`)
are separate binaries.

| Version | 12 workers | 6 workers | peak MB at 12 w (before → engine) | search CPU (s) | expansions | ns/expansion |
|---|---|---|---|---:|---:|---:|
| v1 | −11.9%, 10/10 pairs | −7.9%, 8/10 | 78.8 → **89.4** | 1.076 → 0.897 | 7.43M | 142 → 121 |
| v2 | −4.1%, 6/10 | −3.3%, 7/10 | 78.8 → 80.8 | 1.132 → 1.069 | 7.43M | 150 → 144 |
| v3 | **−12.8%, 10/10** | −6.6%, 9/10 | 79.0 → 82.8 | 1.143 → **0.727** | 7.43M | 151 → **98** |
| v3b | −8.7% (paired −6.8%), 9/10 | **−11.8%, 10/10** | 79.4 → 82.1 | 1.031 → **0.727** | 7.43M | 136 → **98** |

The ns/expansion figures for "before" use the 7,573,787 settles from step 25. Expansions in the ring engine
vary by a few dozen between runs, because label-correcting re-expansions depend on the order of meeting and
balancing.

- v1's memory came from retaining every bucket's grown vector: 2,048 vectors per side, each sized to its
  largest-ever use.
- v2's arena fixed the memory, but its per-entry chunk bookkeeping cost most of the speedup on Haswell. The M1
  did not show this.
- v3 keeps v1's loop, halves the entry size and caps retention.

## Decision
**v3b is accepted on the VPS.** It meets the pre-registered target (98 ns per expansion, ≤ 110). Time falls
8.7–12.8% at 12 workers and 6.6–11.8% at 6, in 9–10 of 10 pairs in each of four runs of v3/v3b. The v3 and v3b
times are within noise of each other; v3b keeps less memory.
Costs: peak at 12 workers +2.7 MB (the search phase now sits just above the load peak), and S 714 → 788.
Estimated board effect (estimate, not measured), with T −10%, M +2.7 MB and S +74:
+1.14 − 0.14 − 0.32 ≈ **+0.7 points**. The board result needs the resample protocol (candidate plus 2–3
record-only pushes against the 21-run baseline mean of 0.2069 s), which requires explicit push authorization.
