# Track C: Join

The task, the input files and the exact output format are in the [spec](https://github.com/hackathon-raptors/speed-demon-2026/blob/main/specs/c-join.md).

## Approach

Rust, standard library only. `records.csv` and `regions.csv` are mapped read-only.

- **Regions.** `regions.csv` becomes a table indexed by `region_id`. It holds the country's rank among the
  sorted country codes, or the `_UNMATCHED` group for holes and ids beyond the table.
- **Splitting.** After the BOM and the header, the records are split into up to 12 chunks, one per thread.
  A chunk starts after an LF followed by an optional BOM, digits and a comma. Under the generator's
  guarantees (spec 2.2), a line inside a quoted note never looks like that, so every chunk starts at a
  record.
- **Parsing.** Each thread reads its chunk 64 bytes at a time. SIMD compares give bit masks of `"`, `,` and
  LF; a prefix XOR of the quote mask marks the bytes inside quotes (a `""` pair toggles twice), and the
  commas and LFs outside quotes separate the fields (spec 2.1):
  - a BOM at a record start is skipped;
  - unquoted fields never contain `"`, so a field with k quotes has a value `1 + k/2` bytes shorter
    than its raw bytes;
  - CRLF and LF both end a record, and the final terminator may be missing.

  The amount becomes exact integer cents. Each thread keeps per-group counts, sums, min/max and byte totals.
- **Batches.** Records are handled 32 at a time: while a batch fills, each name's table slot and each
  region's group entry are prefetched, so the cache misses of the batch overlap.
- **Distinct names.** One lock-free open-addressing table (2^17 slots) is shared by all threads. A name
  is known by a 64-bit fingerprint of its raw quoted bytes, which are one-to-one with the unescaped value,
  so nothing is unescaped or stored. A thread claims a free slot with a compare-and-swap of the
  fingerprint, and the slot is the name's id. A record ORs its name's id bit into its group's bitset;
  `names_distinct` is a popcount. Two different names share a fingerprint with probability about
  n²/2^65, under 10^-9 for the generator's ~100,000 names (an accepted risk). Past 7/8 of the slots, a
  slow exact pass with hash sets counts instead.
- **Regions.** Country codes (three capital letters) map through a 26³ table to their rank, which is also
  the output order; `_UNMATCHED` comes last. The parsed region rows are freed before the records are read. The region-to-group table uses one byte per region when the
  groups fit.

Output equals the Python reference on generated tiny and sample datasets for seeds 1–3, full datasets for seeds 1–2, and on the
spec's worked example and edge-case rules, at 1 to 12 threads.

Build: `cargo build --release --offline --locked`.
Run: `target/release/join --data /data --out /out` (`--threads N` overrides the worker count, capped at 12).

## Dependencies

None.

## With another 24 hours

A smaller name table (8-byte slots), fewer bitset cache misses, and faster number conversion.
