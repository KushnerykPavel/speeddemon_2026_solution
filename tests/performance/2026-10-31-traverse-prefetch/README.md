# Track D: software prefetch in search and renumbering

Baseline: `b9a93cd` (`before-main.rs`). Candidate: `prefetch-main.rs` (scored
source). `profile-*-main.rs` add stderr phase timers only.

## Change

A `prefetch` helper issues `_mm_prefetch(T0)` on x86-64 (no-op elsewhere).
- Search: after popping a vertex, prefetch the row bounds and first packed edge
  of the vertex now on top of that side's heap, so its row is loading while the
  current row is relaxed.
- Renumbering BFS: prefetch row bounds eight queued vertices ahead and the first
  edge four ahead.
Idea credit: Team7's public PERF.md (prefetch in BFS ordering and top of heap).

## Protocol

`my-vps`, six Haswell KVM vCPUs, CPU set 0-5, 8 GB cgroup, pinned Rust image,
offline native release builds (the x86-64 path compiles only here). Full
official seed 1, prewarmed inputs, one warmup plus five rotating-order runs,
trimmed mean of the middle three, cgroup v2 `memory.peak`. Phase profile: four
runs per variant and worker count, medians. All outputs byte-equal to Python.

| Variant-workers | T (s) | Spread | cgroup peak (MB) |
|---|---:|---:|---:|
| `b9a93cd`-6 | 0.5409 | 17.9% | 96.2 |
| prefetch-6 | 0.4818 | 25.4% | 96.4 |
| `b9a93cd`-12 | 0.5411 | 15.5% | 101.3 |
| prefetch-12 | 0.4734 | 14.0% | 101.2 |

| Phase median (s) | `b9a93cd` 6w | prefetch 6w | `b9a93cd` 12w | prefetch 12w |
|---|---:|---:|---:|---:|
| graph load | 0.176 | 0.163 | 0.152 | 0.147 |
| renumbering | 0.110 | 0.083 | 0.112 | 0.088 |
| search (incl. transpose) | 0.270 | 0.252 | 0.272 | 0.236 |

## Correctness

Fifteen Rust tests (`measured-prefetch-tests.rs`) and 204 Python cases under
default and 1/2/4/6/12 workers pass on macOS and VPS. Prefetch is a hint only;
no result depends on it. Release Clippy and rustfmt pass.
