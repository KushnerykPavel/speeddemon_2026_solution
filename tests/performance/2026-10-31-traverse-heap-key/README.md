# Track D: single-u64 heap keys for the u32 search

Baseline: `a37144d` (`before-main.rs`, heap entries `Reverse<(u32, u32)>`).
Candidate: `heapkey-main.rs` (scored source). `profile-*-main.rs` add stderr
phase timers only.

## Change

`Distance` gains an associated heap key. The narrow `u32` search pushes
`distance << 32 | node` as one `u64`, which orders exactly like
`(distance, node)` but sifts with one comparison; the wide `u64` fallback keeps
its tuple key. Idea credit: Team7's public PERF.md (packed `u64` heap keys).

## Protocol

`my-vps`, six Haswell KVM vCPUs, CPU set 0-5, 8 GB cgroup, pinned Rust image,
offline native release builds. Full official seed 1, prewarmed inputs, one
warmup plus five rotating-order runs, trimmed mean of the middle three, cgroup
v2 `memory.peak` via `measure.rs`. Phase profile: four runs per variant and
worker count, medians. All outputs byte-equal to the saved Python reference.

| Variant-workers | T (s) | Spread | cgroup peak (MB) |
|---|---:|---:|---:|
| `a37144d`-6 | 0.5917 | 14.8% | 96.4 |
| heapkey-6 | 0.5627 | 19.0% | 96.4 |
| `a37144d`-12 | 0.5835 | 8.1% | 101.2 |
| heapkey-12 | 0.5587 | 18.7% | 101.4 |

| Phase median (s) | `a37144d` 6w | heap key 6w | `a37144d` 12w | heap key 12w |
|---|---:|---:|---:|---:|
| graph load | 0.176 | 0.167 | 0.148 | 0.149 |
| renumbering | 0.102 | 0.107 | 0.099 | 0.114 |
| search (incl. transpose) | 0.293 | 0.262 | 0.304 | 0.283 |

Total time falls 4.9% / 4.2% (6 / 12 workers), within the
14–19% run spread; the search phase alone falls about 10% / 7%.

## Correctness

Fifteen Rust tests (`measured-heapkey-tests.rs`) and 204 Python cases under
default and 1/2/4/6/12 workers pass on macOS and VPS. Mutation caught: swapping
the key halves (node above distance) fails 2 tests. Release Clippy and rustfmt
pass.
