# Track D: profile-guided optimization (rejected)

Source: `21df4fd` (unchanged). PGO is allowed by the rules (training on
generator output; training time does not count).

## Method

`setup.py`: in the pinned `rust:1-slim-trixie` image (rustc 1.99.1, LLVM
23.1.1, `llvm-tools` added from rustup), build with `-C profile-generate`,
run on generator data **excluding seed 1** (full seeds 11 and 12, sample
seeds 3 and 5, each at 1, 6 and 12 threads) and merge into `pgo.profdata`
(46 KB). `experiment.py`: build plain and with
`-C target-cpu=native -C profile-use=pgo.profdata`, measure on the held-out
full seed 1 (VPS, CPU set 0-5, eleven rotating-order runs, trimmed mean).
`experiment1.py`: the same at 1 and 3 threads, nine runs, where run-to-run
noise is lower. All outputs equal to Python; thirteen Rust tests and 204
Python cases pass.

## Results

| Threads | plain (s) | PGO (s) | change | spread plain / PGO |
|---|---:|---:|---:|---|
| 1 | 2.2828 | 2.2838 | 0.0% | 15.2% / 6.5% |
| 3 | 0.8919 | 0.8930 | +0.1% | 10.5% / 17.8% |
| 6 | 0.4900 | 0.5037 | +2.8% | 19.5% / 36.3% |
| 12 | 0.4847 | 0.4677 | −3.5% | 22.6% / 14.7% |

Peak memory unchanged (78–79 MB).

## Verdict

No effect: single-thread time is identical, and the 6- and 12-thread
differences have opposite signs and sit inside a 15–36% spread. The run is
bound by cache misses (the search settles ~7 million vertices at memory
latency), not by branches or code layout, so PGO has nothing to improve.
It would also add a binary profile that must be rebuilt after every source
change. Not adopted.
