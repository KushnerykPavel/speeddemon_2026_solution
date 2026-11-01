# Track A research log, 2026-11-01

Tiers: VPS (`tools/bench.sh`, docker + cgroup, 6 vCPU Haswell, kernel 7.0) for correctness and
memory; a Linux box with perf counters and callgrind (4 cores/8 threads, kernel 4.15) for
instructions; the board for truth (±4% single-run spread).

| Question | Measurement | Result |
|---|---|---|
| What does the board charge as M? | cgroup `memory.stat` at exit, VPS | `file=10.1 MB` (the output on tmpfs/page cache) plus anon; peak is the sum while both exist |
| Can the output be kept out of M? | harness README/setup-box | no: `/out` is a tmpfs, `memory.peak` includes tmpfs |
| M after freeing records before output | VPS bench | 42.7 → 25.7 MB (board 46.6 → 28.4) |
| Instructions per record (full, 1 thread) | perf stat | 628 (A7) → 448 (step 10) → 442 (step 11) |
| Where do they go | callgrind on sample | chunk/scan ≈ 190, endpoint ≈ 33, quantile+gather ≈ 66, formatting ≈ 9% of total |
| Mispredicts | perf stat | ≈ 1.9% of branches, ≈ 9% of cycles: not the bottleneck |
| Kernel share | perf record | ≈ 10–12% (page-cache mapping, unmapping at exit) |

Rejected or unproven: byte comparison in the endpoint table (kept hash-only, 64-bit); wall-time A/B on
the shared box (too noisy); parallel `munmap` showed no VPS gain (VPS parse-bound at 6 threads) but
cannot be judged there; board T 0.0441 → 0.0408 with it plus parse changes.

Gap to top 10 (cutoff ≈ 84.5 points ≈ T 0.0215 s, M ≈ 19 MB, S ≈ 334): board now 0.0408 s,
30.2 MB, S 544. S is the cheapest remaining lever: it counts formatted code lines, so every
50 lines cost ≈ 0.7 points; the code grew 183 → 544 lines.

## Cheaper endpoint hash (3 words) — rejected, 2026-11-01

Idea: hash only the first, last and middle 8 bytes of an endpoint plus its length instead of every
byte (about 10 of 265 parse instructions per record). Checked offline against the generator's own
catalogue (`Catalogue(seed).names`, 1,842 names per seed, 300 seeds, 552,600 names), same
64-bit multiply-rotate mix as the parser:

| Hash input | Colliding name pairs over 300 seeds |
|---|---:|
| all bytes (current) | 0 |
| first + middle + last 8 bytes + length | **49** (16% of seeds) |
| first + last + 2 evenly spaced 8-byte words + length | 0 |
| first + last + 3 evenly spaced 8-byte words + length | 0 |

The three-word variant merges real endpoints in about one seed in six (names such as
`/api/v2/imports/{version}/returns/{name}` and `/api/v2/reports/{version}/ledgers/{name}` differ
only outside the sampled words), and the shared table identifies a name by its hash alone, so it
would silently corrupt the output on a hidden seed. The four-word variants are collision-free on
these seeds but are only a statistical argument and save about 3% of parse instructions, so the
full-byte hash stays. (Cache Me compares the name bytes after the hash; we measured that at about
10% of parse time earlier and do not.)

## Two-word hash plus SSE2 name comparison — measured, rejected, 2026-11-01

Cache Me's approach, in Rust: hash of the length and the first and last eight bytes, slot holding
the address of the name's own copy (64 zero bytes after it), every probe compared byte for byte
(four 16-byte SSE2 compares for names up to 64 bytes, slice equality beyond). It is correct by
construction: a test with two names of equal length, first eight and last eight bytes stays separate.

| | Instructions, full run, 1 thread | VPS board-like T (9 runs) | VPS M |
|---|---:|---:|---:|
| full-byte hash, hash-only table (current) | 1.277 G | 0.0915 s | 19.9 MB |
| two-word hash + SSE2 compare | 1.391 G (+9%) | 0.0976 s (+7%) | 19.6 MB |

The compare costs more than the shorter hash saves (about 6 words hashed on average against 2):
four loads, compares and movemasks per probe, plus the slot indirection, against roughly 10
multiply-rotate steps. Cache Me's version uses 32-byte AVX2 compares (two per name) and their
gain was against `memcmp`, not against a hash-only table. Reverted; the hash-only table with the
full-byte hash stays (collision-free on 300 generator seeds).

## Hour phase: plain loops instead of iterator chains (rejected, no effect)

Replaced the `flat_map`/`chain` record iterator with a `visit` helper (nested `for` loops, closure inlined).
Full run, 1 thread, rl-lyceum `perf stat`: 1,277,840,205 instructions (iterators) vs 1,277,696,543 (loops),
task-clock 176.9 vs 177.7 ms. The compiler already produced the same loops; VPS wall A/B (2x9 runs each)
was within noise (0.092/0.094 vs 0.087/0.099 s). Reverted. The ~117 instr/record in the hour phase is the
counting sort and selection work itself, not iterator overhead.

## Source-size trim (S 542 -> 536)

`number` now takes the digit count the caller already found, so it no longer re-measures the
length or returns it (the `found != digits` check is gone); `match digits` became a range check.
-6 code lines, output identical on all datasets at 1/3/12 threads.

Rejected: dropping `release`/`kept`/`finished` (about -22 lines, roughly +0.3 pt) raised M from 19.6 to
23.5 MB on the VPS (about -0.8 pt), so it is a net loss. Other candidates are worth under 0.15 pt each
(about 0.014 pt per line): `push_number` -> `write!` (slower text phase), tail copy -> padded mapping (-7 lines, riskier).

## Three-cursor interleaving, 1BRC style (rejected)

Prototype: `scan` splits its piece into 3 sub-pieces (`line_chunks`), each with its own `Cursor`
(position, newline mask, cached hour); one loop iteration parses a line from each (`Cursor::line`, returns a
small `Line` struct) and then hashes/looks up/files the three (`State::finish`).
Correctness: 7 tests pass; all 9 datasets byte-identical at 1/3/12 threads (full seed 1 sha256 `495ca09682b9...`).
Full run, 1 thread, rl-lyceum `perf stat`, baseline `5878e1d` vs prototype:

| | instructions | task-clock |
|---|---:|---:|
| baseline | 1,277.7 M | 177 ms |
| 3 cursors | 1,459.1-1,459.6 M (+14%) | 191.6 / 195.3 ms (+8..10%) |

Instructions grew from `Line` passing and the extra cursor state (spills), and wall time did not
improve; at ~265 instructions per record one line already fills most of the out-of-order window, so
batching three lines in source order does not make the CPU overlap them. Java's JIT/Graal gain does
not carry over. Not pushed; the patch was dropped with the branch. Third inconclusive/negative local
experiment in a row (loops, S trim, interleaving): per `AGENTS.md` the next step is fresh independent
research, not another local tweak.

## AVX2 + target-cpu=haswell (accepted candidate, not yet on the board)

Dockerfile builds with `RUSTFLAGS="-C target-cpu=haswell"`; `newlines` uses two 32-byte AVX2 compares when
`target_feature = "avx2"` is on, else the SSE2 path (and a scalar path off x86), so the source still builds
and passes tests everywhere (macOS tests use the fallback). No runtime dispatch: the binary needs AVX2/BMI2
(any x86 since 2013); a CPU without them would die with SIGILL.
rl-lyceum `perf stat`, full seed 1, 1 thread (baseline `5878e1d` = 1,256.0 M instr, 179 ms):
flags only 1,223.4 M (-2.6%), 177-186 ms; flags + AVX2 newlines 1,206.0 M (-4.0%), 168.6 / 170.7 ms (-5%).
VPS docker A/B (2x9 runs): 0.0871 / 0.0823 s vs 0.1003 / 0.0818 s, within noise; M 19.4-19.5 vs 19.2-19.3 MB.
Output byte-identical: full seed 1 (9 VPS runs), edge-cases, sample-seed7, tiny-seed42, empty; 7 unit tests pass.
Expected board effect about +0.3..0.5 pt (estimate).
