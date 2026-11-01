# Track D step 23 (BFS blobs of 512) official board result

Team Kushneryk, Track D latest run for `cae88c9`:

| Run ID | Commit | Gates | T (s) | M (bytes) | S | D | Points |
|---|---|---|---:|---:|---:|---:|---:|
| run-81554f49a3d1 | `cae88c9` | all passed | 0.198285 | 79,855,616 | 736 | 1 | 65.339497 |

Versus the step 20 samples of the previous code (0.211929 s and 0.203360 s,
64.69 and 65.16 points): time −2.5% against the faster sample, S +13
(736 vs 723). The gain is inside the ±4% spread of single board runs, so a
second sample (record-only commit) decides it. Raw record: `board-result.json`.

## Re-sample (`61d9b2a`, record-only commit, same binary)

| Run ID | Commit | T (s) | M (bytes) | S | Points |
|---|---|---:|---:|---:|---:|
| run-403e26bcde98 | `61d9b2a` | 0.222510 | 79,708,160 | 736 | 64.096032 |

Two samples of the blob build: 0.198 and 0.223 s (mean 0.210 s). Two samples of
the previous code (step 20): 0.212 and 0.203 s (mean 0.208 s). No measurable
gain from blobs, S is 13 lines higher, and single board runs of one binary
span at least 12%. Blobs are reverted. Raw record: `board-result-resample.json`.
