# Track D: renumbering disabled after bidirectional search (rejected)

Question (open since step 12): does the BFS renumbering (~0.11 s of ~0.475 s on the VPS) still pay now that the
search is bidirectional with packed edges and paged distances? Step 9's comparison used forward-only search.

Baseline `before-main.rs` (sha256 `459900905ca4…`, `539b15f`, functionally `21df4fd`). Candidate
`noreorder-main.rs`: the only change is `if true || queries.is_empty() || nodes >= 1 << 27` in `reorder_graph`
(returns the graph unchanged; queries untouched). Same VPS protocol as `2026-11-01-traverse-mu-prune`
(CPU set 0-5, pinned Rust image, native offline build, full seed 1, prewarm, nine alternating rounds, round 0
discarded, trimmed mean of eight), plus a one-thread arm. Outputs equal Python/baseline on full seed 1 and all ten
datasets; 204 Python cases pass for both binaries.

| Workers | baseline (s) | no reorder (s) | change | faster rounds | cgroup peak (MB) base / no-reorder |
|---|---:|---:|---:|---:|---|
| 12 | 0.469 | 1.091 | +132.6% | 0/8 | 78.7 / 229.5 |
| 6 | 0.476 | 1.067 | +124.1% | 0/8 | 78.3 / 143.3 |
| 1 | 2.253 | 5.172 | +129.5% | 0/8 | 70.3 / 70.3 |

Search user CPU rises 2.08 → 5.75 s (12 workers): random IDs make each settle a cache miss and spread distance
pages across the graph (peak +150 MB at 12 workers). Renumbering saves far more than its 0.11 s.

**Decision: rejected; renumbering stays.** Closes the open question; no prior of success remains for variants
that drop the locality order. Cheaper orderings (not the order itself) remain the only direction here.
