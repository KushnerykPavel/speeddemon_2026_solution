# Track A: public solutions found, 2026-11-01

Method: board feed (`board.json`, 2026-11-01 09:36 UTC) gives each team's best Track A commit;
`api.github.com/search/commits?q=hash:<sha>` finds public repos holding it (23 of the top 60
checked before the unauthenticated rate limit slowed the sweep); a repository-name search found
12 more `speed-demon*` repos. Private repos stay private until freeze (Nov 2 15:00 UTC).

| Team | Board A points | T (s) | M (MB) | S | Repo | Language | Licence |
|---|---:|---:|---:|---:|---|---|---|
| HighVelocity | 84.93 | 0.0164 | 21.3 | 584 | insightlabs38-pixel/sd-sm | C++ | MIT (template) |
| Cache Me | 84.30 | 0.0215 | 18.9 | 334 | manusingh090/speed-demon-2026-entry | C++ | MIT (template) |
| Us (best) | 74.00 | 0.0398 | 30.4 | 544 | this repo | Rust | MIT |

The template's MIT licence covers both; we borrow techniques and credit them, not code.
The other top-10 teams' commits were not found in public search (private or unindexed).

## Cache Me: what the architecture does

Source: its `track-a/README.md`, `main.cpp` and a 23-step `PERF.md` with run IDs (the most useful
document found: it contains the board measurements of nearly every idea below).

1. One C++20 file, `-O3 -march=native -static`, 12 threads, 8 line-aligned pieces per thread
   claimed in order (16 or 4 per thread measured slower on the board).
2. Fixed layout: hour index from month/day/hour digits; one 64-byte AVX2 compare after the
   timestamp finds the method end, endpoint end and line end; latency decoded without a loop
   (`maddubs`/`madd`).
3. **One lock-free endpoint table shared by all threads**: no per-thread tables, no merge, no
   remap. Key: CRC32 of the name's first and last eight bytes; names of up to 64 bytes confirmed
   with two AVX2 compares. A name's id is its slot.
4. **4-byte records** (latency above a 13-bit key: 12-bit endpoint, 5xx flag) filed per hour in
   16 KB chunks; latencies of 2^19 or more (about 3%) go on a side list.
5. Ranks of endpoint names computed once; each hour (about 43k records) is counting-sorted by
   rank in an L2-sized scratch array; three branch-free selections per group; chunks are released
   with `madvise(MADV_DONTNEED)` as hours finish, so records and output pages overlap in memory
   instead of adding up.
6. Output: per-group numbers (24 bytes) with exact text sizes, one barrier, `ftruncate` and
   `mmap` the output, every thread formats its hours straight into the file (a pwrite design
   queued on the file lock: 24 to 60 ms of CPU waiting).
7. Each parsed input piece is dropped with `madvise(MADV_DONTNEED)` by its parser: board
   0.0263 → 0.0215 s (-18%) in their design.

Measured on the board by them (their PERF.md): shared table -17% time; page release -18%;
prefetching the log ahead +10% slower; populate (`MAP_POPULATE`, `MADV_POPULATE_READ`) slower;
radix sort per hour not faster; a Rust port of the same design scored 0.0323 s against 0.0301 s
for the C++ and counted 341 lines against 271 (rustfmt's layout), so they went back to C++.
Their best 84.30 was a single run; unchanged-design reruns scored 81.2 to 82.7.

## HighVelocity

Public C++ repo (`sd-sm`, 609 file lines, S 584, T 0.0164 s, M 21.3 MB, the fastest of the three).
Its log (`PERF.md`) shows early steps only (parallel parse workers, CRC endpoint fingerprint on
sampled bytes, flat table); the final design was not read.

## What we'd change (mapped to our gaps)

| Gap | Ours | Cache Me | Change |
|---|---|---|---|
| T 0.040 vs 0.022 | per-thread tables, merge, remap; 442 instructions per record | shared table, CRC32 of 3 words, AVX2 | shared lock-free table (Rust atomics) and a cheaper hash |
| M 30 vs 19 | 4 B + run tables, but about 12 MB of transient 8-byte keys and arrays | 4 B records written directly, chunks freed as output grows | append 4-byte records per hour per thread; drop the 8-byte key pass |
| S 544 vs 334 | 536 rustfmt lines | 222 clang-format lines, 271 on the board | write the design in fewer, longer-lived pieces; their own Rust port did not beat their C++ on K |
| Output | streamed through a mutex, tmpfs page allocation serial | formatted straight into a mapped file | same, after sizes are known |

Adopted already: `MADV_DONTNEED` for parsed input (board 0.0408 → 0.0398 s: much smaller than
the 18% they saw, because our other costs dominate); records freed before output (their idea 5,
ours measured 46.6 → 28.4 MB).

## Recommendation

Their design is the template to follow for the remaining time; in Rust it would be a rewrite of
`State`/`chunk`/`summarize` (not a patch). Their own data says the Rust version of this exact
design lands near 0.032 s and 341 lines, i.e. about 80 points, not top 10; the C++ route gave
82 to 84 points on their code. Top 10 (84.5 points) needs the C++ design plus luck on the board's
±4%. A rewrite should be written in our own words and credit them in `README.md`/`PERF.md`.

## Other contests checked (2026-11-01, read-only)

Sources cloned/downloaded to the session scratchpad (`ext/`), not run.

### SIGMOD 2016 shortest path (H_minor_free winner, uoa_team runner-up)
Tarballs from `dsg.uwaterloo.ca/sigmod16contest/downloads/`. Both are bidirectional BFS on dynamic graphs
(winner: batch the updated edges first so queries run lock-free in OpenMP threads; runner-up: adjacency
lists with cache tuning, 23 threads, multiversioning). Nothing transfers to a log-aggregation pipeline.
Only generic point: no locks on the hot path, which Track A already has (shared lock-free name table).

### SIGMOD 2025 SortMergeJoins (TUM)
The winning repository was not found (search shows only the contest template `umbra-db/contest-sigmod2025`,
a single "initial commit", and `fxsc03/sigmod-25-contest`, also a template). Not verified, no technique
extracted. Task is a join pipeline over columnar tables, so overlap with Track A would be limited to
thread partitioning and memory handling anyway. Revisit if the team's repo link appears on the contest page.

### 1BRC 2024 (`gunnarmorling/1brc`, `CalculateAverage_thomaswue.java` read in full)
Fastest entry (1.535 s on the 8-core/32 GB board, 0.31 s on a 13900K), credits merged from five authors.

| Technique | In thomaswue's entry | Our status |
|---|---|---|
| mmap input | yes | have |
| work stealing, 2 MB segments | atomic cursor, `SEGMENT_SIZE=1<<21` | have (8 x workers pieces) |
| child process so unmapping happens after the answer is out | `spawnWorker` | have the equivalent: `exit(0)` skips munmap and drops |
| name hash from first 16 bytes, branchless short/long name, masks from tables | yes | rejected: collisions (we hash every byte, compare hashes only) |
| 8-byte delimiter search by SWAR | yes | we use SSE2 64-byte masks |
| branchless number parse | yes | have SWAR `number` |
| **three independent cursors per thread, one segment split in 3, stepped in lockstep** | yes, explicit credit to cutting stalls | **new, untested** |

The one new idea is interleaving: each thread parses three disjoint pieces of its segment in the same loop
iteration so independent dependency chains overlap (loads, hash multiply, table lookup). Our parse runs at
IPC 1.86 with ~265 instr/record, so there is headroom on paper. Costs: three newline-mask states instead of
one, more registers, roughly +20 source lines; gain unmeasured, estimate 5-15% of the parse phase
(parse is 64% of T). Not started; needs a prototype and an instruction/IPC measurement on rl-lyceum before
any claim.
