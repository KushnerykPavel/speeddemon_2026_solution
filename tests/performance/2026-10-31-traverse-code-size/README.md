# Track D: code-size pass

Baseline: `3b648eb` (`before-main.rs`, 866 code lines). Candidate:
`slim-main.rs` (713 code lines; S counts these plus 9 Dockerfile lines).
`measured-slim-main.rs` is the build timed on the VPS; the scored file differs
only in building the renumbered offsets with a plain push loop and a reserved
`Vec` instead of `extend(scan(..))` (same values, no reallocation).

Code lines: `scc` code lines after default `rustfmt`, approximated locally as
non-blank lines that are not `//` comments (866 + 9 = 875 matched the board's
S for `3b648eb`).

## Changes (behaviour unchanged)

- `MappedFile` with `Drop`/`munmap` and a null check → `map_file`, which keeps
  the read-only mapping for the rest of the run.
- Queries: `BufReader` + `read_record` → the mapped file split with
  `split_inclusive('\n')` and the same strict `parse_fields`; `parse_fields`
  rewritten as whitespace split + all-digits check + `u32` parse (same
  accept/reject set, same error kind).
- Search: the `workers == 1` serial path and the `Searcher` struct removed;
  `Search<u32>::shortest_path` takes the shared `u64` search and both graphs.
- Renumbering: degrees are noted during the breadth-first pass, so the separate
  parallel degree pass is gone; offsets are a prefix sum over the regions.
- Smaller items: one `invalid()` helper, `Relaxed` imported, `lock().unwrap()`
  (poisoning cannot occur with `panic = "abort"`), `Default` instead of
  `Distance::ZERO`, compact `line_chunks`, argument parsing and output.

## Protocol

`my-vps`, six Haswell KVM vCPUs, CPU set 0-5, 8 GB cgroup, pinned Rust image,
offline native release builds. Full official seed 1, prewarmed inputs, one
warmup plus five rotating-order runs, trimmed mean of the middle three, cgroup
v2 `memory.peak`. Phase profile: six runs per variant and worker count, medians.
All outputs byte-equal to Python.

| Variant-workers | T (s) | Spread | cgroup peak (MB) |
|---|---:|---:|---:|
| `3b648eb`-6 | 0.4847 | 13.3% | 77.9 |
| slim-6 | 0.4561 | 13.6% | 78.3 |
| `3b648eb`-12 | 0.4816 | 12.2% | 79.0 |
| slim-12 | 0.4681 | 9.6% | 78.6 |

| Phase median (s) | `3b648eb` 6w | slim 6w | `3b648eb` 12w | slim 12w |
|---|---:|---:|---:|---:|
| graph load | 0.113 | 0.122 | 0.102 | 0.104 |
| renumbering | 0.127 | 0.117 | 0.114 | 0.122 |
| search (incl. transpose) | 0.249 | 0.262 | 0.246 | 0.260 |

End-to-end time −5.9% / −2.8%, memory within ±0.5%. Phase medians move both
ways within the spread seen in earlier runs (search 0.248–0.268 s); no phase
shows a consistent loss.

## Correctness

Thirteen Rust tests (`tests/traverse-parser.rs`; the four `read_record` tests
were replaced by one line-parsing test, and malformed-field cases now go
through `parse_fields`, plus an empty line) and 204 Python cases under default
and 1/2/4/6/12 workers pass on macOS and the VPS (`regression-tests.txt`,
`python-checks.txt`).
