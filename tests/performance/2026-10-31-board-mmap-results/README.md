# Official mmap/parallel CSR board result

[Official feed](https://s3.amazonaws.com/raptors.dev/speeddemon.dev/2026/board/board.json),
generated 2026-10-31 16:22:57 UTC (17:22:57 Europe/Warsaw).
Snapshot: `board-result.json`, including team results and the points ranking used
to derive Track D position 66 of 84 scored teams.

Commit `e5bad19ffc9c7c18bb543758bde5a33a470bdd8c`, Track D:
`run-d07b2b8242d7`, completed 16:20:21 UTC. All five recorded gates passed,
including correctness, timeout and sandbox. This is the new team-best D result.

| Metric | Previous D7 (`b8e5bc1`) | Mmap/parallel (`e5bad19`) |
|---|---:|---:|
| T | 1.626762 s | 1.142983 s |
| M | 228.868096 MB | 228.335616 MB |
| S | 230 | 363 |
| D | 1 | 1 |
| K | 6.824792 | 7.606888 |
| Points | 41.704470 | 44.060351 |

Time is 29.74% lower (1.4233x), points rise 2.355881. Memory is approximately
unchanged; code-line growth reduces the score benefit. MB is decimal. These are
single board runs, not the final hidden-seed five-run evaluation. Previous D7
source: `../2026-10-31-board-results/board-team-snapshot.json`.

The same push reran unchanged Track A: `run-c0c10e9cc914`, 0.901183 s,
35.209216 MB, 43.029948 points, all gates passed. Its best remains
0.876075 s and 43.339268 points at `531def7`.
