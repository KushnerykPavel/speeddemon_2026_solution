# Optimization plateau audit: 10 hypotheses, one experiment

Scope: active Rust Tracks A and D at `61d9b2a699ac37922ec6f668f6620ca745b7aa55`, including the existing uncommitted D blob rollback. B/C remain starter Python references, rather than the plateaued implementations. Existing dirty D files and research were preserved. Only hypothesis 1 below is implemented in this audit. No commit, push, or board run is part of this experiment.

## Evidence and decision model

Read: A/D implementations, manifests and Dockerfiles; competition specification and scoring; A/D PERF histories; baseline validation and parser/reference tests; prior strategy/research; D profiles and rejected queue, transpose, load balancing and BFS-blob experiments. The current A path performs strict byte parsing, endpoint interning, an hour dictionary lookup per record, per-group vectors and full latency sorting. D already has parallel bucketed loading, packed CSR, region renumbering, reverse CSR, bidirectional narrow distances with exact wide fallback and software prefetch.

[Official A specification](https://raw.githubusercontent.com/hackathon-raptors/speed-demon-2026/main/specs/a-ingest.md): nearest-rank percentiles, arbitrary record order, exact output bytes, all uint32 latencies. [Official scoring](https://raw.githubusercontent.com/hackathon-raptors/speed-demon-2026/main/harness/scoring.py):

`K = (Tref/T)^0.50 * min(Mref/M,50)^0.20 * min(Sref/S,1)^0.15 * (Dref/D)^0.15`.

For unchanged dependencies and uncapped memory ratios, the relative change is `(Tbefore/Tafter)^.5 * (Mbefore/Mafter)^.2 * (Sbefore/Safter)^.15`. Source lines are counted after formatting. Speed alone is insufficient. Live board samples are noisy: the same D blob binary ranged from 0.198 to 0.223 seconds. The two blob samples do not establish a gain; the existing rollback is preserved.

D's recent VPS profile puts about 0.10 s in loading, 0.12 s in reordering and 0.26 s in search including ~0.055 s transpose. Query scheduling is already within 1–4% of perfect balance. Dial variants and atomic transpose recycling lost time and/or memory; these are not proposed again under new names. Parallel first-touch below isolates a different mechanism from recycling.

New **baseline-only** A diagnostic: five full-seed-1 executions pinned to CPU 2 on the existing Linux VPS. Median read/parse/group phase 1.1387 s; median remaining sort/format/write phase 0.0932 s. The ratio of these medians gives ~92% in ingestion; this is coarse wall-time instrumentation, not a sampled CPU profile or attribution specifically to hashing. Consecutive hour changes: 1,054 / 3,100,000 full records; 872, 850 and 874 / 310,000 for sample seeds 1, 7 and 42. Tiny seed 1: 152 / 1,000. This supports a last-hour cache without assuming sorted input. Raw evidence: [diagnostic](../tests/performance/2026-10-31-ingest-hour-cache/diagnostic.txt).

## Ranked hypotheses

All forecasts below are **untested estimates**, not measured speedups. They include a central estimate for source/memory cost, so the ranking is auditable. Improvement per engineering hour means absolute delta in that track's K divided by estimated implementation plus validation time; this does not model the judges' score or optimize the best-three-tracks overall formula. Baseline K anchors: A 7.358 (43.339268 points); D 20.100 (65.160080 points). Source anchors A ~183 (175 nonblank/non-comment Rust lines plus eight Dockerfile lines), D 722. These are local approximations pending canonical board counts. Near ties are not significant. Estimates are deliberately modest given existing negative experiments.

| Rank | Hypothesis | Central total T change | M ratio after/before | Added S | Hours | Expected ΔK/hour |
|---|---|---:|---:|---:|---:|---:|
| 1 | A: cache the most recently used hour ID | −8% | 1.00 | 8 | 0.5 | 0.528 |
| 2 | A: native CPU compilation | −3% | 1.00 | 1 | 0.5 | 0.214 |
| 3 | D: parallel first-touch of reverse edge storage | −3% | 1.00 | 12 | 1.5 | 0.172 |
| 4 | A: mmap with parallel chunk-local aggregation | −35% | 1.20 | 90 | 6 | 0.155 |
| 5 | A: lightweight endpoint hash with full equality | −10% | 1.00 | 30 | 2 | 0.112 |
| 6 | D: monotone radix queue keyed only by distance | −8% | 1.03 | 70 | 4 | 0.111 |
| 7 | D: PGO trained across several generated seeds | −5% | 1.00 | 25 | 4 | 0.104 |
| 8 | A: contiguous latency arena | −6% | 0.80 | 45 | 4 | 0.080 |
| 9 | A: SIMD newline/separator scanning | −12% | 1.00 | 50 | 3 | 0.069 |
| 10 | A: exact selection for large groups | −3% | 1.00 | 12 | 1 | 0.042 |

1. **Hour ID cache — hash lookups, cache locality and branch prediction.** Bottleneck: default hash/probe of the same 13-byte hour on almost every record. Why: one exact key comparison plus a strongly biased cache-hit branch can replace those probes. Forecast: 3–12% total time reduction on chunked logs; miss-heavy inputs could regress, and a tiny speedup may fail to pay for eight lines. Risks: stale IDs after new endpoints/hours, invalid-record state mutation, accidentally treating locality as an ordering guarantee. Controlled experiment: add only a one-entry `(hour, ID)` cache with unchanged dictionary fallback; compare identical releases on full seed 1 and sample seeds 7/42, plus alternating, shuffled and constant-hour inputs; compare every output with Python and published hashes. **Chosen.**

2. **Native CPU compilation — compiler/instruction selection.** Bottleneck: A's Dockerfile does not use D's existing `target-cpu=native`; generic code may miss useful instructions or tuning. Why: same algorithm/source can compile to better instruction sequences for the judging CPU. Forecast: 0–6% total reduction, no reliable gain promised. Risks: illegal instructions if binaries move to a different CPU; native tuning on the Haswell VPS is not representative of the board's newer CPU. Controlled experiment: change only A's build flag, run both binaries on the same build/run host with unchanged datasets, disassemble relevant hot loops, repeat on board-class hardware before acceptance.

3. **Parallel first-touch — page faults and parallelism.** Bottleneck: serial allocation/initialization before D's reverse scatter. Existing diagnostic measured ~20 ms to touch 28 MB on one thread versus ~6 ms on six. Why: initialize disjoint reverse-edge slices in parallel and distribute physical-page faults before scatter. Forecast: 0–5% total reduction; extra thread startup or redundant zeroing could erase it. Risks: uninitialized reads, out-of-bounds writes, increased peak from overlapping buffers. Controlled experiment: change only reverse edge allocation/initialization, preserve degree counting/scatter, time first-touch and full run under cgroup memory measurement; test empty and skewed graphs. Do not combine with the already-rejected atomic degree/recycle experiment.

4. **Mapped parallel ingestion — parsing and parallelism.** Bottleneck: A's dominant serial ingestion phase. Why: read-only mapping avoids copying the whole file; newline-aligned workers parse disjoint chunks into private groups and merge exact latency multisets. Forecast: 20–55% total reduction, potentially +20–100% peak from local groups/merge. Risks: split records, duplicate/lost records, independent dictionary IDs, CRLF/EOF, memory and source penalties. Controlled experiment: keep parser and quantiles unchanged, introduce only mapped chunk processing/merge, sweep 1/2/6/12 workers at fixed memory limits across several seeds and skewed endpoint distributions. Budget includes ownership-safe mapping and merge validation.

5. **Endpoint hash — hashing and data structures.** Bottleneck: A hashes endpoint bytes with the default secure hasher for every record. Why: a cheaper word hash with full key equality can reduce CPU cost in the small dictionary. Forecast: 3–15% total reduction; degeneracy can make it worse. Risks: treating hashes as identity, unsafe tail loads, collision-heavy inputs. Controlled experiment: change only the endpoint map's hasher; retain full byte equality and parser; test adversarial collisions, long/prefix endpoints, seeded catalogues and synthetic high-cardinality inputs; measure probes as well as total time. No hard-coded endpoint catalogue or answer recognition.

6. **Radix queue — algorithmic priority queue cost.** Bottleneck: D's binary heap comparisons on long searches. Why: nonnegative integer distances make each side's popped distance monotone, allowing radix buckets rather than logarithmic sifts. Unlike the rejected Dial ring, it does not scan a fixed window or retain a far-list until query end. Forecast: −15% to +10% total time (central −8%); redistribution can lose on short queries. Risks: zero-weight edges break monotonicity of a combined `(distance,node)` key; ties, stale entries, u32 sentinel and u64 overflow fallback. Controlled experiment: replace only the u32 side heap with distance-keyed radix storage, retain the wide baseline path, check all-pairs randomized graphs and exact fallback boundaries; measure pushes/redistributions and total T/M. Do not change direction scheduling simultaneously.

7. **PGO — compiler layout and branch probabilities.** Bottleneck: generic inlining/layout of D's parsing, row decoding, sparse distance and bidirectional search paths. Why: measured branch frequencies can guide code layout and inlining without a new runtime algorithm. Forecast: 0–8% total reduction; training bias and instrumentation overhead may make the result unhelpful. Risks: compiler/profile version mismatch, missing fallback coverage, seed-specific branch assumptions, build-time/training-source score cost. Controlled experiment: train with synthetic/generated seeds 7/42/101 including heavy weights and wide answers; evaluate seed 1 and unseen generated seeds using identical non-PGO flags, record build time and all added scored files. Profiles contain behavior counts, not precomputed answers. The estimated 25-line overhead must be replaced by the actual training implementation's S before keeping.

8. **Latency arena — memory layout and allocations.** Bottleneck: A's per-group vector allocations and capacity slack. Why: count/prefix/scatter into one contiguous arena can reduce allocation overhead and improve sorting locality. Forecast: 0–10% less total time, 10–25% less memory; an extra parse/scatter pass may lose. Risks: sparse IDs, prefix overflow, duplicate losses, overlapping ranges and peak during transition. Controlled experiment: keep hashing/parser/output unchanged, replace only latency storage with count/prefix/scatter ranges; validate per-group multisets on skewed/high-cardinality inputs; measure phase peak, allocations, total T and S.

9. **SIMD delimiter scan — CPU instructions and parsing.** Bottleneck: byte-at-a-time newline and field separator searches in A. Why: vector comparisons can identify many separators per instruction while retaining scalar numeric checks. Forecast: 0–20% less total time, depending on how much hashing dominates. Risks: unsafe buffer overreads, vector tails, long lines, unsupported CPUs, accepting malformed records. Controlled experiment: change only delimiter discovery with a guarded AVX2 path and scalar fallback, retain `parse_decimal` and field-count checks; test every buffer split plus malformed/long rows, then measure instructions/byte and total T. Neither a new numeric parser nor a new hasher belongs in this experiment.

10. **Exact multi-selection — algorithms and complexity.** Bottleneck: A sorts all latencies even though only three order statistics are required. Why: standard-library in-place selection over progressively smaller disjoint slices can avoid ordering unused elements. Forecast: 0–5% total reduction: all finalization currently accounts for only ~8%, so this cannot be a large standalone win. Risks: repeated ranks for tiny groups, subslice offsets, subsequent partitions overwriting chosen values, duplicate/max-u32 values. Controlled experiment: change only quantile computation for large groups, leave small groups sorted, choose a general threshold on synthetic group-size distributions before evaluating seeds; compare with full sort for all lengths near rank boundaries. Reject if the saving fails the S break-even condition.

## Experiment result

**Kept hypothesis 1 only.** The one-entry cache adds eight nonblank/non-comment Rust lines (approximate scored total 183 → 191 with unchanged Dockerfile). All nine optimized Rust tests pass. All twelve official/synthetic input cases match Python byte-for-byte, and every timed run checks the reference bytes or SHA-256. Sample/full seed-1 published hashes match. Formatting and Clippy pass. Three independent architecture reviews approve with no findings.

| Input / batch | Before (s) | Cached (s) | Change |
|---|---:|---:|---:|
| Full seed 1, 10 measurements/variant | 1.18864 | 1.05518 | −11.23% |
| Full seed 1, repeat, 6/variant | 1.23853 | 1.12672 | −9.03% |
| Sample seed 7, 6/variant | 0.15665 | 0.13846 | −11.62% |
| Sample seed 42, 6/variant | 0.16441 | 0.14340 | −12.78% |
| Alternating hours, synthetic seed 3, 6/variant | 0.09478 | 0.10395 | +9.68% |
| Alternating hours repeat, 6 batches of 5/variant | 0.09464 | 0.09843 | +4.00% |
| Shuffled hours, synthetic seed 19, 6/variant | 0.09498 | 0.09233 | −2.79% |
| Constant hour, synthetic seed 101, 6/variant | 0.08690 | 0.07253 | −16.53% |

Warmup precedes each comparison; AB/BA order alternates. Table reports mean after dropping one minimum and one maximum per variant; all runs include process startup and output writing. VPS CPU 2, native Linux, pinned offline Rust image and identical build options. Full-run spreads 24.2%/11.4% initially and 8.7%/19.4% in repeat; estimates are noisy and are not an official scoring result. All ten initial full-input pairs favor the candidate. The repeat was justified by spread and the miss-heavy regression, not by a search for a better score.

Separate `/usr/bin/time` measurements: baseline RSS 26.28–26.50 MB, cache 26.35–26.50 MB, three runs each. No material RSS difference established; this is **not cgroup memory.peak**. Initial driver `wait4` RSS values all inherited the Python driver's ~284 MB high-water mark and are invalid as solution-memory evidence; retained raw values are explicitly excluded. With M held constant and approximate S 183 → 191, observed full-run times imply K multipliers 1.0546 and 1.0417 (~+1.15/+0.89 speed points), conditional on transfer to the scoring machine. No board K gain is claimed.

Keep rationale: two full batches and two additional official seeds support the same ingestion improvement, and the generic fallback preserves arbitrary ordering. Synthetic miss-heavy inputs expose a real overhead risk; shuffled inputs do not establish a benefit within noise. There is no claim of universally faster execution. No catalogue, time window, public answer or seed is hard-coded. The cache exploits runtime equality, not input recognition. The other nine hypotheses remain unimplemented.

Full seed-1 SHA-256: `495ca09682b9673ef836a506496177680bb4a3af54255174befddb1f83481cd3`.

[Complete experiment evidence](../tests/performance/2026-10-31-ingest-hour-cache/README.md), including exact source/binary hashes, raw samples, protocol, rejected memory measurements, repeat and review results. The original streaming SSH connection stopped delivering output after round 9; the remote run completed and `summary.json` was recovered through a fresh bounded connection. No incomplete batch was used as a completed measurement.

## Next evidence after this experiment

Ideas remain. Before expensive D changes, capture sampled stacks plus cycles/instructions, LLC misses, branch misses, dTLB misses and minor faults on the scoring CPU or a close six-P-core/SMT machine. Attribute transpose allocation, scatter, sparse distance lookup/reset, heap operations and page faults separately. Existing phase timers do not tell us which instructions stall, and the Haswell VM does not reproduce the board's cache hierarchy. Collect cgroup phase peaks and repeated full runs rather than relying on best board samples.

For A, split the ingestion profile into delimiter/number parsing, endpoint hashing, hour hashing and vector growth; separately time sorting versus formatting/output. Use generated seeds and synthetic skew/cardinality/order variations. For radix/PGO work, consult the primary algorithm/compiler documentation when the experiment is selected; do not infer performance from competitors' best times. No evidence here supports claiming either implementation optimal.

## Follow-up experiment: hypothesis 2, native CPU build for A — rejected

Same source (hour cache) built twice in the pinned Rust image on the Haswell VPS, differing only by `RUSTFLAGS="-C target-cpu=native"`. CPU 2, warm input, alternating order, trimmed mean, output SHA-256 identical across variants (full seed 1 `495ca096…81cd3`; samples 7/42 also constant).

| Input | Generic s | Native s | Native/generic | Native faster in |
|---|---:|---:|---:|---:|
| full seed 1 (12 runs) | 1.1530 | 1.1244 | 0.975 | 7/12 |
| sample seed 7 (14) | 0.1371 | 0.1372 | 1.000 | 9/14 generic faster |
| sample seed 42 (14) | 0.1329 | 0.1369 | 1.030 | 8/14 generic faster |

Spreads 21–48%, larger than every effect. Mixed sign, no signal; +1 Dockerfile line and SIGILL risk if build/run hosts differ. **Reverted; nothing kept.** Hot loop is byte-at-a-time scalar, so extra ISA gives the compiler little to use. Not run on board hardware (Haswell VPS is not representative).

Next untested by rank: #3 D parallel first-touch (est. ≤3%, likely noise-level), #4 A mapped parallel ingestion (largest estimated win, ~−35% T, ~6 h). Recommendation: #4 is the only remaining idea with an effect size above measurement noise; skip micro-ideas until the perf-counter profile on scoring-class CPU exists.
