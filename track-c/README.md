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
- **Parsing.** Each thread parses its records with a byte-level state machine that follows spec 2.1:
  - a BOM at a record start is skipped;
  - a quoted field ends at a `"` not followed by another `"`;
  - CRLF and LF both end a record, and the final terminator may be missing.
  
  Field ends are found 8 bytes at a time (SWAR). The amount becomes exact integer cents. Each thread keeps per-group counts, sums, min/max and byte totals.
- **Distinct names.** One lock-free open-addressing table (2^18 slots) is shared by all threads. Names are
  kept in their raw quoted form, which is one-to-one with the unescaped value, so nothing is unescaped:
  - a thread claims a slot with a compare-and-swap on its tag, copies the bytes into an append-only arena,
    then publishes the tag;
  - lookups compare full bytes, so ids are exact;
  - each new name gets a dense id (its insertion number, below 2^17);
  - a record ORs its name's id bit into its group's bitset, and `names_distinct` is a popcount.
  
  Past half the slots (over 131,000 names; the generator has about 100,000) a slow exact pass with hash sets
  counts instead.
- **Regions.** Country codes (three capital letters) map through a 26³ table to their rank, which is also
  the output order; `_UNMATCHED` comes last.

Output equals the Python reference on generated tiny and sample datasets for seeds 1–3, full datasets for seeds 1–2, and on the
spec's worked example and edge-case rules, at 1 to 12 threads.

Build: `cargo build --release --offline --locked`.
Run: `target/release/join --data /data --out /out` (`--threads N` overrides the worker count, capped at 12).

## Dependencies

None.

## With another 24 hours

Wider SIMD field scanning, fewer bitset cache misses, a cheaper table probe, and a smaller region table.
