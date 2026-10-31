# Track D: single-pass bucketed graph build

Baseline: `b938156` (`before-main.rs`, two parallel parse passes with atomic
degree counters and atomic cursor scatter). Candidate: `bucketed-main.rs`
(scored source). `profile-*-main.rs` add stderr phase timers only.

## Change

Each chunk worker parses its newline-aligned slice once and pushes every
`(u, v, w)` into a bucket for the part owning `u`; a part is a power-of-two
block of vertex IDs (at least 1,024; about eight parts per worker), so the
bucket index is a shift. After a prefix over part sizes, each worker takes a
contiguous run of parts and, per part, counts row lengths, prefixes them and
places the part's edges with plain writes into its own `split_at_mut` slices of
`offsets` and `edges`. A part's region is a few hundred KB, so counting and
placement stay in cache; no atomics or unsafe remain in loading. Edge order in a
row is now deterministic (chunk order, then line order). Idea credit: Team7's
public Track D build (MIT); implementation is ours.

## Protocol

`my-vps`, six Haswell KVM vCPUs, CPU set 0-5, 8 GB cgroup, pinned Rust image,
offline native release builds. Full official seed 1, prewarmed inputs, one
warmup plus five rotating-order runs, trimmed mean of the middle three, cgroup
v2 `memory.peak` via `measure.rs`. Phase profile: four runs per variant and
worker count, medians. All outputs byte-equal to the saved Python reference.
Run spreads were 17–22% (shared host); the time change exceeds them.

| Variant-workers | T (s) | Spread | cgroup peak (MB) |
|---|---:|---:|---:|
| `b938156`-6 | 0.7903 | 20.9% | 128.1 |
| bucketed-6 | 0.5793 | 19.9% | 122.0 |
| `b938156`-12 | 0.8050 | 17.3% | 144.0 |
| bucketed-12 | 0.5959 | 21.8% | 136.0 |

| Phase median (s) | `b938156` 6w | bucketed 6w | `b938156` 12w | bucketed 12w |
|---|---:|---:|---:|---:|
| graph load | 0.379 | 0.172 | 0.350 | 0.158 |
| renumbering | 0.108 | 0.113 | 0.093 | 0.112 |
| search (incl. transpose) | 0.289 | 0.303 | 0.312 | 0.328 |

## Correctness

Fourteen Rust tests (`measured-bucketed-tests.rs`) pass on macOS and VPS. The
loader contention test now uses 5,000 vertices in five parts (hot rows 0, 1,023
and 4,999 plus one edge per vertex) and checks forward and reverse row
multisets for 1/2/4/6/12 workers. Mutation caught: dropping the per-part edge
base (3 failing tests). `traverse-check.py`: 204 cases byte-equal to Python
under default and 1/2/4/6/12 workers; invalid inputs and thread options fail
cleanly. Release Clippy and rustfmt pass.

Raw rows: `runs.jsonl`, `profiles.jsonl`; summaries: `summary.json`,
`profile-summary.json`; console: `console.txt`; hashes: `source-sha256.txt`.
