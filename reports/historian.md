# Track A historian report (2026-11-01, read-only)

Tags: F = fact verified from a file/feed; I = inference; M = missing evidence.
State at start: HEAD 6d5802b (tag best-board-6d5802b), branch speed-demon. track-a/ and src are clean. Untracked: .codex/, AGENTS.md and four tests/performance dirs (flamegraphs, chunks4, competitor-research, rl-lyceum). Those are evidence only, not source.

## 1. Best verified board version vs best local candidate

I downloaded the live feed read-only (https://s3.amazonaws.com/raptors.dev/speeddemon.dev/2026/board/board.json, generated 2026-11-01T11:06:56Z). It shows only each team's best and latest entry per track.

| Entry | Commit | Run ID | T (s) | M (B) | S | K | Points | Source |
|---|---|---|---|---|---|---|---|---|
| **Feed BEST** | be5ed24 | run-20d64f688eec | 0.031212 | 20,377,600 | 550 | 36.868765 | **78.332929** | live feed (F) |
| Feed latest | 6d5802b | run-9e5d2dfb4e65 | 0.031695 | 20,561,920 | 554 | 36.481259 | 78.103491 | live feed (F) |
| Earlier first run of the same source | ea2ee34 | run-7ad2f004409d | 0.034436 | 20,516,864 | 550 | 35.052698 | 77.236072 | PERF.md and archived board.json, 10:15Z (F) |

- **be5ed24 is a docs-only commit (F).** `git diff ea2ee34 be5ed24 -- track-a tests/ingest-parser.rs` shows only PERF.md changed. So run-20d64f688eec is a **rerun of the ea2ee34 source**, and it is the highest board score we have.
- **Contradiction with the brief.**
  - best-board-6d5802b is not the best board result. It scores 0.23 pt below the rerun of the older source.
  - The stale "77.26" in the brief is also wrong. Evidence supports 78.33 as the board high-water mark and 78.10 as the latest.
- **Board effect of AVX2 + haswell is unresolved (I).**
  - The claimed "+0.85" compares 6d5802b against one unlucky ea2ee34 run.
  - The same source scored 77.236 and 78.333 on two board runs, a 9% T spread (0.0344 vs 0.0312).
  - Against the better run, the effect is -0.23 pt.
  - The local estimate was only -4% instructions and -5% task-clock.
  - AVX2 added about 10 counted lines (S 554 vs the 550/544 expected without it). That costs roughly 0.14 pt (I).
- **5878e1d (claimed 77.258) cannot be verified (M).**
  - The feed keeps only best and latest, and neither is 5878e1d.
  - No saved JSON contains "77.258".
  - PERF.md stops at step 13 and has no rows for 5878e1d, 6d5802b, or the be5ed24 rerun.
  - The run ID for 5878e1d was not found anywhere in the repo, so I record none.
- **Verified in PERF.md only, with no raw board JSON archived (M).** The run IDs are in PERF.md (F) and the PERF.md figures were not re-checked against the feed:

| Step | Commit | Run ID |
|---|---|---|
| 10 | db59a56 | run-17eeea974461 |
| 11 | fbc79ec | run-3b925c36564d (T 0.040816, M 30.25 MB, 73.74 pt) |
| 12 | 8188235 | run-220bc44d8e29 (T 0.039775, 74.00 pt) |

  - The docs call out that the 01bf7d2 board JSON (run-3dde37561ccb) is missing from the local archive.
  - Only the ea2ee34 run has archived raw JSON (tests/performance/2026-11-01-track-a-competitor-research/board.json and summary.json).
- **Rank and cutoff.**
  - The archived 10:15Z feed puts ea2ee34 at 38th. In the 11:06Z feed our best (78.33) ranks about 35th of 117.
  - Cutoff #10 is Lex at 84.737 pt (T 0.016902, M 20.9 MB, S 575). #11 Cache Me is 84.655.
  - Top 1 is Cause Of Death at 87.756 (T 0.016091, M 17.26 MB, S 346, Rust).
  - The brief's ~84.7 is right for now. Older docs say 84.3/84.4/84.5 because they were written against earlier snapshots.
- **Best locally measured candidate.**
  - There is none newer than 6d5802b (F). The working tree is clean for track-a.
  - Its local evidence is in docs/track-a-research-2026-11-01.md, section "AVX2 + target-cpu=haswell". On rl-lyceum perf stat (full seed 1, one thread), 1,256.0 M instr / 179 ms at 5878e1d became 1,206.0 M (-4.0%) / 168.6-170.7 ms (-5%).
  - VPS docker A/B (2x9 runs) showed 0.0871 / 0.0823 s vs 0.1003 / 0.0818 s, within noise.
  - Output was byte-identical on the datasets listed there, including 9 VPS runs of full seed 1.
  - Because the board did not clearly confirm it, "best candidate" and "latest" are the same source. The better board number belongs to the older ea2ee34 source.

## 2. Mechanisms attempted

Local (L) and board (B) results are separate. Local measurements are on the rl-lyceum i7-7700 (instructions, perf) and the VPS (6-vCPU Haswell KVM, docker), full seed 1 unless noted.

| Mechanism | Commit | Status | Evidence and scope | Gaps |
|---|---|---|---|---|
| Rust port, reuse buffer, key allocation | 55522c4, 3897216, ff39a9a | Accepted | VPS full seed 1: -21% each. Board: 1.40 -> 1.09 -> 0.94 s. | Steps 3 and 4 have no public run ID. |
| Buffered byte parser | a2217a2 | Inconclusive, kept (L) / loss (B) | VPS +3.4% and +1.1% with 15-24% spread. Board 0.9385 -> 0.9596 s, 42.88 -> 41.17 pt. | Cause not isolated. |
| Interning | 531def7 | Accepted | VPS -5.8% / -3.7% with 9-28% spread. Board 0.876 s, run-12ff8c772294. | |
| Hour-ID cache | in 69aefa2 | Accepted | VPS -9..-11%. Alternating-hour synthetic input regressed 4-10%. | No board number alone. |
| mmap + parallel chunks | 69aefa2 | Accepted | Board run-6d2d50ab6cf7, 0.187 s. VPS scaling at 1/2/6/12 workers. | Profiles describe 69aefa2 only (stale). |
| Fixed-layout parser, custom table, sorted keys | 14001f9 | Accepted | Board run-7893403a45a6, 0.0726 s. | Bundled, mechanisms not isolated. |
| SWAR, selection quantiles, streamed output | 70c6a36 | Accepted | Board run-2f867ed017e2, 0.0497 s. | Bundled. |
| Exact-size hour buckets, hash-only table, in-worker writes | 01bf7d2 | Accepted | Board run-3dde37561ccb, 0.0448 s, 71.67 pt. | Raw JSON missing. No local A/B. |
| Per-chunk grouping, free before output | db59a56 | Accepted | Board run-17eeea974461, M 46.6 -> 28.4 MB. | |
| Newline-first SSE2, hour run cache, parallel unmap | fbc79ec | Accepted | Board run-3b925c36564d. Instr/record 1.40 G -> 1.37 G (-2%) but T 0.0441 -> 0.0408 s. | S rose 486 -> 544 and ate most of the gain. |
| madvise instead of munmap | 8188235 | Accepted | Board run-220bc44d8e29: 0.0408 -> 0.0398 s. | Gain was a single run. |
| Shared lock-free table, 4-byte records, mapped output | ea2ee34 | Accepted | VPS T 0.105 -> 0.094 s, M 25.7 -> 20.0 MB. Board 0.0398 -> 0.0344 s, +3.2 pt (run-7ad2f004409d). | Rerun of the same source gave 0.0312 s. |
| Chunks x8 -> x4 | in worktree only | Inconclusive, rejected for promotion | VPS -0.85%, Lyceum -3.02%, intervals cross 1. VPS one-worker +5.6%. Full seed 1 only for the headline numbers. Base db59a56. | Not retested on the current architecture. |
| Byte comparison in endpoint table | | Rejected (L) | "About 10% of parse time" per PERF.md. No raw evidence found. | Quoted figure unverified. |
| 3-word endpoint hash (first+middle+last+len) | be5ed24 | Rejected | Offline: 49 colliding pairs over 300 seeds (16% of seeds). | Collision test only, no timing. |
| 4/5-word hash variants | | Not adopted | 0 collisions over 300 seeds. Saves about 3% parse instructions. Statistical argument only. | |
| 2-word hash + SSE2 compare | 2926a22 (tests only) | Rejected | Instr +9% (1.277 G -> 1.391 G). VPS 0.0915 -> 0.0976 s (+7%, 9 runs). | |
| Iterator chains -> loops in hour phase | 9d16b5e | Rejected, no effect | Instr 1,277,840,205 vs 1,277,696,543. Task-clock 176.9 vs 177.7 ms. VPS within noise. | |
| number() with known digit count, -6 lines | 5878e1d | Accepted as a trim | Instr 1,277.7 M -> 1,256.0 M (-1.7%). Output identical at 1/3/12 threads. | Board result unverified (see section 1). |
| Drop release()/kept/finished | not committed | Rejected | VPS M 19.6 -> 23.5 MB. Estimated net -0.5 pt. | |
| push_number -> write!, padded tail mapping | not tried | Dismissed by estimate | Under 0.15 pt each. Never measured. | |
| Three-cursor 1BRC interleave | prototype dropped | Rejected | Instr +14% (1,459 M), task-clock +8-10%. | |
| AVX2 newline scan + target-cpu=haswell | 6d5802b | Accepted | See section 1. | Board gain unresolved. |
| O_DIRECT output | | Dismissed | Reasoning only: /out is tmpfs. | Not measured. |

Count of recent non-winning local experiments: the loops, S-trim and interleave were logged as three negative or inconclusive results in a row. AVX2 then counted as accepted on a -5% local gain. Under AGENTS.md the three-failure rule has been satisfied once and "fresh research" is what this round is.

## 3. Measurement confounders and contradictions

- **Board single-run noise is about 9%, not the 4% the docs claim.**
  - Same source ea2ee34 gave T 0.034436 and 0.031212, a 10% spread. PERF.md states +-4%. The docs say to resample under 10%, and the AVX2 gain is below that.
  - 6d5802b has one board sample. Its apparent gain over ea2ee34's best is negative, and over its first run positive. Neither is a controlled comparison.
- **VPS vs board.**
  - VPS full-run T is about 0.09-0.10 s against board 0.031 s. VPS is 3x slower and oversubscribed, so it cannot resolve changes under about 10% (PERF.md says so).
  - VPS spreads in the logs run from 8% to 28%. AVX2's VPS A/B is within noise and only the instruction count showed a difference.
- **Statistic changes between experiments.**
  - Trimmed means (drop one min/max) were used for most VPS comparisons, medians for rl-lyceum profiling, and a single run per board push.
  - The board's official protocol is "max-of-five M" and runs_per_push is 1 (feed `rules`).
  - Min vs trimmed mean was never compared on the same data.
- **Instruction counts vs time.** AVX2 -4% instructions gave -5% time on Lyceum. Step 11 gave -2% instructions but -7.5% board T. Step 12 (madvise) changed no instructions. Instructions are an unreliable proxy for kernel-side costs, which are 10-18% of cycles (page-cache mapping and unmapping).
- **CPU sets and topology.** VPS used affinity 0-5 on a shared 6-vCPU box. Lyceum has 4 cores / 8 threads. The board's worker count is unknown (docs say 12 workers on 6 P-cores with SMT, from Track D). Worker defaults were tuned on neither.
- **M accounting.**
  - /out is a tmpfs, so the 10.1 MB output is charged to memory.peak (F, PERF.md and research log).
  - THP is disabled via prctl and mallopt (F, README).
  - VPS M (19.2-20.0 MB) tracks board M (20.4-20.6 MB) closely. Single board M values differ by 0.2 MB between runs.
  - RSS numbers in the old profiles include the mapped input and are not M.
- **S.**
  - Local line counts differ from the board. The research log's "542 -> 536" vs board 550 (ea2ee34) and 554 (6d5802b) suggests an 8-line offset.
  - The AVX2 commit added about 10 board-counted lines.
  - Competitors with similar T have S 294-346, and each S line is about 0.014 pt.
- **Stale and conflicting statements.**
  - PERF.md still says "Current best: step 5".
  - README.md still describes the pre-ea2ee34 design (per-worker tables, "no SIMD yet").
  - PERF.md has no rows for steps 14-16.
  - The competitor doc (written at ea2ee34) says output is built in per-hour Vec<u8> and copied. PERF.md says ea2ee34 formats straight into the mapped file. Source still has `push_number(text: &mut Vec<u8>, ...)`, so direct formatting is probably partial (I).
  - The competitor doc says there is no profile of current source. Only the research log's perf stat numbers (265 instr/record, parse 0.82 G of 1.27 G) cover it, and no raw files back them.
- **Correctness risk (F, in source).**
  - ea2ee34 and later use a hash-only endpoint table (no byte comparison).
  - The chunks4 experiment found this gap and restored equality only in its worktree.
  - The offline check is a 300-seed generator catalogue (1,842 names per seed, 0 collisions). That is a statistical argument, not proof, and the spec requires exact byte identity.
- **Environment.** Docker/tmpfs memory behavior on the board and its kernel (7.0 on the VPS vs 4.15 on Lyceum) differ. Lyceum is the instruction-count host only.

## 4. Untested mechanisms (not tried and not ruled out)

1. **Resampling.** Several board pushes (or best-of-N reruns) of the same commit to separate source effects from the 9% noise. Cheapest path to the extra ~0.2 pt, given the best score is already a noise outlier.
2. **Sampled CRC32 (SSE4.2) hash with exact byte equality.** The two-word + SSE2 compare was rejected, but a CRC hash needs fewer instructions than the multiply-rotate chain. Combined with the existing haswell target, it adds no new dependency. Would also close the correctness gap.
3. **Worker/chunk tuning on current code.** `workers * 8` is untested since ea2ee34. VPS oversubscription means the board topology is the real unknown.
4. **Persistent worker pool.** The code spawns three pools (parse, hours, text) via `run`. Startup cost was never measured.
5. **Direct bounded formatting into the mapped output.** `push_number` still builds a Vec<u8> per hour.
6. **Scratch allocation.** `vec![0_u32; most]` per hour worker: untouched pages may be lazily zero, so M effect is unknown.
7. **Source simplification beyond the 6 lines.** Candidate lines are tiny each, so only a multi-line removal would matter. The release/kept/finished removal is already a measured loss.
8. **Streaming and early hour close.** Spec gives no disorder bound, so it needs an exact fallback. Competitors report mixed results.
9. **Six-byte records.** Obsolete: records are already 4 bytes.
10. **C++ port for S.** Public C++ entries reach S 294-315. Our Rust design is at 550. No evidence a language change alone helps.
11. **Prefetch and SIMD selection.** Never profiled on current source.
12. **Profile of current source.** The only flamegraphs are for 69aefa2. A perf record of 6d5802b is missing.

## Evidence paths
- track-a/PERF.md
- docs/track-a-research-2026-11-01.md (local experiments after ea2ee34)
- docs/track-a-competitor-research-2026-11-01.md
- tests/performance/2026-11-01-track-a-competitor-research/board.json (10:15Z feed, ea2ee34 only)
- tests/performance/2026-11-01-track-a-chunks4/README.md
- Live feed (11:06:56Z): https://s3.amazonaws.com/raptors.dev/speeddemon.dev/2026/board/board.json. The run IDs run-20d64f688eec and run-9e5d2dfb4e65 are not in any file in the repo.
