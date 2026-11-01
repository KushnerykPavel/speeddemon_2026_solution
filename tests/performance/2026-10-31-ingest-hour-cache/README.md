# Track A: one-entry hour cache — kept

Audit completed 2026-11-01 Europe/Warsaw. Directory label reflects when the audit
began. Baseline: Track A source at `61d9b2a699ac37922ec6f668f6620ca745b7aa55`.
Only cache hypothesis implemented; nine alternatives remain untested.
[Ten-hypothesis audit](../../../docs/optimization-plateau-2026-11-01.md).

## Change and evidence

`Groups` holds the last `(hour, ID)`. An exact 13-byte comparison reuses that ID;
otherwise the unchanged hour dictionary lookup finds or creates it. IDs remain
stable as new hours/endpoints arrive. Parsing completes before cache mutation.
No ordering assumption, endpoint catalogue, dependency, unsafe code, compiler
flag, quantile change or parallelism was introduced. Eight net Rust code lines
added; approximate total including Dockerfile: 183 → 191. Canonical board S is
not yet measured.

Baseline-only diagnostic: five full runs, median ingestion 1.1387 s versus
0.0932 s finalize/output. Only 1,054 hour transitions / 3.1 million records;
sample seeds 1/7/42 have 872/850/874 transitions in 310,000 records. Tiny seed 1
has 152/1,000. This identifies a plausible target, not the precise hashing cost.

## Results

| Input | Baseline s | Cache s | ΔT |
|---|---:|---:|---:|
| full seed 1 | 1.18864 | 1.05518 | −11.23% |
| full repeat | 1.23853 | 1.12672 | −9.03% |
| sample seed 7 | 0.15665 | 0.13846 | −11.62% |
| sample seed 42 | 0.16441 | 0.14340 | −12.78% |
| alternating hours | 0.09478 | 0.10395 | +9.68% |
| alternating repeat | 0.09464 | 0.09843 | +4.00% |
| shuffled hours | 0.09498 | 0.09233 | −2.79% |
| constant hour | 0.08690 | 0.07253 | −16.53% |

Each variant gets one warmup; AB/BA order alternates. Initial full: ten measured
runs per version. Other initial cases: six. Repeat full: six. Repeat alternating:
six measured batches of five separate executions per variant; per-process mean
within batch, then trim across batches. Tables give mean after dropping one
minimum and maximum per variant. Process start, output writing and exit included;
reference comparisons and builds excluded. CPU affinity 2, warm input, same
pinned Rust image and offline release build with no extra RUSTFLAGS. Native Linux
on six-vCPU Haswell-class KVM VPS; not competition container timing. No host
exclusivity/governor control. Exact CPU/compiler details are saved.

Full spreads 24.2%/11.4%, repeat 8.7%/19.4%; all ten first-batch full pairs favor
the candidate. Two batches support the direction, not an exact universal speedup.
Miss-heavy synthetic input regresses in both batches; magnitude remains noisy.
Keep for observed generated-workload locality across multiple seeds; generic
fallback preserves correctness for arbitrary order. No claim all inputs improve.

Initial wait4 RSS inherited the large Python driver's high-water mark (~284 MB)
and is **excluded**. Independent `/usr/bin/time` measurements in repeat:
26.28–26.50 MB baseline, 26.35–26.50 MB cache (three each). No material RSS change;
not cgroup memory.peak. Holding M constant and approximate S 183 → 191, full-run
K multipliers would be 1.0546 and 1.0417. This is a conditional model, **not an
official board score**. No board run, commit or push performed.

## Correctness and checks

Nine optimized Rust tests pass locally and on VPS. The two added tests cover
runs, alternating hours, revisits, month rollover, changing endpoints and
rejected input preserving cache/state. Formatting and Clippy pass. One review
iteration: OO, clean architecture and API reviewers all approve, zero findings.

Twelve cases match Python byte-for-byte: empty, edge fixtures, tiny/sample
seeds 1/7/42, full seed 1, and independently generated synthetic alternating,
shuffled and constant-hour inputs (300,000 records each, seeds 3/19/101, full
u32 latency values and changing endpoints/statuses). No synthetic answers are
copied from Rust. Official cached Python reference source, inputs and outputs
are SHA-verified before reuse. Every initial timed run checks bytes and SHA;
every repeat/memory run verifies SHA. Sample/full published hashes match.

Full seed-1 output SHA-256:
`495ca09682b9673ef836a506496177680bb4a3af54255174befddb1f83481cd3`.

## Artifacts and reproduction

- `before-main.rs`, `candidate-main.rs`, `measured-tests.rs`, Cargo snapshots:
  exact measured sources; source and binary hashes in `summary.json`.
- `run-diagnostic.py`, `diagnostic.txt`: timing-only baseline and hour locality.
- `remote-experiment.py`: isolated VPS driver, build/reference/benchmark protocol.
- `run-experiment.py`: local source bundler and SSH launcher.
- `summary.json`: **complete** first batch, source provenance, hashes and all runs.
- `console.txt`: partial streaming log; original SSH stopped delivering after
  round 9, but the VPS job completed. Summary/metadata were recovered through a
  fresh bounded connection. The partial console is not the timing authority.
- `repeat.py`, `repeat-console.txt`, `repeat-summary.json`: complete repeat,
  separate memory measurements and all raw samples.
- `cpu.txt`, `rustc-version.txt`, `regression-tests.txt`, `design-review.md`:
  environment and verification evidence.

Remote experiment: `my-vps:/tmp/ingest-hour-ab-_4gt3c1q`.
Diagnostic: `/tmp/ingest-hour-on7kswwh`. Sources/builds/output stay in those fresh
temporary directories; no remote checkout was changed. Official input data:
`/home/student/speed-demon-2026/data/rust-validation/a`. Verified cache:
`/tmp/ingest-byte-linux-mjj_tld6/batch1` plus its adjacent `summary1.json`.
If cache is missing, regenerate the Python outputs and validate published hashes;
do not silently bypass cache-source/input/output validation.

From repo root, `python3 tests/performance/2026-10-31-ingest-hour-cache/run-experiment.py`
rebuilds snapshots in a fresh VPS directory. It requires configured SSH and the
cached pinned Rust image. To repeat, run `repeat.py` on VPS with that new remote
directory as its argument. Keep independent source/toolchain/order/affinity
identical. Runtime RSS does not replace a final restricted-container cgroup
measurement on board-class hardware.
