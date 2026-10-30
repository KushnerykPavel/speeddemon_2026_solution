# Traverse release-profile experiment — 2026-10-30

Same current CSR/array-distance implementation in both variants. Only build settings
change: default release versus `lto="fat"`, `codegen-units=1`, `panic="abort"`,
plus `RUSTFLAGS="-C target-cpu=native"` for the candidate. This measures the combined
settings; it does not isolate each flag or measure the earlier CSR change.

## Result

| Round | Default release (s) | Tuned release (s) | Use |
|---|---|---|---|
| 0 | 15.921458108 | 17.756333987 | warmup |
| 1 | 18.135001941 | 17.457003298 | measured |
| 2 | 18.235619201 | 17.525414698 | measured |
| 3 | 19.688484287 | 17.184048557 | measured |
| 4 | 18.650938718 | 17.622001528 | measured |
| 5 | 15.351041382 | 17.240244180 | measured |

Five measured runs per variant, discarding each variant's min and max:
**18.340519953 s → 17.407554059 s**, 1.053595462× speedup, **5.0869% less time**.
Medians: 18.235619201 s → 17.457003298 s.
Baseline max/min spread is 28.255%; candidate spread is 2.549%.
The small observed improvement is **inconclusive on this shared VPS**; these data
are not enough to claim a reliable speedup or an official scoring benefit.
Keep the settings provisionally pending a scoring-machine measurement.

All twelve outputs, including warmups, match the saved Python full-seed1 reference
byte-for-byte. The reference SHA-256 was checked before the run:
`a23dbdf0a3e49cc8901310a9ee1dc6a9a569a85297b6da75c2a7002cb2ed277e`.
No new multi-seed Python validation was run in this experiment.

## Conditions and reproduction

- Host: `my-vps`, Linux x86_64 KVM, Haswell with AVX2, six logical CPUs;
  benchmark pinned to CPU 2. Different CPU from the scoring machine.
- Compiler: rustc 1.99.1 (58cf8f9c3 2026-10-13), LLVM 23.1.1.
- Official full seed 1: 1,740,512 nodes, 5,000,000 edges, 2,000 queries.
- `cargo build --release --offline --locked` in the pinned Rust image from
  `track-d/Dockerfile`; only candidate receives native CPU flags. Builds run on
  the same VPS as execution. Candidate compilation 5.29 s versus 0.71 s default;
  build time is excluded from runtime measurements.
- Pinned Debian runtime image from that Dockerfile; no network, read-only root,
  8 GB memory/no swap, unprivileged user, dropped capabilities, 512 PID limit.
- Warmed input page cache; one warmup and five measurements per variant;
  alternating before/after and after/before ordering.
- Rust `Instant` wraps child process launch through exit in one long-lived
  container. Docker startup, hashing and byte comparison are excluded.
- No cgroup peak memory or points measured; unrelated VPS work not excluded.

`remote-experiment.py` is the exact self-contained remote driver, including source
and manifests for both variants. Run by feeding it to `ssh my-vps python3 -`.
It creates a fresh temporary directory and needs the existing images, data and
Python reference paths shown in the script. `runner.rs` is the exact timing and
correctness runner. `measured-main.rs` preserves the shared source independently;
`source-metadata.json` records SHA-256 and local HEAD. The measured source includes
working-tree CSR changes, so local HEAD alone does not identify the measured code.
`candidate.patch` records build-setting changes relative to local HEAD.

`vps-raw.txt` contains commands, compiler output and every timing;
`vps-summary.json` contains raw data and statistics; `cpu.txt` records CPU details.
Original remote artifacts: `my-vps:/tmp/traverse-build-t9o4c8h9` (temporary).
No remote checkout was modified. No live-board run ID exists.
