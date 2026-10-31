# Track D: memory diet (8-byte buckets, order-then-copy renumbering)

Baseline: `d6954e2` (`before-main.rs`). Scored candidate: `balanced-main.rs`.
`profile-*-main.rs` add stderr phase timers only; `memphase-*/memphase-main.rs`
add cgroup `memory.current`/`memory.peak` marks at phase boundaries.

## Why

The cgroup peak is the maximum over phases. At `d6954e2` (12 workers) the load
peak was 101.2 MB (12-byte bucket records + CSR), renumbering 94.1 MB (region
edge buffers + old graph), search 93.7 MB. All three had to drop together.

## Changes kept

1. **Load:** bucket records are `(source, packed target | weight code)`, 8 bytes
   instead of 12; weights that do not fit the code go to a per-bucket list in
   record order and are read back in the same order during placement.
2. **Renumbering, order then copy:** the breadth-first pass only claims
   vertices and records each region's visiting order (no edge buffers). Equal
   pieces of the concatenated order (8 per worker, dealt round-robin) first
   write their degrees, a serial prefix sum gives offsets, then the same pieces
   copy rows from the old graph, translating heads through the claims. Pieces
   rather than regions because regions differ in size.
3. **`mallopt(M_MMAP_THRESHOLD, 256 KB)`** next to the THP opt-out, so freed
   large buffers return to the kernel instead of glibc raising its threshold.

## Variants tried and dropped

| Variant | What | Result (12 workers) |
|---|---|---|
| diet (first run) | changes 1–3 plus search-state trimming in `Side::reset` | 79.3 MB, +11% time; search +30 ms |
| keep | diet without the trimming | 79.3 MB, +3.4% — trimming saved no peak and cost time |
| nomall | diet without `mallopt` | 98.2 MB — the threshold is what makes the diet count |
| freepart / freepart32 | free each bucket part right after placement (256 KB / 32 KB threshold) | 78.7 / 78.8 MB, +6% / +8.5% — no gain |
| claimpf | prefetch claims of the heads four rows ahead in the copy | renumbering 0.112 → 0.121 s in profile — no gain |

## Protocol

`my-vps`, six Haswell KVM vCPUs, CPU set 0-5, 8 GB cgroup, pinned Rust image,
offline native release builds. Full official seed 1, prewarmed inputs, one
warmup plus rotating-order runs, trimmed mean, cgroup v2 `memory.peak`. Phase
profile medians. All outputs byte-equal to Python. Each sub-directory holds its
own `experiment.py`, `summary.json`, `profile-summary.json` and raw rows.

| Run | Variant-12 | T (s) | Spread | cgroup peak (MB) |
|---|---|---:|---:|---:|
| first-run | `d6954e2` | 0.4631 | 14.0% | 101.4 |
| first-run | diet | 0.5139 | 7.0% | 79.3 |
| attribution | `d6954e2` | 0.4709 | 10.3% | 101.5 |
| attribution | keep | 0.4869 | 20.2% | 79.3 |
| balanced | `d6954e2` | 0.4642 | 15.9% | 101.2 |
| balanced | keep | 0.4788 | 20.5% | 79.1 |
| balanced | **balanced (scored)** | **0.4807** | 10.6% | **79.0** |

| Phase median (s), `balanced` run | `d6954e2` | keep | balanced |
|---|---:|---:|---:|
| graph load | 0.102 | 0.103 | 0.091 |
| renumbering | 0.087 | 0.129 | 0.114 |
| search (incl. transpose) | 0.248 | 0.265 | 0.268 |

Net at 12 workers: time +3.6%, peak −22.0%. By the score formula
(`0.2·log10` memory, `0.5·log10` time) that is about +0.7 points before the
code-size term.

## Where the peak is now (`memphase-balanced/rows.json`, 12 workers, MB)

| Mark | current | peak |
|---|---:|---:|
| buckets parsed | 50.5 | 50.7 |
| CSR placed | 78.8 | 78.8 |
| graph loaded | 33.5 | 78.8 |
| order built | 48.0 | 78.8 |
| rows copied | 74.7 | 78.8 |
| old graph dropped | 45.7 | 78.8 |
| transposed | 58.7 | 78.8 |
| searched | 31.8 | 78.8 |

The peak is the end of placement (buckets + new CSR); the renumbering copy
(two packed graphs + claims + orders) is next at ~75 MB. Search no longer sets
the peak.

## Correctness

Sixteen Rust tests (`diet-tests.rs`, includes `balanced-main.rs`) and 204
Python cases under default and 1/2/4/6/12 workers pass on macOS and the VPS;
every timed run's output is byte-equal to the Python reference
(sha256 `a23dbdf0…277e`).
