// All-pairs check of `answer_queries` against an independent u64 Dijkstra on random graphs.
// Weight mixes cover bucket-internal relaxations, ring wrap-around, the far heap and u32 overflow.
#[allow(dead_code)]
mod traverse {
    include!("cand-main.rs");

    fn reference(n: usize, edges: &[(usize, usize, u32)], s: usize) -> Vec<u64> {
        let mut adj = vec![Vec::new(); n];
        for &(a, b, w) in edges { adj[a].push((b, w)); }
        let mut d = vec![u64::MAX; n];
        let mut h = BinaryHeap::new();
        d[s] = 0;
        h.push(Reverse((0_u64, s)));
        while let Some(Reverse((x, u))) = h.pop() {
            if x > d[u] { continue; }
            for &(b, w) in &adj[u] {
                if x + (w as u64) < d[b] { d[b] = x + w as u64; h.push(Reverse((d[b], b))); }
            }
        }
        d
    }

    #[test]
    fn ring_search_matches_reference() {
        let mut seed = 0x9E3779B97F4A7C15_u64;
        let mut rnd = move || { seed ^= seed << 13; seed ^= seed >> 7; seed ^= seed << 17; seed };
        let mut checked = 0;
        for round in 0..400 {
            let n = 2 + (rnd() % if round % 4 == 0 { 300 } else { 40 }) as usize;
            let m = (rnd() % (4 * n as u64)) as usize;
            let kind = round % 9;
            let edges: Vec<(usize, usize, u32)> = (0..m).map(|_| {
                let w = match kind {
                    0 => 0,
                    1 => (rnd() % 5) as u32,
                    2 => (rnd() % 1000) as u32,
                    3 => (1 << 31) + (rnd() % 1000) as u32,
                    4 => u32::MAX - (rnd() % 3) as u32,
                    5 => (rnd() % 130) as u32,
                    6 => (rnd() % 600_000) as u32,
                    7 => if rnd() % 4 == 0 { u32::MAX - (rnd() % (1 << 20)) as u32 } else { (rnd() % 300) as u32 },
                    _ => if rnd() % 3 == 0 { (rnd() % 400_000) as u32 } else { (rnd() % 200) as u32 },
                };
                ((rnd() % n as u64) as usize, (rnd() % n as u64) as usize, w)
            }).collect();
            let mut text = format!("{n} {m}\n");
            for &(a, b, w) in &edges { text += &format!("{a} {b} {w}\n"); }
            let graph = build_graph(text.as_bytes(), 3).unwrap();
            let refs: Vec<Vec<u64>> = (0..n).map(|s| reference(n, &edges, s)).collect();
            let all: Vec<(usize, usize)> = (0..n).flat_map(|a| (0..n).map(move |b| (a, b))).collect();
            for workers in [1, 3] {
                let got = answer_queries(&graph, &all, workers);
                for (i, &(a, b)) in all.iter().enumerate() {
                    let want = refs[a][b];
                    assert_eq!(got[i], (want != u64::MAX).then_some(want), "round {round} kind {kind} {a}->{b}");
                    checked += 1;
                }
            }
        }
        println!("checked {checked}");
        assert!(checked > 1_000_000);
    }
}
