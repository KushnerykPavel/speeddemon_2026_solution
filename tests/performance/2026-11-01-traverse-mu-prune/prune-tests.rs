#[allow(dead_code)]
mod traverse {
    include!("cand-main.rs");

    fn reference(n: usize, edges: &[(usize, usize, u32)], s: usize) -> Vec<u64> {
        let mut d = vec![u64::MAX; n];
        let mut h = BinaryHeap::new();
        d[s] = 0;
        h.push(Reverse((0_u64, s)));
        while let Some(Reverse((x, u))) = h.pop() {
            if x > d[u] { continue; }
            for &(a, b, w) in edges {
                if a == u && x + (w as u64) < d[b] {
                    d[b] = x + w as u64;
                    h.push(Reverse((d[b], b)));
                }
            }
        }
        d
    }

    #[test]
    fn pruned_bidirectional_matches_reference() {
        let mut seed = 0x9E3779B97F4A7C15_u64;
        let mut rnd = move || { seed ^= seed << 13; seed ^= seed >> 7; seed ^= seed << 17; seed };
        let mut checked = 0;
        for round in 0..300 {
            let n = 2 + (rnd() % 40) as usize;
            let m = (rnd() % (4 * n as u64)) as usize;
            let kind = round % 5;
            let edges: Vec<(usize, usize, u32)> = (0..m).map(|_| {
                let w = match kind {
                    0 => 0,
                    1 => (rnd() % 5) as u32,
                    2 => (rnd() % 1000) as u32,
                    3 => (1 << 31) + (rnd() % 1000) as u32,
                    _ => u32::MAX - (rnd() % 3) as u32,
                };
                ((rnd() % n as u64) as usize, (rnd() % n as u64) as usize, w)
            }).collect();
            let mut text = format!("{n} {m}\n");
            for &(a, b, w) in &edges { text += &format!("{a} {b} {w}\n"); }
            let graph = build_graph(text.as_bytes(), 3).unwrap();
            let all: Vec<(usize, usize)> = (0..n).flat_map(|a| (0..n).map(move |b| (a, b))).collect();
            for workers in [1, 3] {
                let got = answer_queries(&graph, &all, workers);
                for (i, &(a, b)) in all.iter().enumerate() {
                    let want = reference(n, &edges, a)[b];
                    let want = (want != u64::MAX).then_some(want);
                    assert_eq!(got[i], want, "round {round} {a}->{b}");
                    checked += 1;
                }
            }
        }
        assert!(checked > 100000);
    }
}
