# Track D: Dial bucket queue (rejected)

Baseline: `before-main.rs` = scored `1851b13` with `Graph::row` back on
`enumerate()`. Idea source: public Track D code of Gigatron (`solve.cpp`, Dial
ring of 8192 with a pooled bucket list) and FAHHHH (`main.c`, ring of 4096),
who measured 15% and 33% gains over their own binary heaps.

## Variants

- `vec-buckets/dial-main.rs`: bidirectional search kept; each side's
  `BinaryHeap` replaced by a ring of 4096 `Vec<u32>` buckets plus a `far` list
  for labels at least `RING` above the cursor; stale entries skipped on pop;
  sides balanced by queued count. `dial8k-main.rs`: ring of 8192.
- `pooled/pool-main.rs`: the same queue with one pooled linked list for all
  buckets and a 64-bit occupancy bitmap so empty buckets are skipped 64 at a
  time (Gigatron's layout).

## Results (VPS, same protocol as earlier steps; all outputs equal Python)

| Variant-workers | T (s) | Spread | cgroup peak (MB) | search median (s) |
|---|---:|---:|---:|---:|
| before-6 | 0.4977 | 20.7% | 78.3 | 0.260 |
| dial-6 | 0.4923 | 30.7% | 78.0 | 0.272 |
| dial8k-6 | 0.5358 | 16.9% | 78.1 | 0.283 |
| before-12 | 0.4765 | 10.9% | 78.7 | 0.274 |
| dial-12 | 0.5288 | 15.2% | 86.8 | 0.303 |
| dial8k-12 | 0.5386 | 8.6% | 89.7 | 0.316 |

| Variant-workers | T (s) | Spread | cgroup peak (MB) | search median (s) |
|---|---:|---:|---:|---:|
| before-6 | 0.4994 | 29.3% | 78.3 | 0.244 |
| pool-6 | 0.5057 | 22.6% | 78.2 | 0.247 |
| before-12 | 0.5111 | 12.7% | 78.7 | 0.258 |
| pool-12 | 0.5179 | 12.3% | 84.8 | 0.274 |

## Verdict

Rejected. No Dial variant beat the existing heap (single packed `u64` key,
prefetch of the next top's row). At 12 workers every variant raised the
cgroup peak: the search phase overtook the load phase because buckets keep
every pushed entry (stale ones included) until the query ends, while the heap
only holds pending keys. The public gains were measured against slower heaps
and, for FAHHHH, a forward-only search.

Correctness was kept throughout: thirteen Rust tests (with extra random
weights just above the ring size) and 204 Python cases passed for every
variant; a mutation that put far labels into the ring failed four tests.
