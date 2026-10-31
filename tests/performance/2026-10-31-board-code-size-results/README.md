# Track D step 20 (code-size pass) official board result

[Official feed](https://s3.amazonaws.com/raptors.dev/speeddemon.dev/2026/board/board.json),
generated 2026-10-31 21:19:47 UTC. Team Kushneryk, Track D latest run:

| Run ID | Commit | Gates | T (s) | M (bytes) | S | D | K | Points |
|---|---|---|---:|---:|---:|---:|---:|---:|
| run-49484fa644ae | `1851b13` | all passed | 0.211929 | 79,613,952 | 722 | 1 | 19.672249 | 64.692701 |

Versus step 19 (`run-b507a889127d`, 0.195022 s, 79,945,728 B, S=875, 64.951278
points, still the team best): time +8.67%, memory −0.41%, S 875 → 722
(+0.63 points by itself), **−0.258577 points** overall. The VPS A/B showed the
slim build 2.8–5.9% faster, so the board time is either single-run noise
(steps 17–20 ranged 0.195–0.212 s) or a board-only effect. Rank 25 of 91 by
best points (tenth: 68.46). Raw record: `board-result.json`.

## Re-sample: same track code, record-only commit

| Run ID | Commit | Gates | T (s) | M (bytes) | S | D | Points |
|---|---|---|---:|---:|---:|---:|---:|
| run-abe73b97cc7d | `2012f4e` | all passed | 0.203360 | 79,261,696 | 722 | 1 | 65.160080 |

`2012f4e` changes only `PERF.md` and this folder, so the board built the same
binary as `1851b13`. The second sample is 4.0% faster than the first, which
puts the slim build at a new team best (+0.208802 points over step 19) and
shows that single board runs of this code vary by about ±4% in time. Raw
record: `board-result-resample.json`.
