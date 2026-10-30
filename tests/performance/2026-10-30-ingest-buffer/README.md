# Ingest line-buffer experiment — 2026-10-30

Before: `55522c458f6bd6463544d6bc130e2f9ed5b97014`.
After: `389721669dd32ecd60a25e35012e1e33e29f2896`.
Only the Track A line buffer and field splitting changed.

## Preserved evidence

- `vps-raw.txt`: original build and timing output copied from local temporary storage.
- `vps-summary.json`: all runs, source SHA-256, conditions and computed statistics.
- `runner.rs`: exact Rust timing/correctness runner used inside the container.
- `board-team-snapshot.json`: complete Kushneryk team entry and rules from the feed
  generated at 15:32:11 UTC (16:32:11 Warsaw), including A and D results.
- `board-baseline-transcribed.json`: selected baseline fields transcribed from
  the earlier feed read in the chat at feed time 15:19:42 UTC; not an original raw snapshot.

Board source: <https://s3.amazonaws.com/raptors.dev/speeddemon.dev/2026/board/board.json>.
Live view: <https://speeddemon.dev/board/>. These links change over time;
the archived JSON records the observations used here.

## Local timings

| Round | Before (s) | After (s) | Included in measured set |
|---|---|---|---|
| 0 | 2.073549519 | 1.809828907 | No: warmup |
| 1 | 2.012824176 | 1.573380868 | Yes |
| 2 | 1.931898139 | 1.804865451 | Yes |
| 3 | 2.231016527 | 1.610122782 | Yes |
| 4 | 2.259817460 | 1.719828636 | Yes |
| 5 | 2.146541772 | 1.678517319 | Yes |

Discard min/max independently for each variant: before 2.130127492 s,
after 1.669489579 s; speedup 1.275915417x, time reduction 21.62489872%.
Five measured runs plus one warmup per variant. Orders alternate each round.
Every output was compared byte-for-byte with the first run; its SHA-256 matches
the official seed-1 full output. No Python full run was performed in this experiment.

Shared `my-vps`: Linux 7.0.0-28-generic x86_64, six logical CPUs; pinning to CPU 2.
CPU model/governor were not copied into the local log; the remote experiment
directory retains `cpu.txt`. No exclusivity from unrelated host work was enforced.
No peak memory was measured locally. Spread was roughly 17% before and 15% after
(max/min minus one); treat the local ratio as an estimate. Board measurements
independently confirm the improvement on the official machine.

## Build and run conditions

Both versions used identical manifests/lockfiles and
`cargo build --release --offline --locked`, with no added CPU flags.
Compiler: rustc 1.99.1 (58cf8f9c3 2026-10-13).

- Build image: `rust:1-slim-trixie@sha256:1a939748a7cbab172de91a014e3804a97af1394a8c844faa67fce9401c4e40fb`.
- Runtime: `debian:13-slim@sha256:62bd423aeee912639ca8ee681c2b762cb3e25137e0a92b20be455898a9f034ad`.
- Input: official full seed 1, 3,100,000 rows, 238,836,234 bytes, read-only mount.
- Input was read sequentially once on the host before the runner to warm page cache.
- Runtime flags: `--network=none --read-only --cpuset-cpus=2 --memory=8g
  --memory-swap=8g --pids-limit=512 --cap-drop=ALL
  --security-opt=no-new-privileges --user 65534:65534`.
- Writable `/work` holds binaries and output; input mounted at `/data:ro`.
- One long-lived container hosts the runner. `Instant` wraps each child process
  launch/wait; Docker startup, file comparison and hash calculation are excluded.
  Child process initialization and teardown are included.

This is a controlled before/after experiment, not an exact reproduction of every
board harness setting. The live-board JSON declares one run per push; do not
describe its results as the final five-run trimmed scoring protocol.

## Reproduction

Use a new writable experiment directory mounted at `/work`. Export each commit's
`track-a` into `/work/before` and `/work/after`, preserving `Cargo.toml`, `Cargo.lock`
and `src/main.rs`; build both using the pinned Rust image and command above.
Compile this directory's `runner.rs` with that same image:
`rustc --edition=2024 -O runner.rs -o runner`.
Warm the full input, then run `/work/runner` in the pinned Debian image using the
runtime flags and mounts above. The runner expects binaries at
`/work/{before,after}/target/release/ingest-baseline`.

Original remote artifacts: `my-vps:/tmp/ingest-speed-9rls_vfu`.
Original remote input: `/home/student/speed-demon-2026/data/rust-validation/a/full-seed1`.
Those temporary paths may disappear; the measurements and runner are preserved here.

## Official Track A comparison

| Metric | Before | After |
|---|---|---|
| Run ID | run-735e9cb7363e | run-aa9a80757414 |
| T (s) | 1.397731 | 1.087126 |
| M (bytes) | 62058496 | 61673472 |
| S | 69 | 87 |
| D | 1 | 1 |
| K | 6.020118 | 6.601109 |
| Points | 38.980249 | 40.980844 |
| Gates | All passed | All passed |

Time reduction: approximately 22.22%; speedup: 1.2857x. Points increased by
2.000595 despite the source-size increase. These public-seed live results do not
predict the final holdout score.

Track D reran unchanged: 12.531876 s / 22.057032 points became
12.360906 s / 22.202853 points. Its variation is not credited to this optimization.

## Correctness scope

The earlier baseline multi-seed matrix is documented in `tests/BASELINE_VALIDATION.md`.
For this optimization: release build, formatting and Clippy passed; byte comparisons
with Python passed for the existing percentile/date/order fixtures, empty input,
CRLF and no final newline. Missing/extra fields were rejected. Full seed-1 output
matched in all 12 local executions and passed the official board correctness gate.
No new full multi-seed matrix was run for this change.
