# Track D D12: `u32` distances with exact `u64` fallback

Baseline: `accbb64` (BFS reorder, `u64` distances), `before-main.rs`.
Candidate: `u32-main.rs` (scored source). Rejected intermediate: `u32-local-main.rs`.

## Change

Each worker searches with `u32` distances (7 MB per worker instead of 14 MB) and
8-byte heap entries. A relaxation whose sum does not fit below `u32::MAX` (the
unvisited sentinel) is skipped and marks the search as overflowed. A distance
found by the narrow search is exact: every prefix of a shorter path is also
below that distance, so no needed relaxation was skipped. If the target is not
found and an overflow occurred, the query reruns in a `u64` search.

The first attempt (`u32-local`) gave every worker its own lazily allocated
`u64` search. On full seed 1, 4 of 2,000 answers exceed `u32::MAX - 1`; those
queries, plus unreachable queries whose narrow pass overflowed, retry in `u64`.
Net peak changes (+42 MB at six workers despite −42 MB of narrow arrays; +12.7 MB
at twelve despite −84 MB) imply about six and seven 14 MB fallback arrays, so
cgroup peak **rose**. The fallback count was not instrumented directly.
The candidate shares one lock-protected `u64` search across workers (one array).

## Protocol

`my-vps`: six Haswell KVM vCPUs, Linux x86-64, CPU set 0-5, 8 GB cgroup, pinned
Rust 1.99.1 image, offline native release builds (`compiler.txt`, `cpu.txt`).
Full official seed 1, inputs prewarmed outside the cgroup. Per variant one
warmup and five rotating-order runs; trimmed mean of the middle three. Memory is
cgroup v2 `memory.peak` from `measure.rs`. All 36 outputs match the saved Python
reference (SHA-256 `a23dbdf0…277e`). VPS results, not board measurements.

| Variant | Workers | T (s) | Spread | cgroup peak (MB) | ΔT | ΔM |
|---|---:|---:|---:|---:|---:|---:|
| `accbb64` | 6 | 1.6469 | 10.0% | 147.5 | — | — |
| per-worker `u64` fallback | 6 | 1.5104 | 5.3% | 189.4 | −8.3% | +28.4% |
| **shared `u64` fallback** | 6 | 1.5429 | 8.1% | **119.7** | −6.3% | **−18.8%** |
| `accbb64` | 12 | 1.6163 | 8.1% | 234.0 | — | — |
| per-worker `u64` fallback | 12 | 1.5662 | 4.6% | 246.7 | −3.1% | +5.4% |
| **shared `u64` fallback** | 12 | 1.5737 | 17.7% | **163.2** | −2.6% | **−30.3%** |

Time changes are within run spread; the memory reduction matches the predicted
−84 MB of `u32` arrays at twelve workers plus one shared 14 MB `u64` array.
The first run (`first-attempt-*`) measured only baseline vs `u32-local` and is
kept for provenance.

## Correctness

Twelve Rust regression tests (`measured-u32-tests.rs`) pass on macOS and the
VPS, including a new fallback test: answers `u32::MAX - 1`, `u32::MAX` and
`u32::MAX + 2`, a short path beside an overflowing direct edge, unreachable
targets after overflow, and state reset/reuse. `tests/traverse-check.py`: 204
cases byte-equal to Python under default and 1/2/4/6/12 workers, eight invalid
inputs and three invalid thread options fail cleanly. Release Clippy and
rustfmt pass.

## Reproduce

Stage `before/`, `u32-local/`, `u32/` (each `src/main.rs` + track `Cargo.toml`/
`Cargo.lock`), `measure.rs`, `tests/u32-tests.rs`, `tests/traverse-check.py`,
`tests/reference/d_traverse.py` and `experiment.py` into one directory, tar it, then:

```sh
ssh my-vps 'run_dir=$(mktemp -d /tmp/traverse-u32-XXXXXX) && tar -xf - -C "$run_dir" && python3 -u "$run_dir/experiment.py" "$run_dir"' < u32-vps.tar
```

Raw rows: `runs.jsonl`; summary: `summary.json`; console: `console.txt`;
source hashes: `source-sha256.txt`.
