# Buffered byte parser: Linux x86-64 comparison — 2026-10-30

**Result: correctness passed; no demonstrated speedup.** Both independent batches
gave a slower trimmed mean for the byte parser, with substantial shared-VPS noise.
This experiment does not justify promoting the parser as a performance improvement.
No implementation was reverted and no commit or push was performed for this measurement.

## Versions and isolation

Measured repository revision: `f70b65599340593274e6d8f08353821868ae514c`.
Track A's parser was introduced by `a2217a2`.

Before is the exact archived pre-parser working-tree source from
`../2026-10-30-ingest-byte-parser/before-main.rs`, not an earlier commit in its
entirety. It already contains the nested group maps, `sort_unstable` and tuned
release profile. Both versions have identical Cargo manifests and lockfiles.
Thus the comparison isolates the buffered byte-parser change.

Exact source snapshots: `before-main.rs`, `measured-main.rs`,
`before-Cargo.toml`, `measured-Cargo.toml`; hashes in `metadata.json` and both summaries.

## Environment and method

- Host: `my-vps`, native Linux 7.0.0-28-generic x86-64, glibc 2.43, KVM VM,
  six logical CPUs; exposed CPU model Intel Core Processor (Haswell, no TSX).
- Pin the Python driver and all child processes to logical CPU 2 with `taskset -c 2`.
- Build both in the same pinned Rust container using
  `cargo build --release --offline --locked`; rustc 1.99.1, no extra CPU flags.
  The complete image digest and commands are in `metadata.json`.
- Execute binaries natively on the Linux host. This is not a container-limit,
  cgroup-memory or official scoring-machine validation.
- Official full seed 1: 3,100,000 rows, 238,836,234 bytes. Correctness runs warm
  the input before timing. Process launch through exit is timed; builds, Docker
  startup, output comparisons and hashing are excluded.
- Each batch: one warmup plus five measured runs per version, alternating order.
  Drop min/max separately per variant; report the mean of the remaining three.
- Batch 2 was triggered because batch 1's spread exceeded 5%. It reused batch 1's
  Python outputs only after verifying reference source, input and output hashes.
- Shared host: CPU affinity does not remove VM scheduling or unrelated workload
  effects. Host load averages before/after and full `lscpu` output are archived.

## Timing results

| Batch | Before trimmed mean (s) | Byte parser (s) | Change in time | Spread before / after |
|---|---:|---:|---:|---:|
| 1 | 1.270541 | 1.313733 | +3.40% | 15.6% / 21.8% |
| 2 | 1.269154 | 1.283111 | +1.10% | 23.8% / 18.0% |

Spread means `max/min - 1` across the five measured runs. The sign and magnitude
of a small difference cannot be established confidently with this variation;
the evidence supports **no demonstrated performance win**, not a precise regression estimate.
Do not pool batches or compare their absolute times with older experiments.

All raw runs, including warmups, are retained in `raw1.txt`, `raw2.txt`,
`summary1.json`, `summary2.json` and `console.txt`.
Peak RSS returned by `wait4` was 49,078,272 bytes in batch 1 and 49,246,208 bytes
in batch 2 for both variants on every benchmark execution. This is not cgroup
`memory.peak`; process creation can also include inherited driver memory in the
high-water mark. These values do not establish a memory improvement.

## Correctness

Both versions matched Python byte-for-byte on all nine cases: empty, existing
edge cases, tiny/sample seeds 1/7/42 and full seed 1. Batch 1 freshly executed
the preserved Python reference; batch 2 verified and reused its outputs.
Sample/full seed-1 outputs matched published hashes. Every warmup and measured
execution also matched the full reference bytes. Full output SHA-256:
`495ca09682b9673ef836a506496177680bb4a3af54255174befddb1f83481cd3`.

Six optimized parser regression tests passed on Linux: integer/status boundaries,
malformed records, every small-buffer split with LF/CRLF and final-line variants,
empty input, records over 128 KiB and read errors. Output is in `regression-tests.txt`.
Tests were compiled with the pinned toolchain using
`rustc --edition=2024 --test -O tests/ingest-parser.rs -o parser-tests`.

## Reproduction and artifacts

`runner.py` is the existing documented native runner with two metadata adaptations:
accept compiler-version text captured from the build container and record actual
CPU affinity. Timing, correctness and aggregation logic are unchanged.
`remote-experiment.py` contains the exact remote build/check procedure.
It consumes a JSON object with `commit` and `files` (relative path to text contents)
on stdin and always creates a new temporary directory.

The source bundle contains the before crate, current `track-a` crate, lockfiles,
`tests/ingest-parser.rs`, `tests/reference/a_ingest.py`, the runner and a sibling
`before-main.rs`. `metadata.json` records every bundled file's hash.
Once built, the exact `taskset` runner commands are listed in that file.
Original remote directory: `my-vps:/tmp/ingest-byte-linux-mjj_tld6`.
Official input remains at
`/home/student/speed-demon-2026/data/rust-validation/a`.

Recommended decision: retain the archived pre-parser implementation as the
performance baseline; investigate phase costs or test on the official machine
before claiming a benefit from buffered byte parsing. Any implementation rollback
is a separate change, not part of this measurement.
