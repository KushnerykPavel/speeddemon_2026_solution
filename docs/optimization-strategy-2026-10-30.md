# Speed Demon 2026 — Tracks A & D optimization strategy

Date: 2026-10-30 (kickoff day). Supersedes nothing: it builds on
[`optimization-research-2026-10-29.md`](optimization-research-2026-10-29.md) (Russian, same author team)
and adds (1) verified harness measurement facts that change several priorities, (2) a points-based
cost model for every candidate, (3) the A–K structure requested for the team plan.

**Evidence labels used throughout**

- **[V]** verified in an official file or fetched primary source during this research (link given).
- **[L]** literature result measured on a *different* machine/workload — direction only, not a forecast.
- **[H]** our hypothesis; untested on the scoring box. Almost every speed estimate in this document is [H].
- **[D]** our arithmetic from verified facts.

No Rust variant beyond the committed baselines has been measured. The only local timings are single
correctness runs on a 6-vCPU VPS including Docker start (A 2.88 s, D 21.32 s; `tests/BASELINE_VALIDATION.md`).

---

## A. Executive summary

**Track A — most promising design.** `mmap` the read-only log (std-only `extern "C"` declaration, no crate),
split into 12 newline-aligned chunks, each thread runs a hand-written byte parser that (a) derives
`hour_id ∈ [0,72)` from 4 fixed timestamp bytes, (b) interns the endpoint in a small thread-local
open-addressing table keyed by a word-hash of the borrowed slice, (c) appends `(local_group, latency)` as
two `u32`. Then: merge endpoint dictionaries, assign global IDs **in byte-sorted order** (so group-id order
*is* output order), counting-sort the pairs into one contiguous latency arena, compute p50/p95/p99 per group
with `sort_unstable` (small) / cascaded `select_nth_unstable` (large), format with a digit-pair table into
per-thread buffers, one `write`. Zero dependencies.

**Track D — most promising design.** `mmap` both files, parallel parse into a `u32` CSR
(`offsets[n+1]`, packed `(head,weight)`), **DFS/BFS-reorder the vertices once** to undo the random relabelling,
reject trivially-decidable queries by degree (`s==t`, `outdeg(s)==0`, `indeg(t)==0`), then answer the
2,000 queries in parallel on 12 threads with per-thread reusable `u64` distance arrays + touched-list reset,
lazy-deletion heap, stop when `t` is popped. No routing index: the whole query workload is only ≈14
single-source-shortest-path (SSSP) equivalents [D], so CH/ALT/hub labels cannot amortize.

**Top five opportunities overall (by expected points / effort)**

1. **Parallel zero-copy parsing + fused aggregation (A) / parallel CSR build (D)** — both tracks' baselines
   are single-threaded line/`String` parsers. T exponent 0.5 dominates the score.
2. **D: array-based Dijkstra state + degree rejection + 12-way parallel queries** — removes the per-query
   `HashMap` and the two "no-in target" queries that each settle the whole 1.74 M-node main body.
3. **D: vertex reordering (DFS/BFS)** — PHAST measured binary-heap Dijkstra 12,683 ms on a random layout vs
   5,804 ms on a DFS layout of the same Europe graph [L]. Our input is *explicitly* randomly relabelled.
4. **A: dense group IDs + one contiguous latency arena + selection** — removes ~3.1 M hash lookups on
   `(String,String)` keys, 127 K `Vec` allocations, and the full sort of every group.
5. **Build configuration** (`lto="fat"`, `codegen-units=1`, `panic="abort"`, `target-cpu=native`) — zero
   source-line cost; Perf Book: LTO "can improve runtime speed by 10-20% or more" [L].

**Major technical risks**

- Correctness gate is SHA-256: percentile rank `(p*n+99)/100` 1-indexed, `floor` ppm, bytewise endpoint order
  with prefix-first, `-1` for unreachable, `u64` sums. Any shortcut here scores zero.
- Hold-out seed: nothing may depend on seed-1 endpoint catalogue, latency ranges or graph shape beyond the
  spec's contract.
- Source-size penalty is steep here (S_ref = 56 lines for A, 43 for D). A clever 600-line solution pays
  ≈ −7 points on S. Every feature must earn its lines (see §B cost model).
- Memory is measured as cgroup `memory.peak`; reading the input into heap memory costs real points; `mmap`
  of the pre-warmed input does not (§B).
- Local dev machine is Apple M1 Pro (ARM, no AVX2). All performance decisions must be measured on x86-64
  (VPS or board), never extrapolated from the laptop.

**Implement first:** build profile → A parser/interning/arena (serial) → A parallel → D CSR + arrays +
degree rejection → D parallel → D reorder. Details in §K.

---

## B. Verified competition constraints

Sources: official repo checkout `hackathon-raptors/speed-demon-2026@38936f6` (2026-10-30 13:41 +0200),
files `specs/a-ingest.md`, `specs/d-traverse.md`, `harness/scoring.py`, `harness/run_submission.py`,
`harness/static_axes.py`, `harness/dependencies.py`, `harness/calibration-seed1.json`,
`harness/FINAL_SCORING.md`; and https://speeddemon.dev/ [V]. The earlier baseline validation used
generator commit `575a99c`; `git diff 575a99c..38936f6` shows **no changes** to `generators/`, `specs/`,
`harness/scoring.py` or `harness/run_submission.py`, so existing validation hashes stand.

### Scoring (harness/scoring.py, frozen Oct 28) [V]

```
K = (T_ref/T)^0.50 × min(M_ref/M, 50)^0.20 × min(S_ref/S, 1)^0.15 × (D_ref/D)^0.15
points = clamp(50·log10 K, 0, 100);  track_total = 0.65·points + 0.35·judges
```

| Axis | How measured [V] | A ref | D ref |
|---|---|---|---|
| T | `sd-timer` CLOCK_MONOTONIC exec→exit inside container (Docker start excluded); 5 runs, mean of middle 3; spread must be ≤ max(5 %, 50 ms) | 32.047 s | 72.693 s |
| M | cgroup v2 `memory.peak` of the run cgroup; `/data` is **pre-warmed into page cache before the cgroup exists** (`prewarm()`: "or the submission faults it in itself and the pages count toward its memory.peak") | 227,983,360 B | 892,915,712 B |
| S | scc code lines after canonical `rustfmt` (team config ignored), **every file in the track dir** except registry crates first published before 2026‑10‑01; TOML/MD/lockfiles not counted | 56 lines | 43 lines |
| D | 1 + direct `[dependencies]` in `Cargo.toml` (optional included; dev/build deps excluded) | 1 | 1 |

M cap ⇒ maximum memory credit reached at M ≤ 4.56 MB (A) and ≤ 17.9 MB (D).

### Points sensitivity [D]

| Change | Points |
|---|---|
| Halve T | +7.5 |
| Halve M | +3.0 |
| Double S (above S_ref) | −2.3 |
| First dependency (D 1→2) | −2.3; second (2→3) −1.3 |
| A: S 56→200 lines | −4.1 |
| D: S 43→200 lines | −5.0 |

Consequences: one crate must buy > 1.23× speed-up (≈ 19 % less time) to break even; adding Δ lines to an
S-line solution must buy > ((S+Δ)/S)^0.3 speed-up (e.g. +50 lines on 200 ⇒ needs ≥ 6.9 %). Std-only code
is the default; `rayon`, `memchr`, `memmap2`, `hashbrown`, `itoa`, `radix-heap` all fail this bar unless a
measurement shows otherwise. Std already contains hashbrown (`HashMap` with a custom `BuildHasher`) and
scoped threads.

### Memory consequence [D, mechanism V]

Reading the 239 MB log into a `Vec<u8>` puts ≈ 239 MB of anonymous memory into the cgroup — **worse than
the A reference** (228 MB). `mmap` of the pre-warmed file maps pages already charged elsewhere; only page
tables and our own structures count. The difference for A is ≈ +9.8 points (239 MB → ~25 MB), i.e. worth as
much as a 2.7× speed-up. Same logic for D's 92 MB graph. `/tmp` is tmpfs and **does** count. The output
file's page cache (10 MB for A) is charged to the run; unavoidable.

`mmap` without a crate: declare `extern "C" { fn mmap(...) -> *mut c_void; }` — std already links libc on
Linux, so this adds no dependency (≈ 6–10 lines of `unsafe`).

### Sandbox and build [V]

12 hardware threads = six P-cores with SMT, `--cpuset-cpus=0-11`, turbo off, governor `performance`;
8 GB, no swap; 512 pids; `--network=none`, read-only rootfs, `/tmp` 1 GB tmpfs; unprivileged uid;
Docker default seccomp (**io_uring blocked, mmap works**). Build offline ≤ 10 min, image ≤ 2 GB, `FROM`
allowlisted + digest-pinned. Native builds are built on the scoring box ("native means the scoring CPU, a
Core i5-13500 with AVX2 and no AVX-512"). PGO/"build-time training on generator output" is allowed; training
timings do not count. Precomputed answers or input recognition = disqualification (rule 04). Live board
timeout 3× T_ref, final 10× T_ref. Final scoring re-calibrates T_ref/M_ref on a private hold-out seed.

Board: at most one run per 15 min per team — plan board runs; do local x86 measurement in between.

Judges (35 %): code quality 15 %, technique 12 %, PERF.md 8 % (each step needs a live-board run ID).

### Track A workload facts [V, specs/a-ingest.md]

- 3,100,000 lines, 238,836,234 B; `ts` exactly 27 bytes; all timestamps in
  `2026-10-30T18:00Z … 2026-11-02T17:59:59.999999Z` **for every seed and scale** (contract — using it for a
  72-slot hour index is not overfitting).
- 7 methods; endpoint 1–125 bytes, no spaces; exactly 1,842 endpoints per seed, Zipf exponent 1
  (top endpoint ≈ 12.4 % of traffic ⇒ ≈ 5.3 K requests per hour-group; rank 1000 ≈ 5/hour [D]).
- status 3 digits, any 100–599 must work; error ⇔ first digit `5`.
- latency `u32` 0…4294967295, no leading zeros. Generator: `base·2^k + U[0, base·2^k)`, ×3 for 5xx, plus
  injected 0 and 4294967295 — wide range, so per-group counting sort is not viable; the contract allows any
  u32 anyway.
- Order: 8 interleaved shards; **no bound on disorder** — no hour flushing.
- Output: 126,908 lines / 10,113,024 B at full seed 1; hour then endpoint bytes (prefix first).
- Banned: dataframe/SQL engines, datamash/xsv/etc. Everything else allowed and priced on D.

### Track D workload facts [V, specs/d-traverse.md]

- n = 1,740,512, m = 5,000,000, 2,000 queries; dense IDs `[0,n)`; weights `0…u32::MAX`; answers < 2^63.
- Road-like jittered grid, arterials every 8th, highways every 48th row/col; **main body (1,740,349 nodes) is
  one SCC**; IDs randomly relabelled, edge lines shuffled.
- Special structures: 17 isolated, 17 no-in, 25 no-out nodes, 8 islands of 14 nodes; ~5 K parallel groups,
  1 K duplicate lines, 508 self-loops, ~2.5 K zero weights — together ≈ 0.2 % of edges [D].
- Normal queries (1,950): `s` uniform in main body, `t` = r-th settled node with `log2 r` uniform in 1..17.
  Expected settled nodes per query ≈ 12,287 ⇒ ≈ 24 M settles total ≈ 14 full SSSPs [D].
  **Octave 16 (≈ 6 % of queries) is ≈ 50 % of all query work; octave 15 another 25 %** [D].
- Answer `-1` queries: 10 at full (island→main, no-out source, isolated source, no-in target). The 2 no-in
  target queries make the reference settle the whole main body each.
- Banned: any shortest-path/routing-index library (petgraph::algo, fast_paths, pathfinding, RoutingKit…).
  Allowed: heaps incl. radix heaps, hash maps, parallel runtimes, graph containers.

---

## C. Track A — optimization catalog

Baseline being replaced: `track-a/src/main.rs` — `BufReader::lines()` (one `String` per line), `split(' ')
.collect()`, `HashMap<(String,String),Group>` with two key allocations per line, `Vec<u32>` per group,
stable `sort()` of each group, `writeln!` formatting. Single-threaded.

Format: **ID — name** · stage · replaces · why it applies · bottleneck · complexity · effect · memory · S/D ·
difficulty (1–5) / effort · correctness risk · benchmark · source · **priority**.

**A1 — Release profile + native CPU** · build · default release · hot loops are branchy byte code where
inlining/LTO matter; target is fixed (i5-13500, AVX2) · instruction count · — · [L] LTO "10-20% or more",
target-cpu helps "especially if… vectorization" (Perf Book) · none · 0 code lines (TOML; Dockerfile
`ENV RUSTFLAGS` adds ~1 line — verify Dockerfile counting with `static_axes.py`) · 1 / 15 min · none ·
A/B end-to-end · https://nnethercote.github.io/perf-book/build-configuration.html · **P0**
*Mechanics [V]:* `[profile.release]` goes in `Cargo.toml`. The Dockerfile only `COPY`s `Cargo.toml
Cargo.lock src`, so a `.cargo/config.toml` is invisible unless a `COPY .cargo .cargo` line is added; the
alternative is `ENV RUSTFLAGS="-C target-cpu=native"` in the build stage. The board detects native flags by
regex over the track files (`board/steps.py` `NATIVE_CPU_RE`) and then builds on the scoring box instead of
Depot ("Depot's CPU has AVX-512 and the box's does not"); final scoring always builds on the box
(`harness/README.md`). Fallback if box builds queue badly: `-C target-cpu=x86-64-v3` (AVX2, no AVX-512).

**A2 — `mmap` the input, no crate** · I/O · `BufReader` lines · enables zero-copy borrowed endpoint slices and
lock-free chunking; pre-warmed page cache means mapped pages are not charged to M [V] · syscalls/copies,
M · — · [H] removes 239 MB copy; [L] Koerkamp 1BRC: mmap step 17 s→12 s · ≈0 MB anon for input ·
~10 lines unsafe extern · 2 / 1 h · must not read past end (last line ends in LF, guaranteed) · M via
`check.py`/cgroup + T · https://curiouscoding.nl/posts/1brc/ · **P0**

**A3 — Hand-written field parser** · parse · `split().collect()` + `str::parse` · grammar is fixed: bytes
0–26 timestamp, method ∈ 7 literals (length from first two bytes or scan to space from byte 28), endpoint up
to next space, then `SSS␠latency\n`. Parse latency backwards from the LF or forwards after status; status
error test = `byte == b'5'` · branch mispredicts, bounds checks · O(bytes) unchanged, constant ↓ · [L]
Koerkamp: "bytes" 105→84 s; branchless parse cut branch misses 440M→140M · none · ~30 lines · 2 / 2 h ·
latency `0`, 10-digit `4294967295` (must use u64 accumulator or exact u32 path), status `5xx` incl. 599 ·
unit tests on E1–E15 fixtures · spec §2 · **P0**

**A4 — Hour index from 4 bytes** · parse · `ts[..13]` `String` key · contract fixes the 72-hour span:
`hour_id = 24·day_index(ts[8..10] with month ts[6]) + HH − 18`, i.e. Oct 30→0, Oct 31→1, Nov 1→2,
Nov 2→3 · hashing/allocation · — · [H] · none · ~5 lines · 1 / 30 min · E4/E5 boundaries, month rollover ·
spec E4/E5 + worked example · **P0**

**A5 — Endpoint interning, custom open-addressing table** · aggregate · `HashMap<(String,String)>` · 1,842
keys fit in a 4096-slot table (L1/L2 resident); key = borrowed `&[u8]` into the mapping; hash = multiply-xor
over 8-byte words (FxHash-style `(h + w)·K`, rustc-hash) plus length; store `(hash, ptr, len)`; full byte
compare on hash match · hashing, cache misses · O(1) per line · [L] Koerkamp FxHash 50→41 s; thomaswue
`findResult()` hashes first 16 bytes with full-key compare · ~40 KB/thread · ~30 lines · 2 / 2 h · must
compare full key (endpoints share long prefixes, up to 125 B) · microbench lookups/s; end-to-end ·
https://github.com/gunnarmorling/1brc (`CalculateAverage_thomaswue.java`), rustc-hash `src/lib.rs` · **P0**

**A6 — Fused `(group,latency)` pair stream + counting-sort arena** · aggregate/finalize · per-group `Vec<u32>`
· each thread pushes `local_eid·72+hour` and latency into two flat `Vec<u32>` (or one `Vec<u64>`), plus a
count per local group. After merge: global counts → prefix sums → scatter latencies into one 12.4 MB arena.
No per-group allocation; scatter is one linear pass · allocator, pointer chasing · O(N + G) · [H] · ≈25 MB
pairs + 12.4 MB arena (can drop pairs per thread after scatter, but peak counts) · ~30 lines · 3 / 3 h ·
nothing order-dependent (percentiles are order-free) · M and T · ref doc A2 idea + VLDB 2026 fused
parse/aggregate (https://www.vldb.org/pvldb/vol19/p3579-ellmann.pdf, from 10-29 doc) · **P0**

**A7 — Global endpoint IDs assigned in byte order** · finalize · `sort` of 127 K `(String,String)` keys ·
sort only 1,842 unique endpoints (`[u8]` `Ord` is unsigned-byte lexicographic with prefix first — exactly
the spec order); `gid = hour·E + rank`; iterating gid ascending yields output order directly · sort cost ·
O(E log E) vs O(G log G) · [H] · negligible · ~10 lines · 1 / 1 h · tiny scale has < 1,842 endpoints — use
the discovered count `E`, never 1,842 · byte-compare against reference · spec §4 · **P0**

**A8 — Newline-aligned parallel chunks, thread-local state** · parse · single thread · lines are independent
and LF never appears inside a field; std `thread::scope`, 12 chunks, boundary = next LF after `i·len/12` ·
CPU throughput · ideal ≤ 6–12× · [L] Koerkamp 1BRC: one chunk per thread 12 s→2.0 s; 12 threads with HT
1.7→1.34 s; thomaswue "Runs in 0.31 on an Intel i9-13900K" for 1 B rows · per-thread tables + pairs ·
~25 lines · 3 / 3 h · last chunk end, empty file (zero bytes → zero output), chunk smaller than a line ·
1/2/4/6/12 threads, page-fault time · same sources · **P0**

**A9 — Percentile: `sort_unstable` small, cascaded `select_nth_unstable` large** · finalize · stable `sort()`
· only 3 ranks needed; std introselect is O(n) "guarantees linear runtime"; for k99 ≥ k95 ≥ k50: select k99
on `[0,n)`, then k95 on `[0,k99)`, then k50 on `[0,k95)` (each later selection is within the left part
already ≤ the found value, ranks equal ⇒ reuse) · comparisons, memory traffic · O(n log n) → O(n) ·
[H] the large Zipf groups (≤ ~5 K) dominate work; most groups are ~24 values where sort is fine · none ·
~12 lines · 2 / 1 h · rank formula `(p·n+99)/100 − 1`; n=1, n=101 (51/96/100), all-equal values ·
threshold sweep 16/32/64/128 on sample+full · https://doc.rust-lang.org/std/primitive.slice.html ·
**P0** (sort_unstable alone first, then threshold)

**A10 — Parallel finalize by hour** · finalize · serial loop · 72 hours × E groups are independent; give each
thread a contiguous hour range, it selects percentiles and formats into its own `Vec<u8>`; concatenate in
hour order · CPU · ≤ 12× on finalize · [H] · per-thread output buffers (≈10 MB total) · ~15 lines · 2 / 2 h
· order of concatenation · phase timer · — · **P1**

**A11 — Digit-pair integer formatting + precomputed hour prefixes** · output · `writeln!` formatter ·
126,908 lines × 6 numbers; `core::fmt` has overhead; 72 hour strings `YYYY-MM-DDTHH:00:00Z` precomputed ·
instruction count · — · [L] itoa: "avoids the performance penalty of going through core::fmt::Formatter" ·
none · ~15 lines (hand-rolled, not the crate) · 1 / 1 h · no leading zeros, `0` · hash compare · itoa
`src/lib.rs` (`DECIMAL_PAIRS`, `divmod100`) · **P1**

**A12 — Single `write` + skip teardown** · output/exit · `BufWriter` + drops · one `write_all` of the joined
buffer; `std::process::exit(0)` after flush to skip freeing ~40 MB of structures and `munmap` · syscalls,
munmap lock · — · [L] Koerkamp: `munmap` takes a process-wide lock (~180 ms in his case) · none · 2 lines ·
1 / 15 min · must flush/close output before exit · T · curiouscoding.nl · **P1**

**A13 — SWAR newline / space search** · parse · byte loop · 8 bytes per step with
`(x ^ 0x0A…) - 0x01… & !x & 0x80…`; endpoints average ~20 bytes so a byte loop runs ~20 iterations ·
branch count · constant ↓ · [L] thomaswue `findDelimiter()`; Koerkamp: manual scans beat non-inlined libc
memchr (41→47 s regression) · none · ~10 lines · 2 / 1 h · tail handling near file end (mmap is page-padded
but don't rely on reading past `len` — use scalar tail) · perf stat instructions/branch-misses · thomaswue
source · **P1** (only if profile shows scanning > 20 %)

**A14 — `MAP_POPULATE` / `madvise(MADV_SEQUENTIAL|WILLNEED)`** · I/O · lazy minor faults · 58 K minor faults
on a 239 MB mapping; populate in kernel or let 12 threads fault in parallel · page-fault time · — · [H] ·
none · 1–3 lines · 1 / 30 min · none · `perf stat -e page-faults`, T · Linux man mmap(2) · **P2**

**A15 — Interleaved multi-cursor parsing (ILP)** · parse · one cursor per thread · 2–4 independent line
cursors per thread hide dependency chains · IPC · constant ↓ · [L] Koerkamp: 4 streams 1.34→1.23 s, and
"Hyperthreading now slows the code" after this; thomaswue uses 3 cursors per segment · none · +30–50 lines ·
4 / 4 h · chunk-boundary bookkeeping · IPC + T at 6 vs 12 threads · both sources · **P2**

**A16 — PGO with committed profile** · build · — · LLVM needs `llvm-profdata` to merge; the pinned
`rust:1-slim` image has no `llvm-tools` and builds are offline, so the merged `.profdata` would be generated
outside and committed; must match the image's exact rustc/LLVM version · branch layout · — · [L] Perf Book
"10% or more" · none · 0 code lines, binary file (check S treatment) · 3 / 3 h · stale profile is only a
perf risk · A/B · https://doc.rust-lang.org/rustc/profile-guided-optimization.html · **P2**

**A17 — u32-only latency accumulation with 10-digit guard** · parse · `str::parse::<u32>` · accumulate in
`u64` and cast (≤ 10 digits ⇒ ≤ 9,999,999,999 fits u64; input guaranteed ≤ u32::MAX) · branch · [H] · none
· — · 1 · correctness on `4294967295` · E2/E7 · **P0 (part of A3)**

**A18 — AVX2 64-byte structural bitmask (simdjson stage 1)** · parse · scalar scan · classify LF/space in 64
B blocks, extract positions with `tzcnt` · branch mispredicts · [L] simdjson ≥ 2 GB/s on Skylake for JSON ·
none · +60–100 lines `core::arch` intrinsics · 4 / 6 h · tails, portability (local M1 can't run it) ·
GB/s microbench on x86 · https://arxiv.org/abs/1902.08318 · **P2/Reject** — 239 MB at a few GB/s/thread ×
12 threads is already ~20 ms; S cost likely exceeds gain.

**Rejected for A**

| Idea | Reason |
|---|---|
| t-digest, DDSketch, KLL, HDRHistogram, log buckets | Approximate; output must equal an input value at exact rank. |
| Per-group counting sort / global `u32` histogram | Range up to 2^32 (16 GiB counters); generator emits values to ~5·10^8. |
| Radix-sort crate (rdst etc.) | Dependency; rdst's own tuner picks comparison sort for `input_len <= 128`, and the average group is ~24 values [V source]. |
| MSD radix select (Lucene `RadixSelector`) | Correct and exact, but falls back to introselect ≤ 100 elements; groups ≤ ~5 K — gain unproven, extra code. P2 at best. |
| Hour-watermark streaming flush | Spec: "no bound on how far a line can be out of order is promised". |
| Perfect hash (PtrHash) | 1,842 keys already L1/L2-resident with linear probing; build code cost. |
| `rayon`, `memchr`, `memmap2`, `hashbrown`, `itoa` crates | Each costs −2.3 points; std/hand-rolled equivalents are ≤ 15 lines each. |
| `read()` whole file into `Vec` | +239 MB charged to M (≈ −10 points vs mmap). |
| Hard-coding seed-1 endpoint catalogue or 1,842 | Seed-dependent; tiny has fewer; rule 04. |

---

## D. Track D — optimization catalog

Baseline being replaced: `track-d/src/main.rs` — `lines()` + `split_whitespace().collect()`,
`Vec<Vec<(usize,u32)>>` (16-byte edges, 1.74 M separate allocations), per query a new `HashMap<usize,u64>`
and `BinaryHeap`, sequential queries, no rejection.

**D1 — Release profile + native CPU** · build · as A1 · **P0**

**D2 — `mmap` + hand-written integer scanner** · parse · `String` lines · 92 MB of `u v w\n`, ~18.4 B/line;
digit loop `v = v·10 + (b − b'0')` · parse CPU, M · [L] Koerkamp bytes/mmap steps · input not charged to M ·
~25 lines · 2 / 2 h · `w` up to 4294967295 (accumulate u64) · parse phase timer · spec §2 · **P0**

**D3 — CSR with `u32` offsets and packed `(head:u32, w:u32)`** · build · `Vec<Vec<(usize,u32)>>` · contiguous
neighbor ranges; edges 40 MB + offsets 7 MB = ≈ 47 MB [D] vs ≈ 80 MB + per-Vec headers (42 MB) +
allocator overhead today · cache misses, allocation · O(n+m) build · [H] · −50 % graph memory · ~20 lines ·
2 / 2 h · nodes with no edges still in `offsets` · build timer, M · MIT 6.172 L22 slides (CSR); RoutingKit
`inverse_vector.h` (read-only reference) · **P0**

**D4 — Parallel CSR build** · build · serial · either (a) parse once into per-thread `(u,v,w)` triples (60 MB),
count degrees, prefix sum, scatter; or (b) parse twice: pass 1 counts `outdeg`/`indeg` with `AtomicU32`
relaxed adds, pass 2 scatters with an atomic cursor per node. (b) avoids the 60 MB transient peak at the cost
of re-parsing 92 MB (cheap in parallel) · parse + scatter CPU, M peak · [H] · (a) +60 MB transient, (b) +7 MB
cursors · ~30 lines · 3 / 3 h · within-node edge order becomes nondeterministic — harmless, answers are
exact · M and T for both variants · — · **P0** (start with (a) serial-scatter if simpler, measure)

**D5 — Degree rejection** · query · none · `s==t ⇒ 0` (first, also for isolated nodes); `outdeg(s)==0 ⇒ −1`;
`indeg(t)==0 ⇒ −1`. The two no-in-target queries otherwise settle all 1.74 M nodes (reference: "about 3 s
each" in Python) · whole-graph searches · removes ≈ 2 full SSSPs ≈ 14 % of total query work [D] · `indeg`
as `u32[n]` (7 MB) or bitset (0.2 MB) · ~5 lines · 1 / 30 min · `s==t` must win over `outdeg==0` ·
fixtures: isolated `s==t`, no-in target · spec §2 queries table · **P0**
*Gap:* degree checks cover exactly the unreachable kinds the generator emits; a legal-but-ungenerated
main-body→island query would still settle all 1.74 M nodes. Closing it needs weak-component labels
(`comp[s] != comp[t] ⇒ −1`). A forward-only DFS (D8) does **not** yield weak components; use union-find over
the edge list during the build pass (O(m·α), `u32[n]` parents) or a traversal over forward+reverse CSR if
D10 already built it. Cost/benefit only matters for hold-out robustness, since the generator never emits that
query kind — P2.

**D6 — Array distances + touched-list reset, reused heap, early exit** · query · per-query `HashMap`/heap ·
dense IDs ⇒ `dist: Vec<u64>` indexed directly, initialised once to `u64::MAX`; record each first-touched
node and reset only those after the query; `heap.clear()` keeps capacity; stop when `t` is popped (not when
first relaxed) · hashing, allocation · O(touched) reset · [H] large; [V] generator's own `rank_target` uses
the touched-list pattern · 14 MB/thread · ~20 lines · 2 / 2 h · stale-entry skip `d > dist[u]`; INF never
added to · fixtures incl. zero-weight, over-u32 · RoutingKit `timestamp_flag.h`, fast_paths
`valid_flags.rs` (epoch alternative) · **P0**

**D7 — Parallel independent queries, dynamic scheduling** · query · sequential · immutable shared CSR,
thread-private state; `AtomicUsize` next-query counter (work is extremely skewed: octave-16 queries ≈ 50 %
of work), answers into `Vec<i64>` by index · CPU · ≤ 6–12× · [H]; Dijkstra on random layout is
latency-bound, so SMT may help more than in A — measure 6 vs 12 · +14 MB per thread (12 threads ≈ 167 MB)
· ~15 lines · 2 / 2 h · output order by index · per-thread busy time, tail · Bast et al. survey notes
Dijkstra "is hard to parallelize" — hence parallelize *across* queries · **P0**

**D8 — Vertex reordering (DFS or BFS order) after CSR build** · preprocess · random IDs · IDs are a random
permutation ⇒ every relaxation touches a random cache line of `dist`, `offsets`, `edges`. Relabel by
DFS/BFS discovery order from a main-body node, rebuild CSR in new IDs, map queries `s,t` through the
permutation · cache/TLB misses · O(n+m) preprocessing ≈ one traversal + one scatter · [L] PHAST Table 1
(Europe, 18 M nodes): binary heap random 12,683 ms vs DFS 5,804 ms; Dial 8,996 vs 3,355; "reordering the
vertices by a simple depth first search (DFS) like procedure… already gives good results"; caveat: our graph
is 10× smaller (node arrays ~7–14 MB vs 24 MB L3 on i5-13500 is not verified here — check) so the gain may be
smaller · +4 B/node permutation, transient second CSR (+47 MB unless done carefully) · ~30 lines · 3 / 4 h ·
unreached components (islands, isolated) must still get IDs — append them after the traversal · settles/s
before/after; end-to-end incl. reorder cost ·
https://www.microsoft.com/en-us/research/wp-content/uploads/2010/09/phastTR.pdf, MIT 6.172 L22
("BFS, DFS, METIS, by degree") · **P1** (highest-value P1)

**D9 — Priority queue variants** · query · `std::BinaryHeap<Reverse<(u64,u32)>>` lazy · candidates: custom
4-ary heap; own one-level radix heap on `u64` keys (65 buckets, `leading_zeros` bucket index); Dial ring with
overflow heap. Heaps here are small (local queries) · heap ops, branch misses · binary O(log h); radix
O(1) push / amortised O(log C) pop · [L] PHAST: smart queue/Dial ≈ 1.4–1.8× over binary heap in Table 1;
Chen et al. TR-07-54 road networks: lazy no-decrease-key "ran faster (often significantly)" and differences
among heaps ≈ 10–20 %; Bast: "4-heaps or 8-heaps tend to work best", multilevel buckets +20 % · heap
memory small · radix ~40 lines, 4-ary ~30 · 3 / 3 h · radix heap requires monotone keys: key must be the
distance only (node as payload), zero-weight edges keep monotonicity; Dial ring must handle w ≈ 2^32 ·
heap ops count, T · https://www.cs.utexas.edu/ftp/techreports/tr07-54.pdf, mpdn/radix-heap `src/lib.rs`
(MIT; read, don't depend) · **P1** (after D8 — layout changes relative cost)

**D10 — Bidirectional Dijkstra** · query · forward only · ball-growing on a 2-D road graph: two balls of
radius d/2 cover ≈ half the area of one ball of radius d · settled nodes · [L] Bast Table 1 (W. Europe,
random long queries): 9.33 M → 4.91 M scanned, 2.20 s → 1.21 s; Goldberg-Harrelson M7: D 605 ms vs B 483 ms
— gains shrink for local queries · reverse CSR +47 MB and its build; 2× per-thread state (+14 MB) ·
~60 lines · 4 / 5 h · stopping rule: stop when `top_f + top_b ≥ μ` (best meeting path), not at first
meeting; INF+INF overflow; zero-weight edges · settled counts by octave · Goldberg-Harrelson SODA'05
https://www.cs.princeton.edu/courses/archive/spring06/cos423/Handouts/GH05.pdf · **P1/P2** — matters
mainly for octaves 14–16 (≈ 87 % of work), benchmark only after D6–D8.

**D11 — Prefetch neighbor offsets/dist on push** · query · none · issue `_mm_prefetch` for `offsets[v]` when
pushing `v` · memory latency · [H] · none · ~5 lines (`core::arch`) · 2 / 1 h · none · perf cache-misses · —
· **P2**

**D12 — u32 distance arrays with u64 fallback** · query · `u64` dist · normal queries have distances far
below 2^32 (weights ≤ few thousand); run with `u32` and saturating check; if any tentative distance would
exceed `u32::MAX`, rerun that query with the `u64` path · per-thread memory 14→7 MB, cache footprint · [H] ·
−84 MB at 12 threads ⇒ ≈ +1–2 points on M [D] · +20 lines (generic over width) · 3 / 3 h · the overflow
detection must be exact (`checked_add`), including queries from no-in nodes (≥ 4,293,918,720) · fixtures
over-u32 · spec §2 · **P2**

**D13 — Query deduplication** · query · each query separately · 20 duplicate lines of 2,000 (1 %) · [D] ≤ 1 %
of work · ~10 lines · **Reject/P2** — S cost ≈ gain.

**D14 — Parallel-edge dedup / self-loop removal** · build · keep all · ≈ 0.1 % of edges [V counts] ·
dedup needs sort/hash per node · **Reject**.

**D15 — SCC / weak-component labels for early `-1`** · preprocess · none · degree checks (D5) already catch
no-out sources, isolated sources and no-in targets; island→main searches settle ≤ 14 nodes · O(n+m) ·
**Reject** (cost ≥ benefit).

**D16 — Thread count and pinning** · query · — · 6 P-cores × 2 SMT; latency-bound Dijkstra may gain from SMT
· [H] · memory scales per thread · 0 lines (constant) · **P0 experiment** (6 vs 12; site: "Twenty workers
will cost you more than they buy").

**D17 — `process::exit` after writing** · exit · drops · avoid freeing ~250 MB · **P1** (same as A12).

**D18 — PGO** · as A16 · **P2**.

**Rejected for D (heavy indexes and theory)**

| Idea | Why not here |
|---|---|
| Contraction Hierarchies | Geisberger et al. Europe: 545 s + 57 s preprocessing (fast variant) [L]; PHAST: CH "amortized away after only 278 trees" — our total workload ≈ 14 SSSP equivalents [D]. Building CH yourself is also large S. |
| ALT / landmarks | Directed ALT with k landmarks needs 2k full SSSPs; k=16 ⇒ 32 SSSPs > the whole query budget (≈ 14) and ≈ 223 MB of tables [D]. Goldberg-Harrelson's 16-landmark speed-ups assume preprocessing is free. |
| Hub labeling, transit nodes, arc flags | Preprocessing far above CH; banned as libraries, prohibitive to write. |
| Geometric A* | No coordinates; IDs randomly relabelled — no admissible heuristic available for free. |
| Delta-stepping | Parallelises one SSSP (Meyer-Sanders: generalisation of Dial/Bellman-Ford); we have 2,000 independent searches — cross-query parallelism is simpler and has no sync. |
| Global Dial buckets | C up to 2^32 ⇒ O(m + nC) and C+1 buckets infeasible (Ahuja et al.). |
| Duan et al. 2025 "Breaking the Sorting Barrier" | Implementation study (arXiv 2511.03007): "Dijkstra's algorithm remains 3 to 4 times faster in all tested scenarios". |
| Source batching / multi-target search | Sources are uniform and essentially distinct. |

---

## E. Algorithm comparison

### Exact percentiles (A)

| Method | Time per group | Extra memory | Fit |
|---|---|---|---|
| Stable `sort` (baseline) | O(n log n), merge-based, allocates | up to n/2 | Correct; wasteful. |
| `sort_unstable` (ipnsort) | O(n log n); O(n log k) for k distinct | none | Best for small n (most groups ~24). |
| Cascaded `select_nth_unstable` ×3 | O(n) expected, linear worst case (median-of-medians fallback) | none | Best for large Zipf-head groups. |
| Multiselect (Kaligosi et al.) | ≈ information-theoretic bound | none | Cascade above is the 3-rank special case. |
| Floyd-Rivest | n + min(k, n−k) + o(n) comparisons | none | Possible P2 for the largest groups; std introselect is likely enough. |
| MSD radix select (Lucene) | O(n · passes) | 257-entry histogram | Exact; wins on large n; groups ≤ ~5 K. |
| Counting sort | O(n + range) | range | Invalid: range 2^32. |
| Sketches (t-digest, HDR…) | O(n) | small | **Not exact — rejected.** |

Global alternative: one LSD radix sort of all 3.1 M `(gid<<32)|lat` `u64` keys gives every group sorted in
one pass structure, but costs 2×25 MB buffers and ~8 passes; the arena + per-group select is cheaper on both
T and M [H].

### Shortest paths (D)

| Algorithm | Preprocessing | Per query | Memory | Directed/0-weight/2^32 safe | Verdict |
|---|---|---|---|---|---|
| Dijkstra + binary heap (lazy) | none | O(r log r) for rank r | dist + heap | yes | Baseline of choice |
| 4-/8-ary heap | none | fewer levels, more compares | same | yes | P1 experiment |
| Radix heap (u64) | none | O(1) push, amortised O(log C) pop | 65 buckets | yes (monotone) | P1 experiment |
| Dial (global) | none | O(m + nC) | C+1 buckets | C≈2^32 ✗ | Reject |
| Bidirectional Dijkstra | reverse CSR | ≈ ½ settled on long queries | 2× | yes, careful stop rule | P1/P2 |
| ALT | 2k SSSPs | 10×+ fewer settles | 16kn B | yes | Reject (amortisation) |
| CH | minutes on 18 M nodes | µs | 2–3× graph | yes | Reject |
| Hub labels | > CH | sub-µs | large | yes | Reject |
| Delta-stepping | none | parallel single SSSP | buckets | yes | Reject (wrong axis of parallelism) |
| BMSSP (Duan 2025) | none | theory O(m log^(2/3) n) | — | yes | Reject (3–4× slower in practice) |

Work model [D]: expected settles ≈ 24 M; one full SSSP ≈ 1.74 M settles; so any method whose preprocessing
exceeds a handful of SSSPs cannot win end-to-end, regardless of query speed.

---

## F. GitHub implementation references (read, adapt patterns, don't depend)

| Repo / file | Technique | Lang / licence | Use |
|---|---|---|---|
| gunnarmorling/1brc `src/main/java/dev/morling/onebrc/CalculateAverage_thomaswue.java` | mmap; segments of `1<<21` claimed via `AtomicLong`; SWAR `findDelimiter()`; 16-byte hash fast path with full compare (`findResult()`); branchless number parse | Java, Apache-2.0 | A3, A5, A8, A13 |
| …`CalculateAverage_merykitty.java` | 256-bit vector compare of whole key against bucket key | Java, Apache-2.0 | A5 (idea only) |
| https://curiouscoding.nl/posts/1brc/ (Ragnar Groot Koerkamp) | Rust step-by-step log with timings; memchr vs manual; ILP streams; munmap lock | Rust, blog CC BY-SA | A whole pipeline; PERF.md style model |
| rust-lang/rustc-hash `src/lib.rs` | FxHash `(h + w) * K`, `rotate_left(26)` | Rust, MIT/Apache | A5 hash function |
| dtolnay/itoa `src/lib.rs` | `DECIMAL_PAIRS`, `divmod100` | Rust, MIT/Apache | A11 (hand-roll ~15 lines) |
| apache/lucene `lucene/core/src/java/org/apache/lucene/util/RadixSelector.java` | MSD radix select, fallback to introselect ≤ 100 | Java, Apache-2.0 | A9 alternative (P2) |
| mpdn/radix-heap `src/lib.rs` | `RadixHeapMap`, bucket = `(a ^ b).leading_zeros()`; max-heap (use `Reverse`) | Rust, MIT | D9 own implementation |
| easbar/fast_paths `src/dijkstra.rs`, `src/valid_flags.rs` | lazy-deletion BinaryHeap loop; u32 epoch reset | Rust, MIT/Apache (read only — crate banned) | D6 |
| RoutingKit `include/routingkit/timestamp_flag.h`, `dijkstra.h`, `id_queue.h`, `inverse_vector.h` | timestamp flags, 4-ary ID queue, CSR from sorted tails | C++, BSD-style (read only — banned) | D3, D6, D9 |
| hackathon-raptors/speed-demon-2026 `generators/d_traverse.py` `rank_target` | touched-list reset pattern | Python, MIT | D6 |

---

## G. Academic research and university resources

| Work | Result relevant here | Applies to | Transfer caveat |
|---|---|---|---|
| Delling, Goldberg, Nowatzyk, Werneck — PHAST TR (2010) https://www.microsoft.com/en-us/research/wp-content/uploads/2010/09/phastTR.pdf | Random vs DFS layout: binary heap 12,683 → 5,804 ms; Dial 8,996 → 3,355 ms; CH amortised after 278 trees | D8, D9, CH rejection | 18 M-node Europe, i7-920; full trees not point-to-point |
| Chen, Chowdhury, Ramachandran et al. — Priority Queues and Dijkstra's Algorithm, UTCS TR-07-54 (2007) https://www.cs.utexas.edu/ftp/techreports/tr07-54.pdf | No-decrease-key faster "(often significantly)"; road heaps fit in L2; Central USA: binary 13,965 ms, aligned 4-ary 12,341 ms | D6, D9 | 2007 hardware; full SSSP |
| Bast et al. — Route Planning in Transportation Networks (2015) https://arxiv.org/pdf/1504.05140 | Bidirectional halves scanned nodes on random queries; 4/8-heaps best; graph-search methods faster for local queries | D9, D10, rejections | Long random queries |
| Geisberger, Sanders, Schultes, Delling — Contraction Hierarchies (WEA 2008) https://ae.iti.kit.edu/1536.php | 545 s + 57 s preprocessing, 223 µs queries (Europe) | CH rejection | Older CPU |
| Goldberg, Harrelson — A* meets graph theory (SODA 2005) https://www.cs.princeton.edu/courses/archive/spring06/cos423/Handouts/GH05.pdf | M7: D 605 ms, bidirectional 483 ms, BLA (16 landmarks) 36 ms; correct bidirectional stopping | D10, ALT rejection | Preprocessing not charged |
| Ahuja, Mehlhorn, Orlin, Tarjan — radix heap (Princeton TR-154-88) https://www.cs.princeton.edu/research/techreps/682 | O(m + n log C) one-level | D9 | Abstract only fetched |
| Meyer, Sanders — Δ-stepping (ESA'98) https://www.cs.utexas.edu/~pingali/CS395T/2013fa/papers/delta-stepping.pdf | Single-SSSP parallelism | Rejection rationale | — |
| Wei et al. — Gorder (SIGMOD 2016) https://raw.githubusercontent.com/datourat/Gorder/master/paper.pdf | RCM 1.45 s, CHDFS 0.53 s, Gorder 13.43 s ordering time (Pokec) | D8 cost: use cheap DFS/BFS, not Gorder | Social graphs |
| Castro et al. — BMSSP implementation study (arXiv 2511.03007) | Dijkstra "3 to 4 times faster in all tested scenarios" | Rejection | — |
| Langdale, Lemire — Parsing Gigabytes of JSON per Second https://arxiv.org/abs/1902.08318 | 64-byte branchless structural bitmaps, ≥ 2 GB/s | A18 | JSON grammar irrelevant |
| Lemire — SWAR 8-digit parse https://lemire.me/blog/2022/01/21/swar-explained-parsing-eight-digits/ | branch-free 8-digit conversion | A3 (late, if latency parse shows up) | needs exactly 8 digits; latencies are 1–10 |
| Barbay, Gupta — multiselection (arXiv 1206.5336) | multiselect bound (citing Kaligosi et al.) | A9 | theory |
| Ellmann, Neumann — One Pass to Parse Them All (PVLDB 19, 2026) https://www.vldb.org/pvldb/vol19/p3579-ellmann.pdf | fused parallel parse + aggregate with chunk-local state (from 10-29 doc) | A6, A8 | server hardware |

**MIT 6.172 (Fall 2018, OCW, CC BY-NC-SA)** —
https://ocw.mit.edu/courses/6-172-performance-engineering-of-software-systems-fall-2018/

| Lecture | Supports |
|---|---|
| L2 Bentley Rules for Optimizing Work | A3/A4 (precompute, short-circuit), D5 (degree rejection) |
| L3 Bit Hacks | A13 SWAR, A4 branch-free hour index |
| L6 Multicore Programming | A8, D7 work distribution |
| L9 What Compilers Can and Cannot Do | A1/D1 inlining, LTO, bounds checks |
| L10 Measurement and Timing | §7 benchmark protocol (variance, warm caches, trimmed means) |
| L14 Caching and Cache-Efficient Algorithms; L15 Cache-Oblivious Algorithms | A6 arena layout, D3 CSR, D8 reordering |
| L22 Graph Optimization (Julian Shun) — slides `MIT6_172F18_lec22.pdf` | CSR, BFS cache-miss analysis, bitvector visited sets, reordering "BFS, DFS, METIS, by degree" → D3, D6, D8 |

---

## H. Architecture recommendations

### Track A

**Recommended baseline — A-Arch-1: mmap + parallel fused parse + interned dense groups + arena + select.**
Pipeline: mmap → 12 LF-aligned chunks → per thread {parse, local intern, push `(lgid, lat)`, count} → merge
dictionaries (≤ 12 × 1,842 entries) → sort unique endpoints → remap → global prefix sums → parallel scatter
into arena → parallel per-hour {select, format} → concatenate, one write → `exit(0)`.
Bottlenecks: parse throughput, then scatter (random writes into 12 MB arena), then selection on head groups.
Memory [D]: pairs ≈ 25 MB + arena 12.4 MB + counts (127 K × 4 B × threads ≈ 6 MB) + output 10 MB ≈ 50 MB ⇒
M factor ≈ (228/50)^0.2 ≈ 1.35. Size: est. 180–250 lines [H].

**Alternative A-Arch-2: serial zero-copy version of the same.** Same data structures, one thread, no merge
step (~40 fewer lines). Useful as the stepping stone and fallback; expected T several × worse than Arch-1 [H].

**Alternative A-Arch-3: per-thread per-group `Vec<u32>` (no arena).** Simpler merge (append vectors per gid)
but ~127 K × threads small allocations and worse locality; consider only if the arena scatter shows up as a
bottleneck in an unexpected way.

Lower-memory variant of Arch-1: count pass first (parse once, counts only), then second parse scatters
directly into the arena — no pairs buffer (−25 MB ⇒ ≈ +1.4 points on M [D]) at the cost of parsing twice.
Decide by measurement; with 12 threads the second parse may cost only tens of ms.

### Track D

**Recommended baseline — D-Arch-1: CSR + degree rejection + array Dijkstra + 12-way parallel queries.**
Pipeline: mmap → parallel parse → CSR (+ `indeg`) → queries via atomic counter, per-thread `dist`/touched/heap
→ answers by index → one write → `exit(0)`. Memory ≈ 47 MB CSR + 7 MB indeg + 12 × 14 MB ≈ 225 MB [D] ⇒
(893/225)^0.2 ≈ 1.32.

**Alternative D-Arch-2: D-Arch-1 + DFS reordering + best heap (radix or 4-ary).** Expected to be the final
form if PHAST's layout effect transfers [L→H]. Extra ~50–70 lines.

**Alternative D-Arch-3: D-Arch-2 + bidirectional search.** Adds reverse CSR (+47 MB), second per-thread state
(+14 MB/thread) and ~60 lines. Only worth it if octave 14–16 queries dominate after D-Arch-2 and the
measured settle reduction pays for the memory (−~1.5 points on M at 12 threads [D]).

---

## I. Roadmap for the 72-hour competition (kickoff Oct 30 15:00 UTC → freeze Nov 2 15:00 UTC)

| Phase | Hours | A | D | Priority |
|---|---|---|---|---|
| 1. Correct baseline + harness | 0–6 | regenerate data from current generator commit; board run of current Rust baseline (step 1 run ID for PERF.md); phase timers on stderr behind env var; A1 build profile | same; D1 | P0 |
| 2. Algorithms & data structures | 6–24 | A3, A4, A5, A7, A9 (serial Arch-2) | D2, D3, D5, D6 | P0 |
| 3. Parsing & memory | 24–40 | A2 mmap, A6 arena, A8 parallel, A11, A12 | D4 parallel build, D7 parallel queries, D16 threads | P0 |
| 4. CPU-level tuning | 40–60 | A10, A13, A14 if profiled; A15 only with spare time | D8 reorder, D9 heap, then D10 / D12 if justified | P1 |
| 5. Validation & submission | 60–72 | multi-seed full hashes, S/D via `static_axes.py`, PERF.md, README, code cleanup for judges; no risky changes in last 6 h | same | P0 |

Budget roughly 40 % implementation of P0, 25 % measurement/validation, 20 % P1, 15 % judging artefacts.
Freeze a known-good commit on `speed-demon` before every experiment branch merges.

---

## J. Top 10 experiments per track

Common protocol: one change per experiment; correctness = byte-identical output vs reference on tiny/sample
seeds {1, 7, 42, 1234} + full seed 1 + full of one unseen seed + the `tests/` edge fixtures; performance =
5 runs on the x86 VPS via `check.py` (trimmed mean, same rule as the harness) and a board run for each kept
step; record T, M (`memory.peak`), S (`static_axes.py`), D. **Keep if** points (§B formula) increase beyond
noise (> 5 % T or the 50 ms floor), else revert.

### Track A

| # | Hypothesis | Change | Bottleneck | Measure | Keep if |
|---|---|---|---|---|---|
| A-E1 | Build flags give free speed | A1 | codegen | T | T ↓ with S unchanged |
| A-E2 | Per-line allocation dominates baseline | A3 + A4 on `BufRead::fill_buf` (serial) | allocation/parse | T, allocs | T ↓ |
| A-E3 | String-keyed hash dominates next | A5 + A7 dense gid | hashing | T, phase timer | T ↓ |
| A-E4 | Full sorts are a visible phase | `sort_unstable` | finalize | finalize timer | phase ↓ |
| A-E5 | Selection beats sort on head groups | A9 threshold sweep {16,32,64,128,∞} | finalize | finalize timer per size bucket | best threshold |
| A-E6 | mmap costs no M and enables zero-copy | A2 | I/O, M | M, T | M ↓ and T not ↑ |
| A-E7 | Parse scales with cores | A8, threads ∈ {1,2,4,6,12} | parse | T vs threads, page faults | best count |
| A-E8 | Arena beats per-group Vecs | A6 vs Arch-3 vs two-pass variant | merge/M | T, M, points | max points |
| A-E9 | Output formatting is now visible | A10 + A11 + A12 | output | output timer | T ↓ ≥ cost of lines |
| A-E10 | SWAR / ILP help after the above | A13, then A15 | parse IPC | perf stat IPC, branch-misses | points ↑ net of S |

### Track D

| # | Hypothesis | Change | Bottleneck | Measure | Keep if |
|---|---|---|---|---|---|
| D-E1 | Build flags help | D1 | codegen | T | T ↓ |
| D-E2 | HashMap distances dominate queries | D6 (serial, still adjacency lists) | hashing | query timer, settles/s | T ↓ |
| D-E3 | Two whole-graph searches are waste | D5 | query | query count settled | T ↓ |
| D-E4 | String parsing dominates load | D2 + D3 | parse/build | load timer, M | T, M ↓ |
| D-E5 | Queries scale across threads | D7, threads ∈ {1,2,4,6,12} | query | per-thread busy, tail | best points (T vs M) |
| D-E6 | Parallel build pays | D4 (a) vs (b) | build | build timer, transient M | max points |
| D-E7 | Random layout costs ~2× | D8 DFS vs BFS order | cache misses | settles/s, perf cache-misses, total T incl. reorder | T ↓ net |
| D-E8 | Heap choice matters after reorder | D9: binary vs 4-ary vs radix | heap ops | heap ops, T | T ↓ ≥ line cost |
| D-E9 | Long queries benefit from bidirectional | D10 | settled nodes, octave 14–16 | settles per octave, M | points ↑ net of M |
| D-E10 | Smaller dist arrays help cache/M | D12 | cache, M | T, M | points ↑ |

### Isolating phases without corrupting the official number

Add `SD_TRACE=1`-gated `Instant` timestamps to stderr for: map/read, parse, build/merge, select/query,
format, write. Never let tracing run in board submissions' hot loops (check by building with tracing compiled
out via a `const` flag). Microbenchmarks (hash lookup rate, settle rate) are for direction only; decisions are
made on end-to-end trimmed-mean T and the full K.

---

## K. Final ranked shortlist

> **"If I had only 12 hours, these are the optimizations I would implement first."**

### Track A (in order)

1. **A1 build profile** — free, zero S. Measure T. Risk: none. Ref: Perf Book build-configuration.
2. **A3 + A4 hand parser with 4-byte hour index** — removes per-line `String`/`Vec` and the `(String,String)`
   key allocation; prerequisite for everything. Measure T, allocations. Risk: latency 10 digits, `5xx` test.
   Ref: spec §2–3; thomaswue parser structure.
3. **A5 + A7 interning with byte-sorted global IDs** — turns 3.1 M string hashes on composite keys into cheap
   word hashes on a 1,842-entry table, and makes output order free. Measure T. Risk: full-key compare;
   tiny catalogue < 1,842. Ref: rustc-hash, thomaswue `findResult()`.
4. **A2 mmap + A8 12-way chunked parsing** — the big multiplier on the dominant phase; mmap keeps M low.
   Measure T vs threads, M. Risk: chunk boundaries, empty file. Ref: Koerkamp 1BRC log, thomaswue.
5. **A6 arena + A9 sort_unstable/select cascade** — removes 127 K allocations and full sorts. Measure M,
   finalize timer. Risk: rank formula. Ref: Rust slice docs.
6. **A11 + A12 output** — once the rest is fast, 10 MB of formatting matters. Measure output timer. Risk:
   leading zeros. Ref: itoa source.

### Track D (in order)

1. **D1 build profile** — free.
2. **D5 degree rejection** — 5 lines, removes ≈ 14 % of query work (two full-graph searches). Risk: `s==t`
   ordering. Ref: spec §2 query table.
3. **D6 array distances + touched reset + reused heap** — removes per-query `HashMap` (the main cost of the
   Rust baseline's 21 s). Measure settles/s. Risk: stale entries. Ref: generator `rank_target`, fast_paths.
4. **D2 + D3 mmap + byte scanner + `u32` CSR** — load and memory. Measure load timer, M. Ref: MIT 6.172 L22.
5. **D7 parallel queries with atomic counter, 6 vs 12 threads** — biggest multiplier on the query phase;
   dynamic scheduling because octave-16 queries are half the work. Measure tail, M. Ref: §B sandbox facts.
6. **D8 DFS reordering** — the strongest literature-backed single change for randomly-labelled road graphs.
   Measure settles/s and net T including reorder. Risk: unreached nodes need IDs; query remap. Ref: PHAST TR
   Table 1.

Everything else (heap variants, bidirectional, u32 distances, SWAR, ILP, PGO) waits for a profile of the
result of these steps.

---

### Appendix — correctness pins (do not deviate)

- A: rank `k = (p·n + 99) / 100` (1-indexed) for p ∈ {50, 95, 99}; `ppm = errors·1_000_000 / count`
  (integer floor, u64); error ⇔ 500 ≤ status ≤ 599; hour = truncation; endpoints compared/sorted as raw
  bytes, prefix first; empty input ⇒ zero-byte output; duplicates count.
- D: `s == t ⇒ 0` always; unreachable ⇒ `-1`; distances `u64` with no saturation in the exact path; stop on
  **pop** of `t`; parallel edges and self-loops need no special handling; output line i answers query line i.

### Appendix — PERF.md plan

Keep the existing table (`Step | Change | Run ID | T (s) | M (MB) | Points`) and add, below it, one short
entry per step: *Hypothesis · Change (commit) · Correctness (seeds/scales hashed) · Measurement (board run ID;
local VPS trimmed mean marked "local") · Conclusion (kept/reverted, why)*. Log reverted experiments too —
judges score the narrative (8 %), and negative results (e.g. "SWAR gave < noise, reverted for S") show
technique.
