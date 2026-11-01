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
  - a quoted field ends at a `"` not followed by another `"`, and its value is unescaped only when it
    contains `""`;
  - CRLF and LF both end a record, and the final terminator may be missing.
  
  The amount is converted to exact integer cents. Each thread keeps per-group counts, sums, min/max and
  byte totals, plus its own name table and the local ids of each group's names.
- **Merging.** Names get one global id each, and a bitset per group counts the distinct names. The lines
  are written in key order, with `_UNMATCHED` last.

Output equals the Python reference on generated tiny, sample and full datasets for seeds 1–3, and on the
spec's worked example and edge-case rules, at 1 to 12 threads.

Build: `cargo build --release --offline --locked`.
Run: `target/release/join --data /data --out /out` (`--threads N` overrides the worker count, capped at 12).

## Dependencies

None.

## With another 24 hours

One shared name table instead of per-thread tables and a merge, and faster field scanning.
