# Track C: performance log

Every step from the reference to your final version, each with the live-board run ID that measured it. Judges check that the run IDs exist and that each change moved the way you say it did.

| Step | Change | Run ID | T (s) | M (MB) | Points |
|---|---|---|---|---|---|
| 0 | The reference, unchanged | | 14.44 | 367 | 0 |
| 1 | Rust: mmap, parallel chunked parsing, per-thread aggregates and name tables, bitset merge (`10cd331`) | run-9f6461f49b69 | 0.464127 | 98.603008 | 39.531957 |
| 2 | One shared lock-free name table and per-group atomic bitsets; 4 KB pages (`prctl`) (`f04f17a`) | run-8e1b33711a41 | 0.109347 | 22.814720 | 60.968410 |
| 3 | Dense name ids (half-size bitsets); the arena compares 8-byte words | pending | | | |

## Step 1 — first Rust version, 2026-11-01

VPS (6 Haswell vCPUs, Docker with the board's flags, sd-timer, cgroup `memory.peak`, full seed 1, 7 runs):
T 0.632 s trimmed mean, M 67.75 MB, output SHA-256 `d8088b1cd42b…` (the published full-seed-1 hash).
Local Mac phase timings at 12 threads: regions 26–44 ms, parse 110 ms, merge interning 42–46 ms.
Board: `run-9f6461f49b69`, 0.464 s, 98.6 MB, S 317, 39.53 points. The board's memory is higher than the VPS
cgroup peak (67.75 MB), as on Track D before step 15 there: the box backs memory with 2 MB transparent huge pages.

## Step 2 — shared name table, 2026-11-01

The per-thread name tables cost 70 of the 110 ms of parsing at 12 threads (Mac), plus a 45 ms serial merge.
Now all threads share one lock-free open-addressing table (2^18 slots):
- a slot is claimed by swapping its tag from 0 to 1, the raw quoted bytes are copied into an append-only arena,
  and the tag (a hash with the top bit set) is published with release ordering;
- a lookup compares the full bytes, so the table is exact;
- the raw form is one-to-one with the unescaped value, so nothing is unescaped;
- a name's id is its slot, and each record ORs one bit into its group's bitset, so there is no merge.

If more than half the slots fill, which needs over 131,000 distinct names (the generator uses about
100,000), a slow exact pass with one hash set per group counts the names instead. Country codes map through
a 26³ table. `prctl(PR_SET_THP_DISABLE)` and `mallopt` as on Track D.

VPS, same protocol: T 0.632 → 0.274 s, M 67.75 → 21.46 MB, full-seed-1 hash `d8088b1cd42b…`; sample seed 1
0.087 s, 20.88 MB, hash `9035fed36823…`. Locally, output equals the reference on 11 datasets (tiny/sample
seeds 1–3, full seeds 1–2, worked example, edge-case rules) at 1/2/3/5/8/12 threads.
Board: `run-8e1b33711a41`, 0.109 s, 22.8 MB, S 383, 60.97 points (+21.4).

## Step 3 — dense name ids, 2026-11-01

Mac isolation at 12 threads (parse phase): about 31 ms of field parsing, 30 ms of table lookups and 25 ms
of `fetch_or` on the shared group bitsets. Two fixes were measured on the VPS, alternating, 9 runs each,
two rounds, full seed 1, all outputs `d8088b1cd42b…`:

| Variant | T (s) | M (MB) |
|---|---|---:|
| step 2 | 0.284 / 0.283 | 21.7 / 21.3 |
| per-thread id lists merged by group (rejected) | 0.260 / 0.271 | 33.4 / 33.1 |
| **dense ids, half-size atomic bitsets (kept)** | 0.288 / 0.273 | 17.9 / 17.9 |

The per-thread lists save about 6% of the time but add 12 MB (about 4 bytes per record), which costs more
points than it gains. Dense ids (insertion numbers below 2^17, packed into the slot's span) halve the
bitsets (241 × 16 KB). The arena now stores 8-byte words, compared a word at a time. Time is unchanged,
memory −17%. Same 11-dataset local check at 1/3/12 threads.
