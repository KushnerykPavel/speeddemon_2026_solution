# Track D: SWAR record parsing

Baseline: `7d0e8ab` (`before-main.rs`, digit-by-digit strict parser). Candidate:
`swar-main.rs` (scored source). `profile-*-main.rs` add stderr phase timers only.

## Change

`fast_field` reads one 8-byte little-endian word: the first byte with bit 4
clear ends the number; every byte before it must be `0x30..=0x39` (high nibble 3
and low nibble + 6 below 16); the separator must be exactly `' '` (or `'\n'` for
the weight). Digits are shifted to the top and combined in pairs, quads and
octets with three multiplies. `fast_record` parses `u v w\n` this way; any
other shape (CRLF, tabs, repeated spaces, more than seven digits, the chunk tail,
invalid bytes) falls back to the unchanged strict line parser, so accepted input
and errors are as before. Idea credit: Team7's public Track D parser (MIT).

## Protocol

`my-vps`, six Haswell KVM vCPUs, CPU set 0-5, 8 GB cgroup, pinned Rust image,
offline native release builds. Full official seed 1, prewarmed inputs, one
warmup plus five rotating-order runs, trimmed mean of the middle three, cgroup
v2 `memory.peak`. Phase profile: four runs per variant and worker count,
medians. All outputs byte-equal to Python.

| Variant-workers | T (s) | Spread | cgroup peak (MB) |
|---|---:|---:|---:|
| `7d0e8ab`-6 | 0.4966 | 26.4% | 96.4 |
| swar-6 | 0.4218 | 18.1% | 96.6 |
| `7d0e8ab`-12 | 0.4951 | 11.2% | 101.2 |
| swar-12 | 0.4428 | 10.1% | 101.6 |

| Phase median (s) | `7d0e8ab` 6w | SWAR 6w | `7d0e8ab` 12w | SWAR 12w |
|---|---:|---:|---:|---:|
| graph load | 0.163 | 0.116 | 0.137 | 0.106 |
| renumbering | 0.080 | 0.086 | 0.081 | 0.085 |
| search (incl. transpose) | 0.232 | 0.248 | 0.243 | 0.247 |

## Correctness

Sixteen Rust tests (`measured-swar-tests.rs`) pass on macOS and VPS. New:
200,000 random records with neighbouring bytes (`/`, `:`, `@`, `p`, 0x10, 0xB5),
tabs, CR, double spaces, 1–10 digit numbers, `u32::MAX` and `u32::MAX + 1`:
whenever the fast path accepts a record, its value and length equal the strict
parser's; fixed cases cover seven digits with leading zeros, eight digits,
missing tail and CRLF. Mutations caught: skipping the digit check (1 test),
a wrong pair-combine multiplier (3 tests). `traverse-check.py`: 204 cases
byte-equal to Python under default and 1/2/4/6/12 workers. Clippy and rustfmt pass.
