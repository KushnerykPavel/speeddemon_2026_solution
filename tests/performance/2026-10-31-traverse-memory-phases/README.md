# Track D: where the cgroup peak occurs (b938156)

Instrumented copy of `b938156` (`memphase-main.rs`) prints cgroup v2
`memory.current` and `memory.peak` to stderr at phase boundaries. `my-vps`,
CPU set 0-5, 8 GB cgroup per run (`measure.rs`), full seed 1, prewarmed input,
three runs each at 6 and 12 workers. All outputs match Python. Raw: `rows.json`.

| Mark (round 0) | 6 workers current / peak (MB) | 12 workers current / peak (MB) |
|---|---:|---:|
| graph loaded | 56.3 / 56.3 | 56.1 / 56.3 |
| renumbering regions built | 110.0 / 110.0 | 110.3 / 110.3 |
| old graph dropped | 62.8 / 110.0 | 63.4 / 110.3 |
| renumbered | 57.0 / 110.2 | 57.8 / 110.3 |
| reverse graph built | 110.7 / 110.7 | 111.8 / 111.8 |
| queries answered | 73.7 / 127.7 | 85.7 / 139.5 |

Final peaks over three rounds: 125.8–127.8 MB (6 workers), 139.5–143.1 MB (12).
The peak occurs during search: about 112 MB of forward plus reverse graph and
about 17 MB (6 workers) / 28–31 MB (12) of search state. Renumbering peaks at
~110 MB. Graph-resident memory is therefore the main memory lever (packed
edges), followed by per-worker search state; a single-pass parser's transient
buffer must stay below the search-phase peak.
