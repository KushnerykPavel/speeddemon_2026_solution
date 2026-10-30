# Interned endpoints and numeric groups — 2026-10-30

Baseline: Track A at `f70b65599340593274e6d8f08353821868ae514c`.
Candidate: uncommitted working-tree change, preserved in `measured-main.rs` and
`candidate.patch`. Exact source hashes are in `metadata.json` and both summaries.

## Implementation

Intern endpoint strings once in `HashMap<String, usize>`. Assign calendar hours
stable IDs using `HashMap<[u8; 13], usize>`. Store groups as `rows[hour_id][endpoint_id]`.
Rows grow independently with empty groups as necessary; a new endpoint does not
change a stride or move existing IDs. No fixed endpoint count or date range is used.
Latency vectors remain per group; no latency arena or quantile algorithm change.

Sort only unique endpoint names and hours for output. Iterate those sorted keys,
look up numeric groups and skip absent/empty combinations. Full byte comparison
preserves endpoint prefix ordering. The byte parser and build options are unchanged.

## Results

| Batch | Before (s) | After (s) | Time reduction | Spread before / after |
|---|---:|---:|---:|---:|
| 1 | 1.291992 | 1.216720 | 5.83% | 23.5% / 27.6% |
| 2 retry | 1.314827 | 1.265930 | 3.72% | 9.2% / 15.8% |

Small improvement observed in both batches; shared-VPS noise prevents a precise
speedup claim. The official score, including code-size and memory effects, is unknown.
No board run was requested. No commit or push was performed.

Each batch uses a warmup plus five measurements per variant in alternating order.
Trim min/max independently and average the middle three. Spread is `max/min - 1`.
Timing spans process spawn through exit, excludes builds and comparisons.
All 24 warmup/measured executions matched the full reference bytes.

Native Linux x86-64 `my-vps`, six-vCPU Haswell-class KVM VM, CPU affinity 2.
Both versions built with identical tuned manifests and rustc 1.99.1 in the pinned
Rust image, using `cargo build --release --offline --locked` and no extra CPU flags.
The full official seed-1 input was warm from correctness runs.
Runtime is the native host, not the competition's constrained container.
CPU and compiler details are in `cpu.txt` and `rustc-version.txt`.
Host exclusivity/governor was not controlled; host-load metadata was not recovered.

Peak RSS from `wait4` was identical for both variants within each batch:
49,184,768 bytes in batch 1, 49,176,576 in batch 2. Driver memory inherited around
process creation can affect this high-water mark. No cgroup memory.peak was
collected and no memory improvement is claimed. Dense rows also reserve empty
combinations: a larger, sparse hour/endpoint cross-product can use extra memory.

## Correctness

Both versions matched preserved Python reference output on all nine cases in
both batches: empty, edge fixtures, tiny/sample seeds 1/7/42 and full seed 1.
Reference source, inputs and cached outputs were hash-verified before reuse from
the preceding Linux parser check. No new Python reference execution or generator
run was necessary. Published sample/full seed-1 hashes matched.
Full output SHA-256:
`495ca09682b9673ef836a506496177680bb4a3af54255174befddb1f83481cd3`.

Seven regression tests passed locally and on Linux; the new test verifies stable
IDs under out-of-order hours, late endpoint discovery, revisits and empty groups.
Existing tests cover numeric boundaries, malformed rows, every small-buffer split,
LF/CRLF, final lines, long records and read errors. Optimized Linux output is
in `regression-tests.txt`. Local release build, formatting and Clippy passed.

## Artifacts and reproduction

- `before-main.rs`, `measured-main.rs`, corresponding Cargo manifests: exact versions.
- `candidate.patch`: candidate relative to baseline.
- `raw1.txt`, `raw2-retry.txt`, `summary1.json`, `summary2.json`: all successful runs.
- `console.txt`, `raw2.txt`: first execution and failed second-batch attempt.
- `resume-console.txt`, `repeat-command.json`: successful repeat.
- `runner.py`: unchanged timing/correctness runner from the Linux parser check.
- `remote-experiment-first.py`: initial procedure, including the failed cache path.
- `remote-experiment.py`: corrected fresh-directory reproduction procedure.
- `remote-repeat.py`: exact procedure used to resume without rebuilding or overwriting results.
- `metadata.json`, `cpu.txt`, `rustc-version.txt`: provenance and environment.

The failed second batch tried to find reference outputs under its own first
batch's directory, but cached references had not been copied there. It failed
before the first timing. The retry read the original cache and checked its hashes;
failed output was never included in the statistics.

Remote work: `my-vps:/tmp/ingest-dense-e_fg95z0`.
Original verified reference cache: `/tmp/ingest-byte-linux-mjj_tld6/batch1`;
its source/input/output hashes are in that experiment's archived summary.
Official data: `/home/student/speed-demon-2026/data/rust-validation/a`.
For a fresh reproduction, supply the JSON source bundle described in the
[Linux parser protocol](../2026-10-30-ingest-byte-parser-linux/README.md) to
`remote-experiment.py`, using this directory's snapshots and current regression
tests. The reference cache must exist; otherwise freshly generate Python outputs
with `runner.py` before using cached-reference options.
