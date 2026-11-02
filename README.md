# Speed Demon 2026: team Kushneryk

Team Kushneryk's entry for [Speed Demon 2026](https://speeddemon.dev), run by Hackathon Raptors. Three
tracks are entered, all in Rust with no dependencies. Track B is the unchanged starter and is not run by
the board.

| Track | Approach in one line | Board best | T (s) | M (MB) | S | Points |
|---|---|---|---:|---:|---:|---:|
| [A: Ingest](track-a/README.md) | mmap, AVX2 line scan, parallel parse into per-thread tables | `run-b6e6f7905600` | 0.0293 | 20.4 | 516 | 79.23 |
| [C: Join](track-c/README.md) | 64-byte block CSV parser, shared lock-free name fingerprints, per-country bitsets | `run-11c93a57a1e3` | 0.0830 | 10.8 | 462 | 66.59 |
| [D: Traverse](track-d/README.md) | parallel CSR with BFS renumbering, bidirectional bucket-ring Dijkstra | `run-366f80786fd3` | 0.1607 | 82.3 | 797 | 67.23 |

Each track's `README.md` describes the approach, the dependencies and what we would do with another 24
hours. Each `PERF.md` logs every step with the board run that measured it, including rejected experiments.

## How we worked

- **Correctness first.** Every change is byte-compared with the Python reference on the published seeds
  and edge-case datasets at 1–12 threads before it is timed. Track D also has a random all-pairs test
  against an independent Dijkstra, with mutants that the test must catch.
- **Measured decisions.** Candidates are timed against the baseline on a 6-vCPU VPS in Docker with the
  board's flags, in alternating pairs, recording cgroup `memory.peak`. A single board run varies by about
  ±9% (21 runs of one binary, Track D PERF), so board results are resampled with record-only pushes
  before a gain is claimed.
- **Calibration against public code.** On 2 November we ran public competitor solutions with known board
  times next to ours on the same VPS (Track D PERF step 29, Track C PERF step 6). The VPS reproduced the
  board's time ratios, and for Track D a phase-by-phase comparison found where our time went. Ideas taken from public
  solutions are credited in the track READMEs, and the code is our own.
- **Evidence.** Raw measurements, scripts and test sources are in `tests/performance/`, and research
  notes are in `research/` and `docs/`.

## Reproduce

```
cd track-d && cargo build --release --offline --locked
target/release/traverse-baseline --data /data --out /out
```

Tracks A and C build the same way; their binaries are `ingest-baseline` and `join`. `check.py` builds
a track with the board's Docker settings and prints the output's sha256.

## Licence

The reference code is copyright Hackathon Raptors CIC, under the MIT licence in `LICENSE`. Your repo has to be public under an OSI-approved licence at code freeze; keeping MIT is the simplest way.
