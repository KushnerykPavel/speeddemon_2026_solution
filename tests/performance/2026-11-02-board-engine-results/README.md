# Track D bucket-ring engine (`9d69ece`): board resample

The same binary is sampled by record-only pushes. Baseline: 21 board runs of the `21df4fd` code,
T 0.2069 ± 0.0069 s, M 79.62 ± 0.21 MB, 64.95 ± 0.35 points. Acceptance rule (set before pushing): the mean T of
the candidate samples must be at least 2 standard errors below the baseline mean.

| Run | Commit | Run ID | T (s) | M (MB) | S | Points |
|---|---|---|---:|---:|---:|---:|
| 1 | `9d69ece` | run-b907b8e94554 | 0.180510 | 81.920 | 797 | 65.989013 |
| 2 | `b56aebf` (Track D record-only; Track C commits on top) | run-d682888f6e1c | 0.172689 | 81.002 | 797 | 66.518843 |
| 3 | `e6c1701` (record-only) | run-366f80786fd3 | 0.160719 | 82.276 | 797 | 67.231013 |
| 4 | `7098fb8` (Track C change only) | run-2ac7dc8f59d4 | 0.170100 | 82.272 | 797 | 66.615302 |
| 5 | `d989a96` (documentation only) | run-0816beddea18 | 0.215697 | 81.965 | 797 | 64.053054 |

After two samples the mean T is 0.1766 s, 14.6% below the baseline mean. Using the baseline SD (0.0069 s), the
standard error of a 2-sample mean is 0.0049 s, so the difference is about 6 SE. **Accepted on the board.** Memory
is 81.0–81.9 MB against the baseline 79.6, and S is 797 against 723 (board count). Both costs are smaller than the
time gain: points 65.99 and 66.52 against a baseline of 64.95 ± 0.35.

Third sample: mean of three 0.1713 s, 17.2% below the baseline mean (about 7 SE with n = 3).
Fourth sample: mean of four 0.1710 s (66.59 points on average), 17.3% below the baseline mean.
Fifth sample: 0.2157 s, the slowest of the five. The Track C sample from the same push (`run-7018018f489b`) was
also about 20% slower than the earlier sample of the same code, so this is most likely runner noise. Mean of
five 0.1799 s (66.08 points on average), 13.0% below the baseline mean.
