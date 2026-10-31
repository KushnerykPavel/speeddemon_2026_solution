# Official board results — 2026-10-31

Source: [live board](https://speeddemon.dev/board/), its
[full feed](https://s3.amazonaws.com/raptors.dev/speeddemon.dev/2026/board/board.json)
and [Kushneryk history](https://s3.amazonaws.com/raptors.dev/speeddemon.dev/2026/board/runs/kushneryk-d7faec.json).
Full feed generated 2026-10-31 15:19:48 UTC (16:19:48 Europe/Warsaw), history
15:19:40 UTC. Latest watched commit is `b8e5bc17c22d380898a650b399bd04aefb21ec04`.
All 18 published A/D results are successful. B and C remain unchanged starter
tracks, are unregistered, and have no scored optimization results.

Each push reruns both registered tracks, including the one whose code did not
change. The optimization tables in `track-a/PERF.md` and `track-d/PERF.md` use
the first measured submission for each implementation. This complete table
also retains reruns; different times for unchanged code are not new optimizations.
Times are single live-board runs, not a controlled five-run final evaluation.
Memory is the board's M_bytes, converted to decimal MB (1,000,000 bytes).

CSR and release-profile changes for D were submitted together at `f70b655`.
Their combined result is verified, but this does not isolate either change.

## All published A/D attempts

| Ended (UTC) | Commit | Track | Change submitted / rerun | T (s) | M (MB) | Points | S | Run ID |
|---|---|---|---|---:|---:|---:|---:|---|
| 2026-10-30 15:01:39 | `55522c4` | A | Rust baseline | 1.397731 | 62.058496 | 38.980249 | 69 | run-735e9cb7363e |
| 2026-10-30 15:01:52 | `55522c4` | D | Rust baseline | 12.531876 | 256.811008 | 22.057032 | 91 | run-7fd912d41bce |
| 2026-10-30 15:29:31 | `3897216` | A | A: line buffer | 1.087126 | 61.673472 | 40.980844 | 87 | run-aa9a80757414 |
| 2026-10-30 15:29:43 | `3897216` | D | Unchanged-track rerun | 12.360906 | 257.007616 | 22.202853 | 91 | run-dcba6eebf961 |
| 2026-10-30 15:45:08 | `ff39a9a` | A | A: borrowed group keys | 0.938535 | 50.634752 | 42.883192 | 103 | Not published |
| 2026-10-30 15:45:21 | `ff39a9a` | D | Unchanged-track rerun | 12.314639 | 256.798720 | 22.247100 | 91 | Not published |
| 2026-10-30 16:22:45 | `a2217a2` | A | A: byte parser | 0.959642 | 49.434624 | 41.171803 | 167 | Not published |
| 2026-10-30 16:22:58 | `a2217a2` | D | Unchanged-track rerun | 12.415153 | 257.155072 | 22.152818 | 91 | Not published |
| 2026-10-30 16:31:06 | `f70b655` | A | Unchanged-track rerun | 0.964830 | 49.328128 | 41.122630 | 167 | Not published |
| 2026-10-30 16:31:14 | `f70b655` | D | D: CSR + release profile | 7.467422 | 115.048448 | 29.954002 | 132 | Not published |
| 2026-10-30 17:02:20 | `531def7` | A | A: endpoint interning | 0.876075 | 35.188736 | 43.339268 | 183 | run-12ff8c772294 |
| 2026-10-30 17:02:28 | `531def7` | D | Unchanged-track rerun | 7.804536 | 115.023872 | 29.475520 | 132 | Not published |
| 2026-10-31 12:54:30 | `6d6732f` | A | Unchanged-track rerun | 0.895660 | 35.057664 | 43.115427 | 183 | Not published |
| 2026-10-31 12:54:40 | `6d6732f` | D | D: byte parser | 8.915943 | 114.974720 | 27.265802 | 167 | Not published |
| 2026-10-31 13:01:50 | `292002c` | A | Unchanged-track rerun | 0.900004 | 35.160064 | 43.050229 | 183 | Not published |
| 2026-10-31 13:01:58 | `292002c` | D | D: degree rejection | 6.927519 | 115.212288 | 29.716415 | 182 | Not published |
| 2026-10-31 15:19:34 | `b8e5bc1` | A | Unchanged-track rerun | 0.906580 | 35.127296 | 42.975236 | 183 | run-b6806a903087 |
| 2026-10-31 15:19:37 | `b8e5bc1` | D | D: parallel queries | 1.626762 | 228.868096 | 41.704470 | 230 | run-cffe0163d7c1 |

## Evidence and Run ID limitations

- `board-team-snapshot.json`: team entry, rules, dataset and generated_at from the full feed.
- `team-runs.json`: exact compact public history, all 18 entries.
- `normalized-results.json`: same history with full commits resolved locally, plus verified Run IDs.

The public compact history contains commit prefix, timestamps, track, success,
time, memory, source lines, K and points, but omits Run IDs. The official
[history publisher](https://github.com/hackathon-raptors/speed-demon-2026/blob/main/board/runs.py)
explicitly compacts each record this way. Missing IDs are marked, never guessed.
The GitHub combined-status/check-run endpoints inspected for intermediate
submissions contained no matching statuses or checks.

Known Run IDs come from this full snapshot or the previously preserved
`../2026-10-30-ingest-buffer/board-team-snapshot.json` and
`../2026-10-30-ingest-buffer/board-baseline-transcribed.json`. The last file is
explicitly a prior transcription, not an original full-feed snapshot. The
values associated with those baseline IDs match the current public history.

Current best A: `531def7`, 0.876075 s, 35.188736 MB, 43.339268 points,
`run-12ff8c772294`. Latest unchanged A rerun: `b8e5bc1`, 0.906580 s,
35.127296 MB, 42.975236 points, `run-b6806a903087`.
Current best/latest D: `b8e5bc1`, 1.626762 s, 228.868096 MB,
41.704470 points, `run-cffe0163d7c1`.

The D byte-parser submission was slower than its predecessor, while degree
rejection and parallel queries improved the subsequent board times. No code
was changed or reverted as part of this results audit.
