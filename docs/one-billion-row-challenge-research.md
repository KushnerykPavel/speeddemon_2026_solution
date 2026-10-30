# One Billion Row Challenge: solutions and lessons for Speed Demon

Researched 2026-10-30. Primary-source review, not a benchmark of this repository. Proposed gains below are hypotheses. This complements [the existing strategy](optimization-strategy-2026-10-30.md) and [Track A plan](track-a-ingest-optimization-plan.md).

## Findings

Fast 1BRC implementations fuse byte parsing with aggregation, allocate keys only on first occurrence, partition work among independent workers, and merge small summaries. The most aggressive implementations additionally optimize instruction dependencies, delimiter scans, hash probes, and process startup/cleanup. **The transferable design is the pipeline; the temperature-specific arithmetic and tiny summaries do not solve Track A's exact percentiles.**

## What the original benchmark measures

1BRC aggregates station temperatures into minimum, mean, maximum, sorted by station. Valid station names are arbitrary UTF-8, 1–100 bytes; temperatures have exactly one fractional digit in −99.9…99.9; there may be 10,000 distinct stations. The usual generated dataset has 413 stations. Java was required for official submissions; non-Java results are unofficial. [Official rules and dataset description](https://github.com/gunnarmorling/1brc/blob/main/README.md).

The official winner is Thomas Wuerthinger, Quan Anh Mai, and Alfonso² Peterssen at **1.535 s**. Evaluation used eight cores of an AMD EPYC 7502P, 128 GB RAM, Fedora 39, RAM-disk input, and launch-script wall time. Five runs were trimmed by dropping fastest/slowest and averaging the other three. Bonus all-core results use a different configuration; their winner and timings differ. These figures do not establish disk throughput or expected Speed Demon runtime. [Official results and evaluation protocol](https://github.com/gunnarmorling/1brc/blob/main/README.md#evaluating-results).

The evaluation script runs a complete correctness pass before timing, so these results have warm-input context even though Hyperfine's explicit warmup count is zero. Linux evaluation pins execution to CPU IDs 0–7; the documented environment disables SMT and turbo. [Evaluation script](https://github.com/gunnarmorling/1brc/blob/main/evaluate.sh), [environment](https://github.com/gunnarmorling/1brc/blob/main/ENVIRONMENT.md).

## Solution comparison

| Solution / primary source | Main design | Why read it |
|---|---|---|
| [Official winning Java source](https://github.com/gunnarmorling/1brc/blob/main/src/main/java/dev/morling/onebrc/CalculateAverage_thomaswue.java) | Memory mapping; dynamically assigned segments; worker-local hash tables; SWAR; three interleaved cursors | Most useful source for a fully combined fast path |
| [Ben Hoyt's Go progression](https://benhoyt.com/writings/go-1brc/) and [final source](https://github.com/benhoyt/go-1brc/blob/master/r9.go) | Buffered chunks; fused delimiter search/hash; custom table; integer temperatures; independent partitions | Shows how far portable code goes before unsafe access or mapping |
| [Ragnar Groot Koerkamp's Rust lab notebook](https://curiouscoding.nl/posts/1brc/) | Profiling; byte parsing; SIMD; perfect hashing; independent parsing streams | Detailed experiments, regressions, and explicitly disclosed dataset assumptions |
| [Le Huy Duc's C++ SIMD implementation](https://github.com/lehuyduc/1brc-simd) | SIMD separators, hashing and equality; mmap; worker-local aggregation; two independent lines per thread | Concrete SIMD/ILP design with long-name fallback |

### 1. Winning Java: where the speed comes from

The [winning source](https://github.com/gunnarmorling/1brc/blob/main/src/main/java/dev/morling/onebrc/CalculateAverage_thomaswue.java) implements these connected optimizations:

1. **Map once, assign segments dynamically.** An atomic cursor distributes 2 MiB segments. Workers adjust boundaries to LF so rows are neither duplicated nor split. Scheduling happens per segment, not per row.
2. **Keep state private.** Each worker has its own table of 131,072 slots and records `min`, `max`, `sum`, `count`. Shared updates are absent from the row loop; summaries merge after joining.
3. **Scan words rather than characters.** The delimiter finder XORs a 64-bit word with repeated `';'`, then uses a zero-byte detection expression. Trailing-zero count identifies the first matching byte. This is SWAR: ordinary integer instructions operate on several packed bytes, without requiring vector intrinsics.
4. **Optimize common short names with a correct fallback.** Cached first/second name words permit cheap short-key matches. Longer names follow a separate loop. Table collisions trigger actual name comparison and probing; a hash is not treated as identity.
5. **Parse the promised numeric grammar.** Mask/shift/multiply operations turn a short temperature token into integer tenths. A 64-bit sum avoids overflow from aggregating a billion observations. This code is specialized to that grammar and representation.
6. **Expose independent work inside each core.** Each segment becomes three independent scanners. Loads, delimiter masks, table lookups, and numeric conversion are interleaved across scanners. This breaks the serial dependency in “find next delimiter → find next record → repeat.” Merely unrolling successive dependent records does not achieve the same effect.
7. **Decode and order at the end.** Station strings are created from retained input addresses when merging into a `TreeMap`, after per-row work is complete.

The [build script](https://github.com/gunnarmorling/1brc/blob/main/prepare_thomaswue.sh) uses GraalVM native image, `-O3`, native-architecture compilation, Epsilon GC, and disabled loop safepoints. This is a package of algorithm, code generation, and runtime choices; the result is not evidence that one arithmetic trick alone provides the speedup.

The program also launches a worker subprocess and forwards its stdout. Its own comment says this lets mapping cleanup continue while the parent returns. This is a benchmark-relevant optimization to inspect, not a default recommendation for Speed Demon: account for all child work and the actual harness's completion definition. [Worker lifecycle and rationale](https://github.com/gunnarmorling/1brc/blob/main/src/main/java/dev/morling/onebrc/CalculateAverage_thomaswue.java).

### 2. Portable buffered processing: an important control

Hoyt reports **105 s → 3.44 s** across nine Go versions: custom numeric parsing, integer representation, fewer scans/allocations, a custom table, then parallel partitions. Parallelizing the original took 22.6 s; optimized sequential processing took 22.1 s; combining both reached 3.44 s. His measurements used Go 1.21.5 on a Linux/amd64 laptop with 32 GB RAM, best of five and cached input; they are not the official leaderboard protocol. He deliberately excluded mmap, assembly, and unsafe operations. [Author's experiment](https://benhoyt.com/writings/go-1brc/).

The [final implementation](https://github.com/benhoyt/go-1brc/blob/master/r9.go) uses 1 MiB reusable buffers, retains the incomplete final record between reads, and computes the station hash while finding `';'`. It copies a station name only when inserting a new table item. Each partition opens/seeks/limits its own reader and returns its own results. This is a useful comparison design for a streaming Track A parser before adopting mapping.

Its `2^17` bucket count matters: a bitmask `hash & (capacity-1)` requires a power-of-two capacity. Equality is checked while linear probing. Avoid transcribing incidental source assumptions: this implementation relies on record termination and valid input, which must be checked against the destination contract. [Hash table and buffer loop](https://github.com/benhoyt/go-1brc/blob/master/r9.go).

### 3. Rust and C++: useful experiments, different contracts

Koerkamp's Rust post documents a progression from ordinary parsing toward SIMD and instruction-level parallelism. It explicitly assumes the city set can be discovered from a 100,000-row prefix; some stages assume particular name lengths or uniqueness of shortened keys. Those assumptions are not general solutions to the official input contract. His four-stream implementation uses independent portions of the file to expose work to the CPU. The reported 0.90 s six-thread figure uses a later 4.6 GHz configuration, internal timing, and excludes unmapping; it must not be compared directly with official end-to-end results. The post also records bugs and fixes for undefined behavior. [Author's Rust experiment](https://curiouscoding.nl/posts/1brc/).

Le Huy Duc's README identifies SIMD delimiter masks, hashing, and string comparison, plus two independent lines per thread. It specializes the common ≤16-byte station case but retains a long-name fallback up to 100 bytes. It also calls out integer parsing, mapping, and thread-count sensitivity. This is a useful source for designing fast paths that preserve the full grammar; no cross-machine speed ranking is inferred here. [Author's C++ design notes](https://github.com/lehuyduc/1brc-simd/blob/main/README.md).

## Transfer to Track A

The following is engineering analysis for this repository, using the constraints recorded in [the Track A plan](track-a-ingest-optimization-plan.md). It is not a claim that these changes have been implemented or measured.

| 1BRC lesson | Track A adaptation | Boundary / cost |
|---|---|---|
| Fuse parsing and updates | Parse only required fields into hour, endpoint, latency, error flag; immediately update a group | Preserve the complete A grammar and error predicate |
| Allocate once per unique key | Discover endpoint IDs at runtime; keep compact hour keys and group state | Full endpoint equality is required, including shared prefixes |
| Worker-private state | Parse LF-aligned chunks into local endpoint/group dictionaries | Remap local IDs during merge; count duplicate rows exactly |
| Small final reduction | Merge counts/errors and concatenate latency data | Percentiles cannot be merged from only local percentile values |
| Efficient table probes | Profile standard-map hashing/equality; consider fusing a byte hash with endpoint scanning | Custom collision handling and resizing add source size and correctness burden |
| Independent streams | Try two/four cursors only after identifying serial parsing dependencies | More live state/register pressure; likely poor first experiment |
| Specialized arithmetic | Keep latency and error rates integer | Temperature tenths parser does not apply to arbitrary `u32` latencies |
| Mapped input | Compare mapping against reusable buffered byte chunks | Resident file pages and aggregation compete within 8 GB; mapping is not zero memory |

### The critical difference: exact percentiles

1BRC's aggregate is constant-size per key: summaries combine using minima, maxima, addition, so streaming aggregation needs O(K) state for K keys, excluding input mapping. Track A needs exact nearest-rank p50/p95/p99, so local `(p50,p95,p99,count)` summaries are insufficient. Merged ranks depend on the underlying distributions, not averages of local percentiles. The current retain-all-values approach uses O(N) latency storage for N records.

A direct correct parallel adaptation retains all latencies, merges them by final group, then runs exact selection or sorting. An exact histogram would also work in principle, but full-range `u32` latency prevents assuming a small dense histogram. Merge can cause a memory peak if local vectors, destination vectors, capacity slack, mapped pages, and dictionaries overlap. A design that speeds parsing while doubling stored values may lose overall.

Do not assume a mapped input is excluded from the memory score. Linux cgroup accounting exposes filesystem cache, mapped file data, and page tables; the actual harness determines what is charged. Verify its cgroup boundaries before interpreting RSS or claiming zero input-memory cost. [Linux cgroup v2 memory accounting](https://docs.kernel.org/admin-guide/cgroup-v2.html#memory).

The inspected [current Track A source](../track-a/src/main.rs) already uses reusable line storage, borrowed endpoint lookup, inline hour keys, and `sort_unstable`. Treat these as the starting point, not proposed future gains; older plans describe an earlier baseline.

## Recommended experiments

1. **Measure phases and whole process.** Separate read/parse/group, merge, percentile computation, and output diagnostically. Record end-to-end time, output hash, peak memory, source size, and dependencies. A fast parser cannot remove an independent percentile bottleneck.
2. **Compare two byte-reader designs.** Reusable buffered chunks versus read-only mapping, keeping grouping unchanged. Include partial records, final tails, empty input, and smallest chunks. This isolates the I/O/parser question.
3. **Make groups cheap before custom SIMD.** Compare runtime endpoint interning plus compact IDs with existing borrowed lookup. Do not pre-discover keys from a fixed prefix or hard-code a seed's endpoint catalog.
4. **Add parallelism with an explicit memory budget.** Compare 1/2/4/6/12 workers on the target environment. Start with static newline-aligned ranges; add dynamic scheduling only if measured imbalance merits it. Include dictionary remapping, vector merge, and final percentiles in the timer.
5. **Only then optimize scans/probes.** Compare scalar, SWAR, or platform SIMD paths on the actual CPU and field lengths. Keep bounded tail loads and a portable fallback; preserve full-key checks. Try multiple independent scanners only if profiling still points to parser dependency chains.

All proposed wins require local evidence. Preserve identical data, compiler options, affinity, limits, and cache policy between candidates; retain all five times and use the project's established trimmed-mean convention. Validate byte-exact output and adversarial key/record boundaries before accepting an optimization. The existing plan already specifies the correctness and benchmark matrix; this research adds implementation sources, not a replacement protocol.

## Reading order

Read [Hoyt's progression](https://benhoyt.com/writings/go-1brc/) for the basic cost model, then [the winning source](https://github.com/gunnarmorling/1brc/blob/main/src/main/java/dev/morling/onebrc/CalculateAverage_thomaswue.java) for the combined implementation. Use [Koerkamp's notebook](https://curiouscoding.nl/posts/1brc/) and [the C++ SIMD design](https://github.com/lehuyduc/1brc-simd) when measurements justify lower-level experiments. None of these sources supplies evidence of a particular speedup for Speed Demon.
