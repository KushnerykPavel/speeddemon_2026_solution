# Track D: transpose experiments (both rejected)

Baseline: `21df4fd`. The search-tail profile
([search tail](../2026-11-01-traverse-search-tail/README.md)) showed transpose
at 51–62 ms, about 11% of a VPS run.

## Sub-phase timing (`edgescan/profile-*-main.rs`, medians, ms)

| | count in-degrees | + prefix and allocation | total |
|---|---:|---:|---:|
| `21df4fd` 6w | 14.3 | 30.5 | 56.4 |
| `21df4fd` 12w | 14.7 | 30.6 | 55.8 |
| edgescan 6w | 11.6 | 28.7 | 55.9 |
| edgescan 12w | 12.8 | 28.8 | 56.8 |

The rest of each total is the scatter.

`page-fault-bench.rs` (same container limits, 4 KB pages): first touch of a
fresh 28 MB buffer costs about 20 ms on one thread and 6 ms on six; touching
it again costs 0.3 ms.

## Variants

| Variant | Change | 6w T / peak | 12w T / peak |
|---|---|---|---|
| edgescan | count in-degrees from the packed edge array instead of decoded rows | +1.0% / −0.2% | +2.9% / −0.5% |
| recycle | count in-degrees with atomics in the renumbering copy; transpose overwrites the old graph's buffers; the counters become the cursors | +2.1% / +2.4% | +7.6% / +8.2% (85.0 MB) |

In recycle, renumbering got 31 ms slower at 12 workers, because the atomic
increments contend inside the copy, and the search phase including transpose
did not get shorter. Phase marks (`recycle/rows.json`) moved the peak to the
end of the copy, 83.6–84.0 MB at 12 workers, because the old graph and the
counters stay alive.

Neither variant is adopted. Correctness held for both: thirteen Rust tests
plus a recycled-vs-fresh transpose check, and 204 Python cases. Removing the
degree increment failed two tests.
