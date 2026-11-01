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
