# Track C step 4: SWAR field scan

- Baseline: `5105fc0` (track-c/src/main.rs, step 3). Candidate: step 4 working tree.
- VPS: 6-vCPU Haswell KVM, Docker with board flags, sd-timer wall time, cgroup `memory.peak`,
  full seed 1, 9 runs per variant (trimmed mean), order c5, c4, c5, c4. `vps-ab.txt` is the raw output
  (`c4` = step 3, `c5` = step 4).
- Correctness: output identical to the official reference on tiny seeds 1–3, sample seeds 1–3,
  full seeds 1–2, the worked example and the edge-case rules file, at 1/2/3/5/8/12 threads.
- Decision: accepted (−7.5% T in both rounds, M unchanged).
