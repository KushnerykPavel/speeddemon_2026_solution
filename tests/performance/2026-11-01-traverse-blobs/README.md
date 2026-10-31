# Track D: chained BFS blobs in the renumbering

Idea from the public PERF.md files of Cache Me (blobs of 512, search 0.33–0.42
→ 0.22–0.24 s on their machine) and FlyHigh (blobs of 64, −8% search locally,
expected larger on the board's smaller L3): instead of one ring-by-ring BFS
region per worker, grow breadth-first balls of at most `BLOB` vertices, each
starting from an unclaimed neighbour of the previous ball's edge.

Baseline `21df4fd` (`before-main.rs`). Variants: `blob512-main.rs`,
`blob128-main.rs`, `blob64-main.rs`. VPS, CPU set 0-5, 12 workers, six
rotating-order runs plus phase profile (medians), outputs equal to Python.

| Variant | T (s) | Spread | cgroup peak (MB) | renumbering (ms) | search (ms) |
|---|---:|---:|---:|---:|---:|
| `21df4fd` | 0.4747 | 6.0% | 78.8 | 120.7 | 259.5 |
| blob 512 | 0.4869 | 26.0% | 78.8 | 118.6 | 258.5 |
| blob 128 | 0.5102 | 14.1% | 91.5 | 133.8 | 295.5 |
| blob 64 | 0.5491 | 16.3% | 104.5 | 146.3 | 310.1 |

Blob 512 leaves the search unchanged on the VPS (258.5 vs 259.5 ms); smaller
blobs are slower and raise the peak. The VPS's cache is larger than the
board's, so a neutral result here does not rule out a gain there.
Thirteen Rust tests and 204 Python cases pass for every variant.
