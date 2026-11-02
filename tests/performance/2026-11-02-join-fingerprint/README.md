# Track C step 6: name fingerprints and freed region rows (2026-11-02)

Raw runs: `vps-runs.txt`. Variant sources: `c-fp-main.rs` (fingerprints only), `c-fp2-main.rs` (+ two-pass
regions and SWAR numbers), `c-fp3-main.rs` (+ one-pass regions dropped, SWAR numbers); the committed
`track-c/src/main.rs` is fp4 (fingerprints + one-pass regions dropped, original number conversion).
The calibration binary is Cache Me's public Track C (`manusingh090/speed-demon-2026-entry` @ `05e40e1`, MIT),
built from its own Dockerfile and only renamed to `/app/solution` for the harness; its source is not copied here.
Correctness, protocol and the decision are in `track-c/PERF.md` step 6.
