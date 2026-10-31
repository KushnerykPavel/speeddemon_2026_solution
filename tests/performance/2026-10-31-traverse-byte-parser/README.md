# Track D byte parser — local loading benchmark

Baseline: commit `531def7dd3ab135c5d606d3e2d3a4aad6536596c`.
Candidate: direct checked ASCII `u32` parser in `track-d/src/main.rs`, reusable
line buffers for headers, edges and queries; unchanged CSR and Dijkstra.
No external dependencies. Parser regression tests live outside the scored track.

On macOS arm64, Rust 1.98.0, a synthetic 100,000-vertex / 1,000,000-edge graph
with 10,000 source=target queries gave median wall times of **0.1700 s before**
and **0.0447 s after** (73.71% reduction). One warmup and five alternating runs
per variant; every output matched the expected 10,000 zero distances.
`summary.json` contains all timings, environment and the generated graph hash.
There was visible timing noise, particularly in the first candidate measurement.

This deliberately makes query work trivial to expose input-loading cost. It is
not a full-track benchmark or a board result; no Linux, full official dataset,
memory or points measurement was performed for this change.

Reproduce from the repository root:

```sh
git show 531def7dd3ab135c5d606d3e2d3a4aad6536596c:track-d/src/main.rs > /tmp/traverse-before-byte-parser.rs
rustc --edition=2024 -O -C lto=fat -C codegen-units=1 -C panic=abort /tmp/traverse-before-byte-parser.rs -o /tmp/traverse-before-byte-parser
cargo build --manifest-path track-d/Cargo.toml --release --offline --locked
python3 tests/performance/2026-10-31-traverse-byte-parser/runner.py \
  /tmp/traverse-before-byte-parser track-d/target/release/traverse-baseline
```

Correctness checks are documented in `tests/README.md`: five parser tests passed,
203 deterministic cases matched the preserved Python reference byte-for-byte,
and eight malformed-input cases failed without panicking. Formatting, release
build and release Clippy with warnings denied passed.
