# D7: Linux x86-64 VPS validation

Host: `my-vps`, six KVM vCPUs exposed as Intel Haswell without TSX. This is a
shared VPS, not the six-core/twelve-thread competition machine.
The remote workspace was clean at `f78482e` and was left unchanged. All builds,
outputs and logs live in the isolated `/tmp/traverse-d7-h8ToLK` directory.

Baseline is commit `292002c9be369bf08a320506ccf1a5080d6a2dfd` (D5 serial).
Candidate is the local D7 working tree; exact SHA-256 hashes of both source trees,
Cargo files and test tools are in `source-metadata.json`. The source change is
preserved in `../2026-10-31-traverse-parallel/candidate.patch`.

## Protocol

Both variants were built offline with the track release profile and identical
`RUSTFLAGS=-C target-cpu=native`, using the pinned Rust image from the Dockerfile.
Compiler details and the VM's reported CPU information are preserved separately.

Runtime containers use pinned Debian 13, CPU set `0-5` for every variant,
8 GB memory with no additional swap allowance, 512 processes, no network,
read-only root, dropped capabilities, no-new-privileges, unprivileged UID 65534,
and a 1 GB `/tmp` mount. Official full-seed1 data is mounted read-only from
`/home/student/speed-demon-2026/data/rust-validation/d/full-seed1`.
Each input is prewarmed before creating each measured container.

The external `measure.rs` wrapper measures child process wall time inside the
container, excluding Docker startup. `wait4` captures child maximum RSS in KiB.
This RSS measurement is **not** the competition's cgroup `memory.peak` and cannot
be substituted into its score formula.

Variants: previous serial binary, new binary with `--threads 1/2/4/6/12`.
One warmup and five measured runs each, rotating variant order each round.
Reported time discards minimum and maximum and averages the middle three;
all five measured times, median and spread are retained. Twelve workers
oversubscribe this VPS's six CPUs; it does not test twelve physical CPU slots.

The same pinned runtime restrictions also apply to the Rust regression tests
and Python-reference checker (the latter uses the already-cached pinned Python
3.13 image from `scripts/verify-baselines.py`).

## Reproduction

`experiment.py` expects a fresh directory containing `before/` and `track-d/`
Cargo projects, `tests/traverse-parser.rs`, `tests/traverse-check.py`, the
preserved `tests/reference/d_traverse.py`, `measure.rs`, and source metadata.
The script only writes within that directory and creates disposable containers.
It requires the existing official data/reference paths and cached pinned images.
Execute on the VPS as the normal user:

```sh
python3 -u /tmp/traverse-d7-h8ToLK/experiment.py /tmp/traverse-d7-h8ToLK
```

Use a new isolated directory for another experiment to preserve these results.
The full expanded commands are recorded in `console.txt`.

## Results

Pinned compiler: Rust 1.99.1, LLVM 23.1.1, Linux x86-64 (see `compiler.txt`).
Full seed 1 results below exclude the warmup; RSS is the maximum over the five
measured runs. Seconds are trimmed means, with medians in `summary.json`.

| Variant | Time, seconds | Speedup vs previous | Max child RSS, MiB | Time max/min spread |
|---|---:|---:|---:|---:|
| Previous serial | 12.9859 | 1.00x | 110.61 | 6.85% |
| D7, 1 worker | 13.1116 | 0.99x | 110.73 | 3.58% |
| D7, 2 workers | 7.3969 | 1.76x | 110.76 | 4.74% |
| D7, 4 workers | 4.6269 | 2.81x | 110.69 | 6.77% |
| D7, 6 workers | 3.7130 | 3.50x | 132.86 | 4.55% |
| D7, 12 workers | 3.7406 | 3.47x | 216.91 | 5.44% |

All 36 full-dataset outputs match the saved Python reference byte-for-byte:
`a23dbdf0a3e49cc8901310a9ee1dc6a9a569a85297b6da75c2a7002cb2ed277e`.
Seven Rust tests pass (`regression-tests.txt`), including parallel ordering and
state reuse. The Python checker passes all 204 cases under default/1/2/4/6/12
workers (1,224 comparisons), eight invalid inputs and three invalid thread flags
(`python-checks.txt`). The default uses six workers on this VPS.

Conclusion: D7 improves full-seed1 time by 3.50x at six workers (71.41% less time).
The 12-worker variant shows no measured benefit on this six-CPU VM and uses about
63% more peak process RSS than six workers. Keep the available-parallelism default;
the competition's six-vs-twelve choice still requires its actual hardware and
cgroup peak-memory metric. Some run spreads exceed 5%, so these are shared-VPS
measurements, not a stable board result or a certified score. No board run ID.

`runs.jsonl` contains every measurement, `summary.json` the aggregate and source
hashes, and `console.txt` the exact commands and complete run log. Candidate and
baseline source/Cargo/test hashes are also in `source-metadata.json`.
