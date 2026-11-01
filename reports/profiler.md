# Profiler report, Track A, HEAD 6d5802b (tag best-board-6d5802b), 2026-11-01

Scope: measurements only. No tracked file, git state or board state was touched. Raw evidence, scripts and
patches for the temporary instrumented copies: `tests/performance/2026-11-01-track-a-review/` (every file below is
in that directory unless stated). Instrumented/prototype sources lived only in the session scratchpad and on the
remotes; their diffs are saved as `*.patch.txt`.

Label convention: **[M]** measured, **[E]** estimate, **[I]** inference from measurements.

## 0. Identities and conditions

* Source: `track-a/src/main.rs` sha256 `d26def2a...a0b0`, `Dockerfile` `5c01eee9...`, working tree == HEAD (`git status` clean for track-a). Previous commit 5878e1d `main.rs` `2d55125e...` (generic x86-64, SSE2 newline scan, via `git archive`).
* VPS `my-vps`: KVM guest, 6 vCPU "Haswell, no TSX" (AVX2/BMI2/AES present), kernel 7.0.0-28, THP `madvise`, **no hardware perf counters** (only cpu-clock/software events), shared with production containers (victoria-metrics, grafana, ...). Load average 0.3-1.4 during runs. Docker 29.7.2, **systemd cgroup driver**.
* rl-lyceum: i7-7700 (4C/8T, Kaby Lake, AVX2), kernel 4.15, HW counters available. Used natively, no docker.
* Board: 6 P-cores/12 threads, 78.10 pts (T 0.031695 s, M 20.56 MB, S 554). The VPS is about 2.6x slower than the board in wall time and has half the threads: **VPS absolute times are not board times.**
* Build: exactly the committed Dockerfile (rust:1-slim-trixie, `-C target-cpu=haswell`, LTO fat, cgu 1, abort). Instrumented copies = HEAD source + marks (`inst.patch.txt`, `inst2-memory-marks.patch.txt`, `inst3-rusage-marks.patch.txt`), same flags. Marks cost about 10 `Instant::now()` calls plus 3 per hour; outputs verified identical (sha `495ca096...`).
* Timed boundary everywhere unless stated: exec to exit by `sd-timer` (as the board), warm page cache (`cat` + 1 untimed run), docker flags identical to `harness/run_submission.py` (cpuset 0-5, 8g, read-only, tmpfs /tmp, `--ulimit nofile`). Input: harness `full-seed1` (238,836,234 B, sha `7ec80ba4...`) unless named.
* One hazard found: `tools/bench.sh` mounts `/out` from an ext4 host directory, the board uses a tmpfs work dir. My runs use `/dev/shm` (tmpfs) except where marked "ext4". Text phase was about 3 ms slower on ext4 (9.3 vs 6.2 ms at 6 threads, 9.2 vs 7.5 at 12; the two sets were taken in different sessions, so the delta is hedged but its direction is the same at both thread counts) [M, `stages-docker-*` vs `stages-tmpfs-*`].

## 1. Baseline reproduction (task 1)

`tools/bench.sh profreview 0-5 15` (unchanged script, ext4 out, 15 runs, `bench-sh-head-15runs.txt`, `bench-sh-stats.txt`):

| | min | p25 | median | mean | trimmed mean (drop min,max) | max | (max-min)/median |
|---|---|---|---|---|---|---|---|
| wall s | 0.0744 | 0.0806 | 0.0882 | 0.0887 | 0.0873 | 0.1216 | 54% |

M (max cgroup memory.peak) 19.17 MB; `memory.stat` at exit: anon 0.28 MB, file 10.12 MB (the output), kernel 1.3 MB, pagetables 0.54 MB. Hash `495ca096...` on all runs.

Interleaved A/B, HEAD vs 5878e1d, 20 alternating rounds each (order flipped each round; `ab-bench.py`, `summarize_ab.py`, `summary-tables.txt`):

| condition | HEAD median / trimmed mean / min | 5878e1d median / trimmed mean / min | paired diff HEAD-prev (median, 95% bootstrap CI) | HEAD faster in |
|---|---|---|---|---|
| 6 vCPU, tmpfs out | 0.0843 / 0.0858 / 0.0665 | 0.0870 / 0.0855 / 0.0634 | +1.0 ms [-10.5, +13.0] | 9/20 |
| 6 vCPU, ext4 out | 0.0858 / 0.0866 / 0.0683 | 0.0832 / 0.0841 / 0.0725 | +3.8 ms [-6.8, +14.4] | 8/20 |
| 1 thread, cpu 3, tmpfs | 0.3215 / 0.3232 / 0.3003 | 0.3226 / 0.3242 / 0.2966 | +4.8 ms [-10.8, +14.0] | 7/20 |

Peak memory 19.2 (HEAD) vs 19.6 MB (prev) tmpfs [M, single maxima]. Official-harness batches (section 4) on HEAD: T 0.0817-0.0922 s.

**Reading:** the VPS cannot resolve the AVX2 effect. Within-condition spread is 40-60% (max/min) at 6 threads and 16-18% even at 1 pinned thread; the CI of the paired difference is about +-12 ms against an expected effect of 1-4 ms. Not "no effect", just unmeasurable here. The expected effect is visible only in counters on rl-lyceum (1 thread, `perf-stat-rl-lyceum.txt`, 10 runs each, +-0.0-0.3%):

| rl-lyceum, 1 thread, cpu 2 | cycles:u | instr:u | task-clock |
|---|---|---|---|
| 5878e1d (generic build) | 590.7 M | 1,168.7 M | 181.5 ms |
| 5878e1d source + haswell flags | 588.2 M | 1,136.6 M | 182.1 ms |
| HEAD (haswell + AVX2 newlines) | 551.5 M | 1,118.3 M | 175.0 ms |

Docs/research notes quote 1,256.0 M and 1,206.0 M instructions for 5878e1d and HEAD: those count user+kernel (my `:u` 1,168.7 M / 1,118.3 M plus 88 M `instructions:k` gives the same numbers), so there is no contradiction. All rl-lyceum counters come from native builds with rustc 1.99.0 and the committed flags (`RUSTFLAGS=-C target-cpu=haswell`, same Cargo profile), not the docker binary (trixie glibc does not run on xenial). HEAD vs generic: -6.6% user cycles, -4.3% user instr, -3.6% task-clock [M]. At 8 threads user cycles 811.5 M vs 879.4 M (-7.7%) but wall 0.131 vs 0.132 s with +-11-15% run spread [M]: unresolved.

Noise notes: VPS drift between sessions is larger than within sessions: `perf stat` native loop gave 73 ms elapsed at one moment and board-like runs 84-88 ms minutes later, same binary. Docker adds nothing measurable: same binary under sd-timer natively (uid 0, taskset 0-5) median 0.0835 vs docker 0.0858 s, 20 alternating rounds each (`native-vs-docker-full-seed1.jsonl`), differences inside noise.

## 2. Where the time goes (task 2)

### 2.1 Stage breakdown, wall clock, VPS docker, tmpfs out, medians (`stages-*.jsonl`, `summarize_stages.py`)

| ms | exec to first mark | parse | rank sort | hours | sort/drop/set_len/mmap | text format | post | exec+exit (wall minus last mark) | wall |
|---|---|---|---|---|---|---|---|---|---|
| threads 12 (20 runs) | 1.3 | 48.7 (57%) | 0.6 | 21.0 (25%) | 0.7 | 7.5 (9%) | 0.5 | 3.1 (4%) | 85.0 |
| threads 6 (20 runs) | 1.0 | 54.9 (62%) | 0.6 | 20.9 (24%) | 0.4 | 6.2 (7%) | 0.5 | 3.4 (4%) | 87.9 |

Fixed or serial pieces (pre-parse, rank, mid, post, exec+exit) total about 6 ms [M], about 7% of VPS wall. If the board shares the proportions (unmeasured) that is about 2 ms of 31.7 ms [E].

### 2.2 Thread scaling (VPS, cpuset n = threads, 12 runs each, tmpfs; `scaling-threads*.jsonl`) and Amdahl

| threads | wall ms | parse | hours | text | serial pieces | speedup vs 1 |
|---|---|---|---|---|---|---|
| 1 | 323.6 | 215.5 | 76.4 | 26.5 | 4.2 | 1.00 |
| 2 | 188.6 | 123.5 | 44.7 | 14.3 | 5.0 | 1.72 |
| 4 | 110.9 | 67.6 | 27.0 | 8.3 | 5.9 | 2.92 |
| 6 | 88.6 | 54.5 | 20.8 | 6.1 | 5.9 | 3.65 |

Amdahl fit gives an effective serial fraction of 13% at 6 threads, but only about 6 ms (7%) is genuinely serial. The rest is parallel-phase efficiency: total CPU (perf stat, native) rises from 306 ms (1 thread) to 349 ms (6 threads), +14%, parse 216 ms single-thread vs 274 ms thread-summed at 6 [M]. Cause not attributed: candidates are mmap_lock/page-fault contention, hypervisor vCPU sharing, memory bandwidth. rl-lyceum (4C/8T, native, 15 runs, `stages-rl-lyceum-native.txt`): in-process 170.6 / 91.1 / 54.9 / 37.6 / 40.7 ms for 1/2/4/8/12 threads; at 8 threads parse 21.8, hours 11.3 (30%), text 2.7 ms. 12 threads on 8 logical CPUs is slower than 8 (40.7 vs 37.6), mostly hours (13.8 vs 11.3) [M]. Hours phase has a tail: thread-summed work 71 ms/8 = 8.9 ms ideal vs 11.3 ms wall [M]; a simulation of the dynamic hour queue with cost proportional to records gives +6%/+9%/+11% over ideal at 6/8/12 threads (`hour-imbalance-model.txt`) [E, cost model unverified].

### 2.3 CPU split (CPU time summed over threads, VPS docker, 6 threads, 15 runs, `rusage-stages-threads*.jsonl`)

| stage | user ms | sys ms | minor faults |
|---|---|---|---|
| parse | 250.9 | 36.3 | 5,833 |
| hours | 87.3 | 11.8 | 2,021 |
| text | 21.5 | 12.8 | 2,664 |
| total before exit | 360.4 | 65.1 | 10,708 |

(12 threads: user 373, sys 72, faults 11.5 k.) Kernel time is about 15% of CPU; inside text format it is 37% of the stage's CPU (mmap'd tmpfs output: 2.66 k faults, about 10 MB / 4 KB; 4.8 us sys per fault on this VM). Exec and exit teardown are outside these marks (3.1-3.4 ms wall). Hardware view, rl-lyceum 1 thread: kernel 118 M cycles vs user 551 M (17.6% of cycles), kernel IPC 0.75; at 8 threads 186 M vs 811 M (18.7%) [M]. Software perf record on the VPS (`perf-record-vps-cpuclock.txt`, 25 runs, cpu-clock 10 kHz, 6 threads): `bin-cur` user 79%, kernel 16.6%, libc 4.3%; by symbol `State::scan` 55.5%, hours closure 20.1%, text closure 2.7%, memmove 2.8%; kernel top entries do_user_addr_fault 2.0, kernel_init_pages 1.9, next_uptodate_folio 0.6, zap_present_ptes 0.55 (the listed kernel symbols sum to about 9% of the 16.6%; the long tail is unattributed). cpu-clock sampling in a VM has skid; use for ranking only.

### 2.4 Hardware counters, rl-lyceum, full-seed1, HEAD build (`perf-stat-rl-lyceum.txt`, `perf-record-rl-lyceum-1thread.txt`, `perf-precise-rl-lyceum-1thread.txt`)

| | 1 thread (cpu 2) | 8 threads (cpus 0-7) |
|---|---|---|
| instructions:u | 1,118 M (361 per record) | 1,119 M |
| cycles:u | 551 M, IPC 2.03 | 811 M, IPC 1.38 |
| instructions:k / cycles:k | 88 M / 118 M | 101 M / 186 M |
| branch-misses:u | 6.03 M (3.8% of branches, 1.95 per record) | 6.05 M |
| cache-misses:u (refs) | 6.6 M (14.0 M) | 6.9 M (15.6 M) |
| page-faults, context-switches | 11.3 k, 14 | 12.3 k, 670 |

User cycles per CPU rise 47% from 1 to 8 threads (SMT sharing of 4 cores; IPC 2.03 to 1.38) [M]; on the board (6 cores/12 threads) a similar SMT penalty is plausible but unmeasured.

Attribution (1 thread, `cycles:u`, symbol level): `scan` 64.9%, hours closure 25.8%, text closure 3.4%, memcpy 2.5%, kernel page_fault entry 1.7% (user-mode only counters). `branch-misses:u` by symbol: scan 69.7%, hours 23.9%, text 2.5%. Precise (PEBS `:upp`) branch-miss attribution to source lines: `main.rs:248` (the endpoint hash `while at.add(8) <= tail` loop exit) **46%** (about 2.8 M, 0.9 per record), `main.rs:193` (newline-mask refill `if mask != 0`) 15%, `main.rs:107` (table probe `found == hash`) 7%, `cmp.rs:2144` (quickselect comparisons) 7%, the rest in sort/select internals. Endpoint trip count (len/8) has entropy 2.9 bits, modal share 26%, equals previous line's in 15.6% (`endpoint-len-dist.txt`), consistent with an unpredictable loop exit. Precise cycles attribution is diffuse: top line `main.rs:37` (end of the AVX2 newline scan) 8.4%, nothing else above 7%. I did not attribute stalls to instructions beyond this.

Selection groups (seed 20261101 output): 127,023 groups, median 6 records, 90.6% of groups hold 28% of records (size <= 32); groups > 128 records are 2.3% of groups and 55% of records (`group-size-dist.txt`).

### 2.5 Memory trajectory (VPS docker, 6 threads, 8 runs, `mem-trajectory-threads6.jsonl`; `memory.current`/`memory.peak` at marks, MB)

| mark | current | peak so far | anon | file |
|---|---|---|---|---|
| parse_end | 16.8 | 16.8 | 14.9 | 0 |
| hours_end | 8.6 | 18.1 | 6.6 | 0 |
| text_start | 8.0 | 18.1 | 6.5 | 0 |
| text_end | 17.9 | 18.6 | 6.0 | 10.1 |

There are two near-equal peaks: during the hours phase (records not yet released plus count/lats/cursor arrays) 18.1 MB, and at the end of text (10.1 MB output file pages plus about 6 MB residual anon) 18.6 MB. Residual anon at text start is not fully attributed; the `hours` rows alone are up to 127 k x 24 B = 3.0 MB [E] plus per-thread buffers/arenas. Board M is 20.56 MB (VPS 18.3-19.9 depending on /out filesystem).

## 3. Correctness and official harness (task 3)

* New seeds, none previously used (used: 1, 7, 42): **20261101** at tiny/sample/full and **987654321** at sample, generated by `harness/generators/a_ingest.py` on the VPS (full-seed20261101 `requests.log` sha `7230a05d...`, 240,352,835 B; sample sha `6010ffa7...`; tiny `3ae70ac6...`). Reference = `harness/reference/a_ingest.py` run directly (python 3.14; the docker image was not rebuilt; same module, and it reproduces the calibration hash on seed 1 via the harness runs below).
* `correctness-vps.txt`: committed HEAD binary (docker-built) vs reference, byte-identical (cmp) at threads 1, 2, 3, 6, 12, 16 on: tiny/sample/full 20261101, sample 987654321, `edge-cases`, `empty`. 36 of 36 identical. Full 20261101 output sha `7e632eb5e25d015275a769b992d8bd38f9489910b6afe14e3c030bcad4e0e5bc` (10,246,441 B).
* Official harness `harness/harness/run_submission.py` is **not runnable unmodified** on this VPS: it reads `memory.peak` of a parent cgroup `/sys/fs/cgroup/speeddemon/run-*` that only survives with docker's cgroupfs driver; this host uses systemd (failed at `peak_and_remove`). I did not change docker. I ran a copy with exactly two edits (drop `--cgroup-parent`, read `memory.peak` from inside the container into `/sd/peak.txt` as `tools/bench.sh` does; `harness-patch.diff`, `patch_harness.py`). Everything else is the official code path: allowlist/Dockerfile check, offline build, prewarm, 5 strictly serial runs, sd-timer, sha256 gate, `docker diff` gate, middle-3 mean, max peak, spread rule. cpuset 0-5 (the board uses 12), tmpfs work dir.

| batch | input | gates | T s | M MB | run walls |
|---|---|---|---|---|---|
| 1 | full-seed20261101 | all pass | 0.0821 | 19.39 | 0.0651 0.0910 0.0842 0.0712 0.0921 |
| 2 | same | pass | 0.0922 | 19.65 | 0.1122 0.0812 0.0818 0.0827 0.1169 |
| 3 | same | pass | 0.0884 | 19.42 | 0.0955 0.0734 0.0878 0.0998 0.0819 |
| 4 | same | pass | 0.0837 | 19.73 | 0.1090 0.0735 0.0820 0.0809 0.0881 |
| 5 | full-seed1 (expected `495ca096...`) | pass | 0.0863 | 18.26 | 0.0838 0.0829 0.0901 0.0921 0.0850 |

(Spread rule `max(5%, 50 ms)` satisfied, no re-runs.) Batch-to-batch T varies 0.082-0.092 s (12%) on the VPS.

* S: rustfmt (edition 2024; source is already rustfmt-clean) non-blank non-`//` lines of `src/main.rs` 546 + Dockerfile 8 = **554** (local re-implementation of scc's count; the pinned static-tools image was not built, no network use) [M]; matches the board's S. D = 1 (0 dependencies; `Cargo.toml` has none). Reference S_ref = 56 (`reference_axes.json`).
* K from `harness/scoring.py` constants: `K = (T_ref/T)^0.5 * min(M_ref/M,50)^0.2 * min(S_ref/S,1)^0.15 * (D_ref/D)^0.15`, T_ref 32.047276 s, M_ref 227,983,360 B (`calibration-seed1.json`). Board values (T 0.031695, M 20.56 MB, S 554, D 1): **K 36.48, points 78.104**, matching the board's 78.10 (`k-calc.txt`). The VPS harness batches give K about 21.6-22.3 (66.7-67.4 pts): not comparable with the board.
* Marginal values at the board point (points = 50 log10 K) [computed]: 1% of T (0.32 ms) = 0.109 pt, 1 ms = 0.348 pt; 1% of M = 0.044 pt, 1 MB = 0.217 pt; **1 source line = 0.0059 pt**, 10 lines 0.059, 22 lines 0.132. `docs/track-a-research-2026-11-01.md` and PERF.md quote 0.014 pt per line and "50 lines = 0.7 pt"; the scoring formula gives 0.0059 pt per line (50 lines = 0.29 pt), so the S lever is worth 2.4x less than documented. The documented rejection of "drop release/kept/finished" is still right: -22 lines +0.13 pt against +20% M -0.79 pt.

## 4. Dominant measured costs

1. **Parse (`State::scan`)**: 57-62% of wall on the VPS, about 65% of user cycles (rl-lyceum), 361 user instructions per record in total, IPC 2.0 single-thread. 0.9 mispredicts per record sit on the endpoint hash loop exit (46% of all branch-misses), 0.3 on newline-mask refill [M]. Kernel input-mapping faults add about 36 ms CPU (5.8 k faults) [M].
2. **Hours phase (counting sort + 3 selections per group)**: 24-25% of wall; thread-summed on the VPS (6 thr, wall-clock inside threads, includes preemption): count 16, scatter 21, select 51, release 3.5 ms. 24% of all mispredicts. Tail imbalance estimated 6-11% of the phase.
3. **Kernel**: 15-18% of CPU (VPS rusage 65 of 425 ms; rl-lyceum 17.6-18.7% of cycles); 10.7-11.5 k page faults; exec+exit 3.1-3.4 ms wall; text phase is 37% kernel.
4. **Text format**: 7-9% of wall (6.2-7.5 ms), CPU 21.5 user + 12.8 sys.
5. **Fixed/serial**: about 6 ms (7%) of VPS wall.

## 5. Which comparisons are reliable

* Reliable: `perf stat` counters on rl-lyceum (instructions, cycles, branch-misses: +-0.0-0.3%); instruction/cycle deltas of builds; stage-wall medians (n = 12-20, medians repeat within 5% between sessions); rusage CPU splits; correctness (byte-exact, many threads); K/S arithmetic; memory.stat trajectory (peak values reproduce within 0.1 MB).
* Noisy: any VPS wall A/B at 6 threads (spread 40-60%, CI +-12 ms); rl-lyceum 8-thread wall (+-11-20%); cross-session VPS comparisons (73 vs 85 ms for the same binary); thread-summed busy times when threads > vCPUs.
* Invalid or misleading: VPS wall vs board wall (different core count, SMT, kernel); `tools/bench.sh` text-phase and ext4 `/out` vs the board's tmpfs; `-r N` `perf stat` elapsed on rl-lyceum with 8 threads; statements that "AVX2 had no wall effect" or "had an effect" from VPS data (neither is supported); the docs' 0.014 pt per S line.
* Missing: any measurement on the board kernel/CPU; per-stage board time; SMT effect at 12 threads; precise kernel symbol breakdown outside the VPS list above.

## 6. Hypotheses (estimates, not measured gains)

Whole-workload bounds first [E from stage fractions, applied to board 31.7 ms]: parse about 18 ms, hours about 8 ms, text about 2.5 ms, serial/exec/exit about 3 ms.

**Tested and falsified (for this implementation): a branch-free endpoint hash.** Mechanism: remove the 0.9 mispredicts/record at line 248 by masking the first 64 bytes with AVX2 and folding with 5 AES rounds (names over 64 bytes continue in the old loop). Prototype (`prototype-aes-hash.patch.txt`, `proto-rl-lyceum.txt`, output identical on full-seed1, edge-cases, sample-seed7, tiny-seed42 at 1 and 8 threads): rl-lyceum 1 thread, 3 alternating rounds of `perf stat -r 10`: branch-misses 6.03 M to 3.28 M (-46%) as predicted, but instructions +5.6% (1,118 to 1,181 M) and **cycles:u +2.2% to +4.1% (551-554 M to 566-574 M)**, task-clock 171-184 vs 175-180 ms (no gain). At 8 threads cycles +4 to +6%. So removing those mispredicts did not pay; the effective mispredict cost is below what the extra AES chain and 20 instr/record cost, and the nominal 15-20 cycle penalty overstates it [I]. Inference on why H1 still ranks first: this loop-exit mispredict is followed immediately by `names.slot(hash)`, which depends on the hash, so speculation had little independent work to lose and the effective penalty is low; quickselect compare mispredicts discard following partition steps, which are independent work, so the prototype result does not undermine H1's premise [I, not measured]. This does not exclude a cheaper branch-free fold; it rules out this one and weakens "mispredicts are the lever" for the parse loop. Hash collision resistance of such a fold was also not proven.

**H1. Hours-phase selection is branch-bound and over-general for small groups.**
Mechanism: `select_nth_unstable` three times per group; 90.6% of 127 k groups have <= 32 records, 62.5% <= 8, yet 55% of records are in groups > 128. Hours is 24-25% of wall; quickselect comparison/partition code carries about 24% of all mispredicts (cmp.rs:2144, smallsort.rs lines; 1.4 M misses). Prediction [falsifiable]: a size-dispatched path (for n <= 32: one in-register sort or sorting network, then read three ranks; large groups unchanged) cuts the hours `select` thread-sum by >= 30% and hours-attributed branch-misses:u by >= 0.7 M, with no change in output. Estimated gain: select is about 60% of hours CPU (51 of 87 ms at VPS 6 threads); 30% of that is about 1.4 ms of board T (about 4%, 0.4-0.5 pt) minus about 10-20 more lines (-0.06 to -0.12 pt) [E]. Correctness risk: low (exact order statistics; test with groups of every size 1-40, ties, duplicates, 0 and 4294967295). Measurement: temporary copy with only the small-group path; rl-lyceum 1 thread `perf stat -r 10` alternating HEAD vs candidate on cycles:u, instructions:u, branch-misses:u, plus `SD_STAGES` select thread-sum; accept only if cycles:u falls at least 1.5% (hours is 26% of cycles, so a 30% select cut is about 4% of total).

**H2. Kernel fault work in parse and text: 10.7 k faults, 65 ms sys CPU (15% of CPU).**
Mechanism: input mapping faults (5.8 k in parse; fault-around maps 16 pages per fault) and per-page write faults on the mmap'd tmpfs output (2.66 k, 4.8 us sys each on this VM). Candidates: populate-ahead per chunk (`madvise(MADV_POPULATE_READ)` on each thread's next chunk, kernel >= 5.14) and writing the output with `pwrite`/`write` per hour instead of storing through a mapping (removes about 2.5 k write faults, `set_len` stays). Prediction: page-faults fall by >= 2.5 k for the output change alone and text-phase sys CPU falls from 12.8 ms to < 6 ms; for populate, parse faults fall by >= 3 k. Estimated gain 1-4% of board T (0.1-0.4 pt) [E]; bare-metal fault cost is likely lower than the VM's 4.8-6 us, so the VPS overstates it. Risks: `FileExt::write_all_at` replaces the existing `copy_nonoverlapping` into the mapping rather than adding a copy, and removes the output `mmap` call and the `base`/`at` bookkeeping, so it is plausibly net negative on S; it charges page cache the same way; populate needs a runtime kernel check (board kernel unknown) or fails soft with EINVAL; correctness risk low; M unchanged (output file pages are charged either way), pagetables may grow about 0.5 MB with populate. Measurement: instrumented copy with output via `pwrite` only, rusage stage split on the VPS (6 and 12 threads, 15 runs) plus rl-lyceum text-phase wall; judge on sys CPU and fault count, not total wall.

**H3. Memory has two co-binding peaks; one-sided fixes are worth about 0.**
Mechanism: hours-phase peak 18.1 MB and text-end peak 18.6 MB (output 10.1 MB file pages plus about 6 MB residual anon). Prediction: removing 2 MB of residual anon (e.g. packing or avoiding the retained `Vec<Row>` and per-thread buffers) lowers the board M by only about 0.5 MB (0.1 pt) unless the hours-phase peak (records 14.9 MB anon at parse end) also drops; a combined reduction of 2 MB would be worth about 0.4 pt (1 MB = 0.217 pt) [E]. Correctness risk: low; code-size cost likely > 20 lines (0.13 pt). Verdict: lowest value of the three unless a cheap way to cut the 14.9 MB record store is found (the 12.4 MB of raw 4-byte records is the floor). Measurement: one `memory.current` poll at finer grain (below) to confirm no higher transient peak between marks.

Ranking by estimated K per engineering hour [E]: H1 (about 0.3-0.4 pt net, well-defined, small code), H2 (0.1-0.4 pt, kernel-dependent, uncertain), H3 (about 0.1 pt, two coupled changes). Mispredict removal in parse: falsified once; do not repeat without a different fold.

## 7. Smallest missing measurement

Per-stage user/sys/fault split on a kernel and CPU like the board's (or at least bare-metal Linux >= 5.14 with 12 hardware threads). The rusage split above exists only for the VPS guest, where fault cost (4.8-6.2 us) is inflated by virtualization; it decides whether H2 is worth more than H1 (if bare-metal sys share is < 5% of CPU, H2 drops below H1 and the 15% kernel figure should not drive work). A second candidate that would change the ranking is the board's actual hours-phase fraction: if hours is under 20% at 12 threads (smaller than the VPS's 24-25%), H1 shrinks accordingly.

## 8. Caveats

* Everything marked [E] is an estimate scaled from VPS or rl-lyceum fractions to the board, not a board measurement.
* Reference outputs came from the reference module run directly, not the reference docker image.
* The official harness ran with two documented edits and cpuset 0-5 (6 CPUs, 6 workers by default); the board runs 12 CPUs. Passing gates there is evidence of contract compliance, not of board timing.
* `perf record` on the VPS is cpu-clock (no PMU in the guest); kernel/user split from it is approximate.
* Remote leftovers (my own directories only): `my-vps:~/sd-review/` (builds, data for seeds 20261101/987654321, harness copy, docker images `sdrev/{cur,prev,inst,inst3}`), `rl-lyceum:~/sd-review-profiler/` (builds). `my-vps:~/sd-bench/profreview/` was created by `tools/bench.sh`. `/dev/shm` scratch dirs were removed.

## 9. Commands (full scripts and raw output in the evidence directory)

* Baseline: `tools/bench.sh profreview 0-5 15 full-seed1`; interleaved A/B `ab-bench.py full-seed1 0-5 20 sdrev/cur sdrev/prev` with `SD_WORK=/dev/shm/sdw` (`--threads 1` and cpuset 3 for the single-thread variant).
* Stages: `stages.py full-seed1 <cpuset> <runs> <threads>` (images built from the `*.patch.txt` copies); native vs docker: `native-vs-docker.py 20`.
* VPS: `perf stat -r 15 -e task-clock,page-faults,context-switches,cpu-migrations taskset -c 0-5 ./bin-cur --data D --out /dev/shm/o2 --threads N`; `sudo perf record -e cpu-clock -F 10000 -- sh -c 'for i in $(seq 25); do taskset -c 0-5 ./bin-cur ...; done'`.
* rl-lyceum: `perf-stat-rl.sh` (`sudo perf stat -r 10 -e cycles:u,cycles:k,instructions:u,instructions:k` and the other event sets, 1 thread on cpu 2 and 8 threads on 0-7), `perf-record-rl.sh` and `perf-precise-rl.sh` (`perf record -e cycles:u|branch-misses:u[pp] -c N`, `perf report --sort srcline`), `proto-rl.sh`, `stages-rl.sh`.
* Correctness: `correctness.sh` (reference via `refrun.py`); generator `python3 harness/generators/a_ingest.py --seed 20261101 --scale full --out ...`.
* Official harness (two-edit copy, see `harness-patch.diff`): `sudo python3 hharness/run_submission.py --context cur --track a --data data/full-seed20261101 --output summary.txt --expected-sha256 7e632eb5e25d0152...e0e5bc --timeout-s 320 --timer ~/sd-review/timer/sd-timer --cpuset 0-5 --work /dev/shm/hw --runs 5 --results hres/...json`.
* K: `k-calc.txt` (constants from `harness/harness/scoring.py` and `calibration-seed1.json`).

Scope note: the AES-hash prototype and the stage/rusage/memory instrumentation went beyond plain stage timing; they existed only as temporary copies outside the tracked tree (patches saved as `*.patch.txt`), within the authorised deviation.
