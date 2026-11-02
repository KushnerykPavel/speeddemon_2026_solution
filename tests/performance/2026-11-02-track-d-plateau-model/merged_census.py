# Census for a single merged bidirectional adjacency (one entry per unordered neighbour pair per row).
import sys, numpy as np
d = sys.argv[1]
with open(d + '/graph.txt', 'rb') as f:
    n, m = map(int, f.readline().split())
    a = np.fromstring(f.read().decode(), dtype=np.uint64, sep=" ").reshape(-1, 3)
u = a[:, 0].astype(np.int64); v = a[:, 1].astype(np.int64); w = a[:, 2]
print('n', n, 'm', m, 'lines', len(u))
self_loops = int((u == v).sum())
k = u != v; u, v, w = u[k], v[k], w[k]
# collapse parallel/duplicate directed edges to min weight
key = u * n + v
order = np.lexsort((w, key)); key, w = key[order], w[order]
first = np.ones(len(key), bool); first[1:] = key[1:] != key[:-1]
dkey, dw = key[first], w[first]
print('self-loops', self_loops, 'directed lines after self-loop drop', len(key), 'distinct directed pairs', len(dkey),
      'collapsed lines', len(key) - len(dkey))
du, dv = dkey // n, dkey % n
rkey = dv * n + du
has_rev = np.isin(dkey, rkey, assume_unique=True)
print('directed pairs with reverse %.4f' % has_rev.mean(), 'one-way pairs', int((~has_rev).sum()))
undirected = int(has_rev.sum() // 2 + (~has_rev).sum())
merged_entries = 2 * undirected  # each unordered pair appears in both endpoint rows
print('unordered neighbour pairs', undirected, 'merged row entries', merged_entries,
      'vs current fwd+rev entries', 2 * len(key) + 2 * self_loops)
ow = int((~has_rev).sum())
print('per-row entry kinds: both-dir', 2 * int(has_rev.sum() // 2), 'out-only', ow, 'in-only', ow)
deg = np.bincount(np.concatenate([du, dv])[np.concatenate([has_rev | True, ~has_rev])] if False else np.concatenate([du[has_rev], du[~has_rev], dv[~has_rev]]), minlength=n)
print('merged row length histogram (0..8):', np.bincount(deg, minlength=9)[:9].tolist(), 'max', int(deg.max()))
print('vertices with merged row length 2 and both dirs on both entries:',
      int(np.sum(np.bincount(du[has_rev], minlength=n)[deg == 2] == 2)))
for bits in (10, 11, 12, 13, 14, 16, 20, 21):
    print('weights >= 2^%d: %.4f%%' % (bits, 100 * (dw >= (1 << bits)).mean()))
print('weight percentiles 50/90/99/99.9/99.99:', np.percentile(dw.astype(np.float64), [50, 90, 99, 99.9, 99.99]).tolist())
print('head bits for n:', int(np.ceil(np.log2(n))))
