# Track D: degree-2 bypass and spur removal (E2, rejected)

Hypothesis (research/plateau-recovery.md, H1/E2): a two-way degree-2 vertex (exactly one light edge to and from
each of two distinct neighbours) is always crossed straight through, and a spur (one edge to and from one
neighbour) can only be left the way it came. Neither is needed between other vertices. Contracting chains to
one edge per direction (exact sum; chain edges below the packing escape, so sums fit `u32`) and dropping spurs,
while always keeping query endpoints, cuts search work. HighVelocity reported −13% wall time.

Census (`census.py`, full seed 1, `census-seed1.txt`): 397,682 two-way degree-2 vertices (22.8%) carrying
1,590,728 edge lines (31.8%), and 100,900 spur vertices. The implementation keeps 1,242,945 vertices.

Baseline `before-main.rs` (sha256 `459900905ca4…`, `539b15f`). Three variants, all byte-equal to the baseline on
local full seeds 1 and 2:

| Variant | Where it classifies and contracts | Mac M1 Pro, 12 threads (ms) | Why it loses |
|---|---|---|---|
| v1 `v1-old-ids-cas-main.rs` | before the BFS, in old IDs; in-sources recorded with a CAS per edge; chains followed in the BFS and in the copy | classify 35–46, renumber +20–25, search −10 to −17; total 201–211 vs 154–176 | random access to every head in the original ID order (≈30 ms for the in-edge pass alone) |
| v2 `v2-old-ids-add-main.rs` | as v1, with `fetch_add` of count, sum and sum of squares (exact names for two sources) | in-edge pass still 31–38 ms; total 197–204 vs 154–184 | same random access; not the atomic type |
| v3 `v3-contract-main.rs` | after renumbering, a separate contraction pass over the local graph that keeps vertex order | contract 27–38 (count 8, kinds 3, ids 1.5, degrees 8, write 9), search −10 to −15, transpose −3; total 169–194 vs 147–163 | contraction costs about twice what the search saves |

v3 on the VPS (`vps_phases.py`, `vps/summary.json`; Docker CPU set 0-5, 12 workers, nine alternating rounds,
round 0 discarded, phase medians from diagnostic builds; every run equal to the Python output):

| | total (s) | renumber | contract | transpose | search | peak MB |
|---|---:|---:|---:|---:|---:|---:|
| before | 0.486 | 120 | — | 59 | 198 | 78.75 |
| v3 | 0.568 (+17%) | 105 | 152 | 50 | 157 (−21%) | 78.63 |

## Decision
Rejected. The work reduction is real (search −21% on the VPS, −15 to −25% on the Mac), but finding and following
chains costs more than it saves in every arrangement tried. Contraction in old IDs pays a random access per edge.
Contraction after renumbering adds a full extra pass with chain walks. A version cheap enough to win would have to
classify inside an existing per-edge pass. That needs in-edge information the forward-only BFS does not have, so it
would mean a different pipeline (for example, both directions built during load). That is outside the remaining
time. No unit-test or Python-suite run was made for the rejected variants beyond the full-seed equality checks.
