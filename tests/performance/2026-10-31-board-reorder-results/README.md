# Official D8 board result

[Official feed](https://s3.amazonaws.com/raptors.dev/speeddemon.dev/2026/board/board.json),
generated 2026-10-31 18:03:56 UTC (19:03:56 Europe/Warsaw).
`board-result.json` preserves the team and points-sorted Track D ranking.
Derived team position: 64 of 88 scored teams. Leader: Cause Of Death,
73.008704 points, 0.121536 s, 68.067328 MB.

Commit `accbb6463356120662e91fc474f6dc8f0acfbf9e`, Track D:
`run-c18ebeae7015`, completed 18:02:46 UTC. All five recorded gates passed.
This is the new best team D result.

| Metric | Mmap/parallel (`e5bad19`) | BFS/parallel rebuild (`accbb64`) |
|---|---:|---:|
| T | 1.142983 s | 0.637161 s |
| M | 228.335616 MB | 234.110976 MB |
| S | 363 | 437 |
| D | 1 | 1 |
| K | 7.606888 | 9.859309 |
| Points | 44.060351 | 49.692323 |

Time is 44.25% lower (1.7939x), points rise 5.631972. Memory increases 2.53%.
MB is decimal. This is a single live-board run, not the final holdout evaluation.
The previous result is preserved in `../2026-10-31-board-mmap-results/board-result.json`.

Unchanged Track A rerun also passed: `run-e0a5164e734a`, 0.907034 s,
35.139584 MB, 42.968281 points. A's best remains 0.876075 s and 43.339268 points.
