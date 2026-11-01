# Track C step 5: 64-byte block parser, batched lookups with prefetch, one-byte region table

Baseline `c1299a8` (step 4, labelled c5 in the raw files). Candidates, each on top of the previous:
- c6: block parser (quote/comma/LF bit masks per 64 bytes, prefix XOR for the inside of quotes) and
  name-table lookups in batches of 32 with prefetched slots and bitset words;
- c7: group aggregates moved into the batch flush, with the region-to-group entry prefetched;
- c8 (committed): the region-to-group table holds one byte per region when the groups fit.

Board-like runs (`tools/bench-c.sh`: Docker with the board's flags, sd-timer wall time, cgroup
`memory.peak`, full seed 1, 9 runs per variant, trimmed mean, alternating order):

| File | Result |
|---|---|
| `vps-ab-c6-vs-step4.txt` | c6 0.2275 / 0.2573 s vs step 4 0.2616 / 0.2596 s: inconclusive |
| `vps-ab-c7-vs-step4.txt` | c7 0.2455 / 0.2373 s vs step 4 0.2911 / 0.2729 s: −16% / −13% |
| `vps-ab-c8-vs-c7.txt` | c8 0.2492 / 0.2420 s, 16.8 MB vs c7 0.2507 / 0.2492 s, 17.7 MB |

Parse-phase timings (Instant around the worker threads, printed to stderr, Docker, `--threads N`,
12 alternating runs, medians): step 4 1420 ms (1 thread) / 246 ms (6 threads), c6 1110 / 245,
c7 986 / 221. `vps-parse-phase-c5-c6-c6haswell.txt` has the 15-run 6-thread comparison including a
`target-cpu=haswell` build of c6 (250 ms median, within noise; not adopted).

Single-thread component costs of c6 on the VPS (switched off one at a time, 5 runs, medians):
parse, number conversion and aggregates ≈ 444 ms; region-to-group lookup ≈ 120 ms; name hash ≈ 45 ms;
name table and bitsets ≈ 440 ms.

Correctness: output identical to the reference on tiny seeds 1–3, sample seeds 1–3, full seeds 1–2,
the worked example and the edge-case rules file at 1/2/3/5/8/12 threads; the two-byte region table
path (forced) also matches at 1/3/12 threads.
