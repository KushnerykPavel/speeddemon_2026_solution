# Track D: search tail profile (diagnostic)

Build: `21df4fd` with timers only (`tail-main.rs`): transpose time, each
query's begin/end on its worker, each worker's finish time. VPS, CPU set 0-5,
full seed 1, four rounds at 6 and 12 workers, outputs equal to Python. Raw
rows: `tail.jsonl.gz` (one JSON line per run; `queries` = `[index, worker,
begin_s, end_s, found]`).

| Run | Transpose (ms) | Search makespan (ms) | Ideal = work / workers (ms) | Last − first worker end (ms) | Max query (ms) | Top-100 queries' share of work |
|---|---:|---:|---:|---:|---:|---:|
| 6w r0 | 59.7 | 186.1 | 183.6 | 4.0 | 11.7 | 52% |
| 6w r1 | 50.8 | 163.6 | 160.9 | 3.9 | 10.6 | 52% |
| 6w r2 | 55.1 | 201.1 | 196.6 | 6.2 | 20.7 | 55% |
| 6w r3 | 62.4 | 201.4 | 198.0 | 5.1 | 21.7 | 54% |
| 12w r0 | 54.7 | 218.7 | 211.3 | 9.9 | 42.6 | 57% |
| 12w r1 | 51.3 | 177.3 | 170.7 | 8.3 | 22.7 | 54% |
| 12w r2 | 61.4 | 201.7 | 196.7 | 6.4 | 49.1 | 56% |
| 12w r3 | 53.1 | 189.5 | 182.1 | 8.1 | 26.2 | 54% |

Findings:
- The tail is small: the search makespan is within 1–4% of perfect balance
  (2.5–7.6 ms). The atomic-counter dispatch already hides long queries; query
  reordering cannot gain more than that.
- Median query 0.03 ms; the 100 longest queries (5%) are about half of all
  search work. The same indices are longest in every run (782, 678, 763,
  1658, 526, ...).
- Transpose costs 51–62 ms, about 11% of the run and a quarter of the search
  phase, more than the whole tail.
