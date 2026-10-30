# Ingest group-key allocation experiment — 2026-10-30

Baseline: `389721669dd32ecd60a25e35012e1e33e29f2896`.
Candidate: working-tree change captured in `candidate.patch`; exact source hashes
are in `vps-summary.json`. No candidate commit or board run exists at recording time.

Change: inline `[u8; 13]` hour keys, nested endpoint maps, borrowed endpoint lookup;
owned endpoint strings are created only for new groups. Exact grouping, bytewise
output ordering and percentile behavior are preserved.

## Measurements

| Round | Before (s) | After (s) | Included in measured set |
|---|---|---|---|
| 0 | 1.859515856 | 1.386299021 | No: warmup |
| 1 | 1.721331717 | 1.391991727 | Yes |
| 2 | 1.802242365 | 1.301971433 | Yes |
| 3 | 1.624429540 | 1.292827523 | Yes |
| 4 | 1.573175956 | 1.292025052 | Yes |
| 5 | 1.578237530 | 1.240763753 | Yes |

After dropping min/max independently: 1.641332929 s before, 1.295608003 s after;
1.266843772x speedup, 21.06366845% less time. Shared-host variation means this is
an approximate local result. No memory or source-size score was measured.

Same [protocol and pinned images as the previous experiment](../2026-10-30-ingest-buffer/README.md):
Linux x86_64 `my-vps`, six logical CPUs, pin to CPU 2, rustc 1.99.1,
`cargo build --release --offline --locked`, full official seed 1 (3.1 million rows,
238,836,234 bytes), warm input page cache. One warmup plus five measurements per
version, alternating order. `Instant` measures the child process launch through
exit; Docker startup and correctness comparisons are excluded. CPU model/governor
and peak memory were not recorded locally; other host work was not excluded.

## Evidence and reproduction

- `vps-raw.txt`: complete original build/timing log and output SHA-256.
- `vps-summary.json`: conditions, every raw run, calculated statistics and source hashes.
- `runner.rs`: exact timing and byte-comparison runner.
- `candidate.patch`: exact measured change relative to the baseline commit.

Follow the linked build/run protocol, exporting baseline into `/work/before` and
baseline plus `candidate.patch` into `/work/after`. The runner expects release
binaries in their respective `target/release/ingest-baseline` paths.
Original remote artifacts: `my-vps:/tmp/ingest-speed-_ahx0au0`; temporary paths may expire.

Every local VPS run matched byte-for-byte. Full seed-1 output SHA-256:
`495ca09682b9673ef836a506496177680bb4a3af54255174befddb1f83481cd3`.
Local Python comparisons also passed the existing percentile/date/order fixtures,
empty input, CRLF, no final newline and synthetic 20,000-row cases with seeds
1, 7, 42, shuffled hours, prefix endpoints and u32 boundary values.
Synthetic cases do not replace the official multi-seed generator matrix, which
was not rerun here. Formatting, Clippy and release compilation passed.

No official performance claim is made until the candidate is measured on the board.
