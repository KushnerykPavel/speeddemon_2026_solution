# Track C: performance log

Every step from the reference to your final version, each with the live-board run ID that measured it. Judges check that the run IDs exist and that each change moved the way you say it did.

| Step | Change | Run ID | T (s) | M (MB) | Points |
|---|---|---|---|---|---|
| 0 | The reference, unchanged | | 14.44 | 367 | 0 |
| 1 | Rust: mmap, parallel chunked parsing, per-thread aggregates and name tables, bitset merge | pending | | | |

## Step 1 — first Rust version, 2026-11-01

VPS (6 Haswell vCPUs, Docker with the board's flags, sd-timer, cgroup `memory.peak`, full seed 1, 7 runs):
T 0.632 s trimmed mean, M 67.75 MB, output SHA-256 `d8088b1cd42b…` (the published full-seed-1 hash).
Local Mac phase timings at 12 threads: regions 26–44 ms, parse 110 ms, merge interning 42–46 ms.
