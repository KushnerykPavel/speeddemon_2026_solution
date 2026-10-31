# Rust baseline correctness checks

Track A byte-parser regression tests can also run locally without Docker:

```sh
rustc --edition=2024 --test -O tests/ingest-parser.rs -o /tmp/ingest-parser-tests
/tmp/ingest-parser-tests
```

These cover every buffer boundary, LF/CRLF and EOF tails, long records, numeric
limits, malformed fields, and read failures. Tests live outside the scored track.

Track D byte-parser regression and Python-reference checks:

```sh
rustc --edition=2024 --test -O tests/traverse-parser.rs -o /tmp/traverse-parser-tests
/tmp/traverse-parser-tests
cargo build --manifest-path track-d/Cargo.toml --release --offline --locked
python3 tests/traverse-check.py track-d/target/release/traverse-baseline
```

These cover parser boundaries and errors, 204 deterministic graph/query cases
(including 200 random graphs), and eight invalid inputs. Full-width weights,
distances above `u32::MAX`, zero weights, duplicate edges, self-loops,
unreachable vertices, empty queries, LF/CRLF and EOF tails are included. Degree
rejection checks cover isolated self paths, no-in targets, no-out sources,
self-loop-only components, bitset word boundaries and state reuse after rejection.
The Python comparison runs every case with default scheduling and explicit
1/2/4/6/12 workers (1,224 comparisons), plus three invalid thread options. Seven
Rust tests include repeated parallel-versus-serial comparisons with skewed work,
duplicate queries, large distances, fewer queries than workers and empty input.

`reference/` preserves the organisers' MIT-licensed Python implementations
unchanged, outside the independently scored track folders.

On a Linux Docker host, pre-pull the pinned Python, Rust and Debian images from
the track Dockerfiles and `scripts/verify-baselines.py`. Clone the official
`hackathon-raptors/speed-demon-2026` generator repository, then run:

```sh
python3 scripts/verify-baselines.py \
  --harness /path/to/speed-demon-2026 \
  --data-root /path/to/data \
  --results /path/to/new-results-directory
```

The default matrix covers both tracks, `tiny` and `sample`, seeds 1, 7 and 42,
plus five explicit edge cases. It generates data in Python 3.13, runs the
Python references, builds Rust in release mode offline, and calls the original
`check.py` under the competition container restrictions. It compares complete
output bytes and checks published seed-1 hashes where available. A failure
stops the script. Each results directory must be new so previous evidence is
preserved. Logs, reference outputs and `summary.json` remain there.

For full-scale checks on the public development seed:

```sh
python3 scripts/verify-baselines.py \
  --harness /path/to/speed-demon-2026 \
  --data-root /path/to/data \
  --results /path/to/new-full-results-directory \
  --scales full --seeds 1
```

Reported Rust times are single correctness-run wall times including Docker
startup, not controlled benchmarks or official scoring measurements.
