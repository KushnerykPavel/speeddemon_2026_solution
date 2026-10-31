# Track D: packed 32-bit edges with an exact heavy-weight list

Baseline: `853ae88` (`before-main.rs`, `(u32, u32)` edges). Candidate:
`packed-main.rs` (scored source). `profile-*-main.rs` add stderr phase timers only.

## Change

Every edge of the loaded, renumbered and reverse graphs is one `u32`: the other
endpoint in the low `head_bits` (bits needed for `n - 1`, 21 for the public
graph) and the weight above it. A weight that does not fit below the all-ones
escape code (2,047 for 21-bit heads) is stored as the escape code, and its exact
value goes into a `(edge index, weight)` list sorted after each parallel build;
reading an escaped edge binary-searches that list. Graphs needing 32 head bits
(over 2^31 vertices, which cannot fit in 8 GB anyway) are rejected. On full seed
1, 3,496 of 5,000,000 weights are ≥ 2,047. Correct for any input; nothing depends
on the generator's weight range.

## Protocol

`my-vps`, six Haswell KVM vCPUs, CPU set 0-5, 8 GB cgroup, pinned Rust image,
offline native release builds. Full official seed 1, prewarmed inputs, one
warmup plus five rotating-order runs, trimmed mean of the middle three, cgroup
v2 `memory.peak` via `measure.rs`. Phase profile: four runs per variant and
worker count, medians. All outputs byte-equal to the saved Python reference.

| Variant-workers | T (s) | Spread | cgroup peak (MB) |
|---|---:|---:|---:|
| `853ae88`-6 | 0.6011 | 9.8% | 121.4 |
| packed-6 | 0.5956 | 18.5% | 96.5 |
| `853ae88`-12 | 0.6014 | 27.6% | 136.2 |
| packed-12 | 0.5735 | 12.5% | 101.4 |

| Phase median (s) | `853ae88` 6w | packed 6w | `853ae88` 12w | packed 12w |
|---|---:|---:|---:|---:|
| graph load | 0.180 | 0.160 | 0.149 | 0.156 |
| renumbering | 0.110 | 0.113 | 0.113 | 0.120 |
| search (incl. transpose) | 0.308 | 0.311 | 0.315 | 0.322 |

Memory falls 20.5% (6 workers) and 25.6% (12); time changes are within run
spread (10–28%). Peaks at 6 and 12 workers now differ by only ~5 MB, so the peak
has likely moved out of the search phase.

## Correctness

Fifteen Rust tests (`measured-packed-tests.rs`) pass on macOS and VPS. New: head
widths for 0, 1, 2, 3, 2^21 and 2^21 + 1 vertices, escape codes and heavy-list
entries at `escape - 1`, `escape` and `u32::MAX`, rejection above 2^31 vertices.
The loader test now gives half its 25,000 edges weights spread over all of `u32`
so the heavy list is exercised in forward and reverse graphs with 1–12 workers.
Mutation caught: decoding escaped edges without the heavy list (6 tests fail).
`traverse-check.py`: 204 cases byte-equal to Python under default and
1/2/4/6/12 workers. Release Clippy and rustfmt pass.

## Where the peak is now

`memory-phases/` (instrumented copy, same protocol as the step-13 memory
profile; raw `rows.json`) shows the packed build peaks while loading: bucketed
triples 69.2 / 72.6 MB after parsing, 96.2 / 101.2 MB once the packed CSR is
placed (6 / 12 workers). Renumbering reaches 92.5 / 94.1 MB and search ends at
78.5 / 93.7 MB. The 12-byte bucket triples are the next memory target.
