# Track D: performance log

Every step from the reference to your final version, each with the live-board run ID that measured it. Judges check that the run IDs exist and that each change moved the way you say it did.

| Step | Change | Run ID | T (s) | M (MB) | Points |
|---|---|---|---|---|---|
| 0 | The reference, unchanged | | 72.69 | 893 | 0 |
| 1 | Rust baseline port; VPS correctness verified | | | | |
| 2 | CSR graph (`u32` offsets, packed edges) + reusable array distances with touched-list reset, replacing per-query HashMap | | | | |
| 3 | Release build: fat LTO, one codegen unit, abort on panic; native CPU in Docker build | | | | |

The Rust baseline preserves adjacency lists and one fresh Dijkstra search per
query, with a hash map and binary heap. No external dependencies.
VPS tiny/sample/full outputs match Python byte-for-byte. Validation evidence
is in `../tests/BASELINE_VALIDATION.md`. No live-board run ID or official
performance measurement is available for this step yet.

Step 2: local check only (not a live-board run). 200 random small graphs match the
Python reference byte-for-byte. On a synthetic 90k-node grid with 300 queries
(Mac), wall time fell from 6.3 s to 1.7 s with identical output. No run ID yet.

Step 3: build configuration only. Docker sets `-C target-cpu=native`; per the
documented board behavior, this requests a build on the scoring machine so CPU
features match the runtime. Local Cargo builds keep their default CPU target.
Local VPS full-seed1 comparison (same CSR source, CPU 2, warm page cache, one
warmup plus five alternating measurements per version): default release
18.340520 s versus tuned release 17.407554 s, trimmed means (5.09% less time).
All twelve outputs match the saved Python reference byte-for-byte. Baseline
max/min spread was 28.26%, so the improvement is inconclusive on the shared VPS;
settings are retained provisionally pending a scoring-machine measurement.
No memory/points measurement or live-board run ID. Reproduction and raw evidence:
`../tests/performance/2026-10-30-traverse-build-profile/README.md`.
Local validation: native-CPU release build, rustfmt and release Clippy passed;
all three existing D fixtures match Python byte-for-byte (edge cases, no edges,
no queries). VPS Docker builds and full-seed1 execution passed; the local Docker
daemon remains stopped.
