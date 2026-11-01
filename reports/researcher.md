# Track A researcher report (HEAD 6d5802b, board 78.10 pts: T 0.031695 s, M 20.56 MB, S 554)

Read-only. Nothing was run. Sources read: track-a/src/main.rs, track-a/PERF.md, docs/track-a-*.md, and the official spec and harness text (specs/a-ingest.md, harness/README.md) from the local scratchpad copy of hackathon-raptors/speed-demon-2026.

## Sensitivities (arithmetic from the scoring formula, not measured)

points = 50 log10 K, so the sensitivities are:
- 1% of T = 0.109 pt. 1 ms of T = 0.34 pt.
- 1 MB of M = 0.21 pt.
- 1 source line = 0.0059 pt, so 50 lines = 0.31 pt.

docs/track-a-research-2026-11-01.md and the PERF.md plateau notes use 0.014 pt per line, which is about 2.4x too high. S is therefore a weaker lever than those notes assume, and time and memory dominate.

Reaching the top-10 cutoff (+6.6 pt) needs about -37% T alone. The five hypotheses below sum to about +2 to +3.5 pt (an inference, not additive). They are not enough for top 10 on their own, but H1 and H3 are the best-value items.

## Spec facts that matter (primary: specs/a-ingest.md)

- "Every line matches the grammar ... a solution may assume well-formed input."
- LF only, no CR, final LF present.
- ts is always exactly 27 bytes. Status is exactly 3 digits. Latency has no leading zeros, 1 to 10 digits.
- Input is 239 MB, 3.1M lines, average 77 bytes.
- S is measured as scc code lines after rustfmt, with comments and blanks excluded (harness/README.md).
- Per-hour record count is about 43k. The endpoint distribution is Zipf with exponent 1 over 1,842 endpoints.

## H1. Select the median first, then recurse into the upper side (about zero source lines)

**Fact.** In main.rs, `values.iter_mut().zip(at.iter_mut()).rev()` selects p99 on the whole group first. It then selects p95 on `group[..p99idx]`, and p50 on `group[..p95idx]`. The ranges shrink only by 1% and 5%, so each of the three selects touches almost the whole group.

**Inference.** Using Knuth's expected quickselect compare counts, the current order costs about 2.1n + 2.3n + 3.2n, or about 7.6n compares. The order p50 on n, then p95 on `group[idx50+1..]` (size n/2), then p99 on `group[idx95+1..]` (size 0.05n) costs about 3.4n + 1.2n + 0.1n, or about 4.7n. That is about 38% fewer compares.

**Measured cost addressed.** The hour phase is about 117 instr/record and 27% of T at 8 threads. The count and scatter passes are about 20 to 25 instr/record. Per-group overhead (rows, ilog10) is probably about 15. By subtraction, selection is probably 60 to 80 instr/record. This split is **missing a measurement** and needs callgrind or perf annotate attribution of the hour closure.

**Difference from earlier work.** The ordering was never recorded as tested. The "iterator chains vs loops" and similar experiments only touched the surrounding code.

**Estimate.**
- Instructions -4 to -7%, T -3 to -6% (about +0.4 to +0.7 pt), M unchanged, S about 0 lines.
- Groups with n < ~20 gain nothing, and about 43% of records sit in groups with n < 100 (inference from Zipf).
- A follow-on is a single insertion sort for n <= 24, then reading the three ranks.
- A further follow-on is Floyd-Rivest selection for n >= 500. It needs about 1.5n compares for the median (Floyd and Rivest, CACM Algorithm 489, 1975; not fetched).

**Correctness obligations.**
- `select_nth_unstable(k)` leaves everything before k <= nth and everything after >= nth. This was recalled from the std docs and not verified on the page; the WebFetch result was truncated.
- Ranks must handle equal indices, for example n = 1 or n = 2, where idx50 == idx95.
- When idx95 == idx50, skip the select and reuse the value. When idx95 > idx50, select relative index idx95-idx50-1 on the upper slice.
- Output must stay byte-identical on the full seed 1 hash (`495ca096...81cd3`), on tiny, sample and edge-case inputs, and on groups with duplicate latencies.

**Experiment.** Change only the select order. Run `perf stat` for instructions and task-clock on rl-lyceum with 1 and 8 threads, 10 runs, alternating A/B.

**Rejection rule.** Reject if instructions fall less than 2% or the 8-thread median does not improve. Time effects inside noise are inconclusive.

**Evidence.** The std slice docs (`select_nth_unstable`); Hoare 1961, "Algorithm 65: FIND". Effort: 1 hour.

## H2. Use the spec's well-formed-input guarantee: delete validation, CR and error plumbing

**Fact.** `scan` still does all of the following:
- status digit validation (the `bad` mask),
- digit validation in `number`,
- the line length < 37 check,
- the `\r` strip,
- the hour range and month checks,
- the `Result` and `BAD` plumbing.

No previous experiment tried removing these. PERF.md step 7 describes LF, CRLF and unterminated-line support as a design goal. The spec forbids CR and guarantees the final LF. Hardcoded values are not involved: the hour is still derived from the digits.

**Missing measurement.** Instructions spent on validation. Callgrind line attribution would give this. My estimate is 30 to 45 of the 265 parse instr/record.

**Estimate.**
- T -4 to -7% (+0.45 to +0.8 pt).
- S -30 to -40 lines (+0.2 pt): drop `BAD`, the `CR` branch, `?` plumbing, the `ok_or` chains, and possibly the unterminated-last-line branch.
- M unchanged.
- Keep the `SLACK` tail copy (or a padded mapping), because the reads past the end of a chunk are a memory-safety issue.

**Obligations.**
- All inputs the generator can produce must give identical output. Test the E1 to E15 edge cases at several seeds, using the official generator at tiny, sample and full.
- Malformed-input behaviour is not required. Remove the Rust tests that expect rejection or CRLF, and say so in PERF.md.
- Keep SLACK reads inside mapped memory.

**Experiment.**
1. Strip validation only, with no other change.
2. Run `perf stat` instructions on the lyceum box.
3. Run the VPS bench for T and M.

**Rejection rule.** Reject if instructions drop less than 5%.

**Evidence.** The spec text above; track-a/src/main.rs `scan`. Effort: 2 hours.

## H3. Fuse the hour phase and the text phase (remove the `rows` and `size` second pass)

**Fact.**
- Each hour's `Vec<Row>` (24 B per group, up to 132k groups) is held until all hours finish.
- The `size` pre-pass runs 6x `ilog10` plus a u64 division per group, and the text phase repeats the division.
- The code ends with a barrier, `hours.sort`, and a third thread spawn.

**Inference.** The retained rows are about 72 hours x 1,800 x 24 B rounded up to Vec capacity doubling, so about 3 to 4.5 MB. They are alive while the 10.1 MB output is allocated, which would make them a visible part of the 20.5 MB peak.

**Missing measurement.** The cgroup `memory.stat` and `memory.peak` timeline decomposition (anon vs file, which phase is the peak).

**Mechanism.**
- After an hour's selection, the thread formats that hour's text directly into a thread-local buffer. The text length is the size, so the `size` pre-pass disappears.
- It then spins until the sizes of the earlier hours are published in an `[AtomicUsize; 72]`, and memcpy's into the mapped output at the prefix-sum offset.
- The output file is `ftruncate`d to an upper bound before mapping (sparse on tmpfs, untouched pages are not charged) and to the exact total at the end.
- There is no deadlock. Hours are claimed in increasing order by running threads, so every predecessor is held by a thread that is making progress.

**Difference from earlier work.**
- Step 10 freed records before output.
- Step 13 formatted straight into a mapped file, but kept the barrier and the rows.
- Cache Me kept one barrier (docs/track-a-open-solutions).

**Estimate.**
- M -2 to -4.5 MB (+0.4 to +0.95 pt).
- T -1 to -3% (+0.1 to +0.3 pt). This removes one barrier and spawn, the size pass, a duplicate u64 division, and rows traffic.
- S about -10 lines.

**Obligations.**
- The exact total must equal the sum of the hour sizes.
- No bytes may be written to the unused tail. An empty output must give a 0-byte file. The final `ftruncate` must happen before `exit`.
- A single worker must give identical output.
- The offset wait must make progress for any thread count from 1 to 12.
- The output map must not be used after truncate.
- Check the sizing assertion against hour 0 and the last hour.
- Output must stay byte-identical on all datasets.

**Experiment.** Measure `memory.peak` on the VPS (5 runs each, alternating A/B, restricted container as in `tools/bench.sh`) and time.

**Rejection rule.** Reject if M does not fall by at least 1 MB or time rises by 2% or more.

**Evidence.** main.rs `summarize`; PERF.md steps 10 to 13. Effort: 3 to 4 hours.

## H4. Branch-free latency decode with SSE (pshufb + pmaddubsw + pmaddwd)

**Fact.** `number()` is about 40 instr/record by callgrind (PERF.md steps 10 to 11). The lengths 1 to 7 take one path and 8 to 10 take a scalar loop. Cache Me uses maddubs/madd.

**Mechanism.**
- Load 16 bytes ending at `line_end` (`line_end-16`).
- `pshufb` with a table indexed by the digit count right-aligns the digits.
- Subtract '0', then `pmaddubsw` with (10,1), `pmaddwd` with (100,1), `packusdw`, and `pmaddwd` with (10000,1), then combine the 10-digit case (about 14 instr).
- Combined with H2, validation is already gone.
- The digit count still has to come from the existing back-scan.

**Difference from earlier work.** Earlier SWAR work (`number` takes the digit count) was measured only in S lines. The AVX2 experiment touched only the newline scan.

**Estimate.** 20 to 25 fewer instr/record (about -5% parse, T -2 to -3%, about +0.3 pt). It needs 12 or more lines for the table and the `cfg` fallback (-0.07 pt).

**Obligations.**
- Handle length 1 to 10 and values up to 4294967295, which overflow 32-bit lane sums only if done in u32.
- Keep the fallback for non-x86 so macOS tests still build.
- Check 0, 4294967295, and every length.

**Experiment.** Apply after H2. Measure instructions.

**Rejection rule.** Reject if the saving is below 10 instr/record.

**Evidence.** Cache Me's public repo (see docs/track-a-open-solutions); Intel intrinsics guide (pmaddubsw). Effort: 3 hours.

## H5. Pointer-bump record append, with a fixed-size endpoint table (no bounds checks)

**Fact.** `State::push` does all of the following per record:
- `count[hour] += 1`,
- the `latency >= PACKED` test,
- `open[hour]`, a bounds check, a `blocks[b].n == CAP` check,
- a `blocks[b]` index with a second bounds check,
- a store, and a read-modify-write of `block.n` (a store-to-load dependency).

`Names::slot` indexes Vec-backed atomics, so the bounds check stays, and it runs a `for _ in 0..SLOTS` loop.

**Mechanism.**
- Keep per-hour (cursor, end) raw pointers into the open block, and bump with `*cur = v; cur += 1; if cur == end { new_block() }` (about 6 instr).
- Make `n` implicit, deriving counts from the pointers when blocks are closed.
- Make the table `[AtomicU64; SLOTS]` so the bounds check goes away.

**Estimate.**
- 12 to 20 fewer instr/record (-3 to -5% instructions), T -1.5 to -3% (+0.2 to +0.35 pt).
- S about 0 to -5 lines.
- The estimate is a guess without asm. It needs `cargo asm` or callgrind line attribution for `push` and `slot`; the profiler may have it.

**Obligations.**
- The block header must still carry the hour and a filled count for the `release()` and hour-phase iterators.
- Blocks must be finalised at the end of each chunk loop and at the end of the thread.
- Keep `wide` handling.
- Output must stay byte-identical on all datasets.

**Experiment.** Measure `perf stat` instructions on lyceum before and after.

**Rejection rule.** Reject if the saving is below 8 instr/record.

**Evidence.** main.rs `push` and `slot`. Effort: 2 to 3 hours.

## Considered and not recommended

- **Histogram or radix select per group.** Zipf mass says only the top ~5 endpoints per hour have n >= 1000 (about 28% of records), and a ~900-bin histogram only pays beyond that. H1 gets most of the gain more cheaply. Revisit only for the head groups if H1 leaves selection dominant.
- **pread streaming into 128 to 256 KB per-thread buffers instead of mmap.** The kernel is about 18% of parse cycles, but a 239 MB copy at roughly 10 GB/s costs about the same CPU as mapping and zapping 58k pages. The buffers add M (about -0.3 pt for 1.5 MB). Expected value is about 0, and MAP_POPULATE was already measured slower by Cache Me. This is an inference, not a measurement.
- **AES-NI or CRC32 hash.** It saves about 8 to 10 instr/record and needs a `cfg` fallback, for about +0.2 pt.
- **Thread count and SMT.** There is no board-hardware data beyond the 12-worker note. This needs a sweep, not a hypothesis.
- **Reusing threads across phases.** About 0.5% of T.

## Note on a missing measurement for everything above

Per-phase instruction attribution of the hour closure (count, scatter, select, rows, size) would turn the H1 and H3 estimates into measurements. The same goes for validation instructions in `scan` (H2) and `push` and `slot` (H5).
