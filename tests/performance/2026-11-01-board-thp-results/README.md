# Track D huge-page candidate (`a86b62a`): board resample

Same binary sampled by record-only pushes. Baseline: 21 board runs of the `21df4fd` code,
T 0.2069 ± 0.0069 s, M 79.62 ± 0.21 MB.

| Run | Commit | T (s) | M (MB) | S | Points |
|---|---|---:|---:|---:|---:|
| 1 | `a86b62a` | 0.196730 | 79.540 | 749 | 65.385135 |
| 2 | `d6900d5` (record-only) | 0.230308 | 81.027 | 749 | 63.593741 |
| 3 | `63124aa` (record-only), `run-6c84f8ae31f3` | 0.216382 | 79.434 | 749 | 64.357190 |

Mean T of the three samples: 0.2145 s, +3.7% against the baseline mean. Sample 2's memory is 1.4 MB above the
baseline range. The VPS gain (−4.7%) did not transfer. Rejected and reverted (the scored code is the `21df4fd` code
again). Run IDs of samples 1 and 2 are not in the compact public history.
