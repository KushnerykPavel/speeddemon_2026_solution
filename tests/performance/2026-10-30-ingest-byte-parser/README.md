# Buffered byte parser — 2026-10-30

Track A now scans complete records in a 64 KiB `BufReader` buffer, retaining only
split records in a reusable byte vector. Decimal fields use checked byte parsing;
endpoint lookup, grouping, sorting and output remain the same.

The baseline is the pre-change **working tree**, including existing group-key,
`sort_unstable` and release-profile changes, rather than a named commit.
`before-main.rs` and `before-Cargo.toml` preserve that exact input. The final
candidate is `track-a/src/main.rs`; source and binary hashes are in
`native-summary.json`. No implementation changes were made to Track D.

## Correctness

All nine cases compared Python reference, before, and final after output bytes:
empty, existing edge fixtures, tiny/sample seeds 1/7/42, and full seed 1.
Published hashes matched for sample/full seed 1. The full output hash is
`495ca09682b9673ef836a506496177680bb4a3af54255174befddb1f83481cd3`.

Inputs were copied read-only from the existing `my-vps` validation data at
`/home/student/speed-demon-2026/data/rust-validation/a`. Every input hash is
preserved in the summaries; no generator was rerun or modified.

Python 3.12.7 ran the preserved reference locally. Final-candidate comparisons
reused those reference outputs after verifying the reference source, every input,
and every cached output hash against the first run's summary.

Additional complete-output comparisons passed for CRLF, no final newline, and
a record over 128 KiB. Missing/extra fields and numeric overflow exited 1 without
panicking (`extra-checks.json`). Six Rust regression tests passed in optimized
mode, covering all buffer capacities from 1 through the multi-record fixture's
length plus one, numeric/status limits, malformed records, long records, and
read failures. Debug-mode tests also passed for the first implementation.

Release offline/locked build, Clippy with warnings denied, formatting and diff
whitespace checks passed. Docker was unavailable locally: these are native checks,
not validation of the pinned Linux container or its restrictions.

## Performance: no demonstrated improvement

Native macOS 14.5 arm64, ten logical CPUs, rustc 1.98.0, identical manifests and
`cargo build --release --offline --locked`. No CPU affinity or exclusive host use.
Full official seed-1 input, warm cache. One warmup plus five measured runs per
variant, alternating order. Timers include child spawn through exit, exclude
Docker and byte comparisons. Peak memory is per-child RSS from `wait4`, **not**
cgroup `memory.peak`.

| Measured round | Before (s) | Final after (s) |
|---|---:|---:|
| 1 | 0.889659 | 0.872518 |
| 2 | 0.854961 | 1.057022 |
| 3 | 0.804849 | 0.852115 |
| 4 | 0.852788 | 0.826622 |
| 5 | 1.090977 | 1.043208 |

Dropping min/max independently gives **0.865802 s before, 0.922614 s after**:
the candidate is 6.56% slower in this measurement. Spread (`max/min − 1`) is
35.6% before and 27.9% after; the magnitude is uncertain and there is no evidence
of an end-to-end speedup. Measured peak RSS ranges were 58,228,736–62,324,736 bytes
before and 56,606,720–62,357,504 bytes after. These overlapping ranges do not
establish a memory improvement.

An initial parser explicitly counted separator positions across the whole row.
Its source, raw output and summary are preserved in `first-*`; its trimmed time
was also worse than baseline (0.861536 vs 0.817358 s, +5.41%). The final parser
uses a simpler borrowed byte-slice split iterator. The two candidates were not
benchmarked in a controlled head-to-head comparison, so no speed gain is credited
to that simplification.

The requested byte-parser design is implemented, but it is **not a measured
performance win**. Run a controlled Linux x86-64 comparison before treating it as
a scoring optimization. No board run was performed and `track-a/PERF.md` was not
changed.

## Reproduction

Create a fresh temporary baseline crate containing `before-main.rs` as
`src/main.rs`, `before-Cargo.toml` as `Cargo.toml`, and the unchanged
`track-a/Cargo.lock`. Build both crates with the identical offline/locked release
command. With copied official case directories under `$DATA`, run from the repo:

```sh
python3 tests/performance/2026-10-30-ingest-byte-parser/runner.py \
  --repo "$PWD" --data-root "$DATA" \
  --before "$BEFORE/target/release/ingest-baseline" \
  --after "$PWD/track-a/target/release/ingest-baseline" \
  --work "$NEW_WORK" --summary "$NEW_SUMMARY"
```

The work directory must not already exist. Raw logs and all runs, source/input/
output hashes, environment and per-run RSS are preserved alongside this README.
Original local work: `/private/tmp/ingest-byte-parser-validation/{run1,run2}`;
temporary data and binaries may eventually disappear.
