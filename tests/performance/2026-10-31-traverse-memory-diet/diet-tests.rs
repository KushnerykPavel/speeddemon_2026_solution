#[allow(dead_code)]
mod traverse {
    include!("balanced-main.rs");

    /// Reorders `bytes` with `workers`, then checks the vertex permutation, the
    /// remapped edge multiset and every answer in `pairs`.
    fn check_reorder(bytes: &[u8], pairs: &[(usize, usize)], workers: usize) {
        let graph = build_graph(bytes, workers).unwrap();
        let nodes = graph.offsets.len() - 1;
        // Self queries first: after reordering they reveal the vertex mapping.
        let original: Vec<_> = (0..nodes)
            .map(|node| (node, node))
            .chain(pairs.iter().copied())
            .collect();
        let expected = answer_queries(&graph, &original, 1);
        let old_edges: Vec<_> = (0..nodes)
            .flat_map(|source| {
                graph
                    .row(source)
                    .map(move |(target, weight)| (source, target as usize, weight))
            })
            .collect();
        let mut queries = original;
        let reordered = reorder_graph(graph, &mut queries, workers);
        let mapping: Vec<_> = queries[..nodes].iter().map(|&(source, _)| source).collect();
        let mut permutation = mapping.clone();
        permutation.sort_unstable();
        assert_eq!(permutation, (0..nodes).collect::<Vec<_>>());
        let mut expected_edges: Vec<_> = old_edges
            .iter()
            .map(|&(source, target, weight)| (mapping[source], mapping[target], weight))
            .collect();
        let mut actual_edges: Vec<_> = (0..nodes)
            .flat_map(|source| {
                reordered
                    .row(source)
                    .map(move |(target, weight)| (source, target as usize, weight))
            })
            .collect();
        expected_edges.sort_unstable();
        actual_edges.sort_unstable();
        assert_eq!(actual_edges, expected_edges);
        for search_workers in [1, 2, 6, 12] {
            assert_eq!(
                answer_queries(&reordered, &queries, search_workers),
                expected
            );
        }
    }

    #[test]
    fn reordering_preserves_permutation_edges_and_all_distances() {
        let bytes = b"130 9\n5 64 0\n64 5 4294967295\n64 129 4294967295\n129 128 4294967295\n5 5 0\n66 67 1\n67 66 2\n5 64 7\n5 64 0\n";
        let pairs: Vec<_> = (0..130)
            .flat_map(|source| (0..130).map(move |target| (source, target)))
            .collect();
        let graph = build_graph(bytes, 1).unwrap();
        assert_eq!(
            answer_queries(&graph, &[(5, 128)], 1),
            [Some(2 * u64::from(u32::MAX))]
        );
        for workers in [1, 2, 6, 12] {
            check_reorder(bytes, &pairs, workers);
        }
        let empty = reorder_graph(build_graph(b"0 0\n", 6).unwrap(), &mut [], 6);
        assert_eq!(empty.offsets, [0]);
        assert!(empty.edges.is_empty());
    }

    #[test]
    fn concurrent_region_claims_cover_every_vertex_exactly_once() {
        // Random sparse graph with several components, so worker regions race
        // for the same vertices and many roots start new regions.
        let (nodes, edge_count) = (3_000_usize, 7_000_usize);
        let mut state = 0x9E37_79B9_7F4A_7C15_u64;
        let mut next = |bound: usize| {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            (state % bound as u64) as usize
        };
        let mut text = format!("{nodes} {edge_count}\n");
        for _ in 0..edge_count {
            let source = next(nodes);
            let target = (source + next(40)) % nodes;
            text += &format!("{source} {target} {}\n", next(1_000));
        }
        let pairs: Vec<_> = (0..400).map(|_| (next(nodes), next(nodes))).collect();
        for _ in 0..10 {
            for workers in [2, 6, 12] {
                check_reorder(text.as_bytes(), &pairs, workers);
            }
        }
    }

    #[test]
    fn graph_loader_preserves_every_edge() {
        let directory = std::env::temp_dir().join(format!("traverse-csr-{}", std::process::id()));
        fs::create_dir(&directory).unwrap();
        let path = directory.join("graph.txt");
        fs::write(
            &path,
            b"5 7\r\n2 3 4294967295\r\n0 1 9\r\n0 1 0\r\n1 1 0\r\n2 3 4294967295\r\n3 0 4\r\n0 2 5",
        )
        .unwrap();
        let expected = [
            vec![(1, 0), (1, 9), (2, 5)],
            vec![(1, 0)],
            vec![(3, u32::MAX), (3, u32::MAX)],
            vec![(0, 4)],
            vec![],
        ];
        for workers in [1, 2, 3, 6, 12, 99] {
            let graph = read_graph(&path, workers).unwrap();
            assert_eq!(graph.offsets, [0, 3, 4, 6, 7, 7]);
            for (node, expected) in expected.iter().enumerate() {
                let mut edges: Vec<_> = graph.row(node).collect();
                edges.sort_unstable();
                assert_eq!(&edges, expected);
            }
            assert_eq!(transpose(&graph, workers).offsets, [0, 1, 4, 5, 7, 7]);
        }
        for nodes in [0, 5, 65] {
            fs::write(&path, format!("{nodes} 0\n")).unwrap();
            let graph = read_graph(&path, 6).unwrap();
            assert_eq!(graph.offsets, vec![0; nodes + 1]);
            assert!(graph.edges.is_empty());
            assert!(transpose(&graph, 6).edges.is_empty());
        }
        fs::write(&path, b"").unwrap();
        let mapping = MappedFile::open(&path).unwrap();
        assert!(mapping.bytes().is_empty());
        assert!(build_graph(mapping.bytes(), 6).is_err());
        drop(mapping);
        fs::remove_file(path).unwrap();
        fs::remove_dir(directory).unwrap();
    }

    #[test]
    fn chunks_cover_input_without_splitting_lines() {
        for length in 0..=150 {
            let mut bytes = Vec::new();
            for index in 0..length {
                bytes.extend_from_slice(if index % 2 == 0 {
                    b"1 2 3\r\n"
                } else {
                    b"4 5 6\n"
                });
            }
            bytes.extend_from_slice(b"7 8 9");
            for workers in 1..=20 {
                let chunks = line_chunks(&bytes, workers);
                assert_eq!(chunks.concat(), bytes);
                for chunk in &chunks[..chunks.len().saturating_sub(1)] {
                    assert_eq!(chunk.last(), Some(&b'\n'));
                }
                let seen = AtomicUsize::new(0);
                let (count, states) = parse_chunks(
                    &chunks,
                    || 0_usize,
                    |records, _| {
                        *records += 1;
                        seen.fetch_add(1, Ordering::Relaxed);
                        Ok(())
                    },
                )
                .unwrap();
                assert_eq!(states.len(), chunks.len());
                assert_eq!(states.iter().sum::<usize>(), length + 1);
                assert_eq!(count, length + 1);
                assert_eq!(seen.into_inner(), length + 1);
            }
        }
        assert!(line_chunks(b"", 12).is_empty());
        for bytes in [&b"1 2 3\n\n"[..], &b"1 2 4294967296\n"[..], &b"1 2\n"[..]] {
            for workers in [1, 6, 12] {
                assert!(parse_chunks(&line_chunks(bytes, workers), || (), |_, _| Ok(())).is_err());
            }
        }
    }

    #[test]
    fn parallel_scatter_preserves_edges_under_cursor_contention() {
        // Three hot source rows in different 1,024-vertex parts, plus one edge from
        // every vertex, so rows in every part and part boundaries are exercised.
        let (nodes, hot) = (5_000, [0, 1_023, 4_999]);
        let count = 20_000;
        let mut bytes = format!("{nodes} {}\n", count + nodes).into_bytes();
        let mut expected = vec![Vec::new(); nodes];
        let edges = (0..count)
            .map(|index| (hot[index % 3], index % 130))
            .chain((0..nodes).map(|source| (source, (source * 7) % nodes)));
        for (index, (source, target)) in edges.enumerate() {
            // Odd edges get weights spread over all of u32, so many need the
            // heavy list (escape code) in both the forward and reverse graph.
            let weight = if index % 2 == 1 {
                (index as u32).wrapping_mul(2_654_435_761)
            } else {
                index as u32
            };
            writeln!(bytes, "{source} {target} {weight}").unwrap();
            expected[source].push((target as u32, weight));
        }
        let mut reversed = vec![Vec::new(); nodes];
        for (source, edges) in expected.iter_mut().enumerate() {
            edges.sort_unstable();
            for &(target, weight) in edges.iter() {
                reversed[target as usize].push((source as u32, weight));
            }
        }
        for sources in &mut reversed {
            sources.sort_unstable();
        }
        for workers in [1, 2, 4, 6, 12] {
            for _ in 0..3 {
                let graph = build_graph(&bytes, workers).unwrap();
                for (node, expected) in expected.iter().enumerate() {
                    let mut edges: Vec<_> = graph.row(node).collect();
                    edges.sort_unstable();
                    assert_eq!(&edges, expected);
                }
                let reverse = transpose(&graph, workers);
                for (node, sources) in reversed.iter().enumerate() {
                    let mut edges: Vec<_> = reverse.row(node).collect();
                    edges.sort_unstable();
                    assert_eq!(&edges, sources);
                }
            }
        }
    }

    #[test]
    fn parallel_answers_match_serial_with_skewed_work_and_duplicates() {
        let nodes = 130;
        let mut text = format!("{nodes} {}\n", nodes - 1);
        for source in 0..nodes - 1 {
            text += &format!("{source} {} {}\n", source + 1, u32::MAX);
        }
        let graph = build_graph(text.as_bytes(), 3).unwrap();
        let queries: Vec<_> = (0..1000)
            .map(|index| match index % 5 {
                0 => (0, 129),
                1 => (129, 0),
                2 => (index % nodes, index % nodes),
                3 => (3, 4),
                _ => (1, 64),
            })
            .collect();
        let backward = transpose(&graph, 1);
        let wide = Mutex::new(None);
        let mut searcher = Searcher::new(nodes, &wide);
        let expected: Vec<_> = queries
            .iter()
            .map(|&(source, target)| searcher.shortest_path(&graph, &backward, source, target))
            .collect();
        assert_eq!(expected[0], Some(129 * u64::from(u32::MAX)));
        for workers in [1, 2, 4, 6, 12, 99] {
            for _ in 0..5 {
                assert_eq!(answer_queries(&graph, &queries, workers), expected);
            }
            assert!(answer_queries(&graph, &[], workers).is_empty());
            assert_eq!(
                answer_queries(&graph, &[(0, 129)], workers),
                vec![expected[0]]
            );
            assert_eq!(
                answer_queries(&graph, &queries[..3], workers),
                expected[..3]
            );
        }
    }

    #[test]
    fn degree_checks_preserve_self_paths_and_reusable_state() {
        let graph = build_graph(b"4 2\n0 1 7\n1 2 0\n", 1).unwrap();
        let backward = transpose(&graph, 1);
        let wide = Mutex::new(None);
        let mut searcher = Searcher::new(4, &wide);
        for (source, target, expected) in [
            (3, 3, Some(0)), // isolated self path wins over both degree checks
            (0, 0, Some(0)), // no-in self path
            (0, 2, Some(7)),
            (1, 0, None),    // no-in target after ordinary search
            (2, 1, None),    // no-out source
            (0, 1, Some(7)), // reuse after both fast rejections
        ] {
            assert_eq!(
                searcher.shortest_path(&graph, &backward, source, target),
                expected
            );
            assert_clean(&searcher.narrow);
            assert!(wide.lock().unwrap().is_none());
        }
    }

    /// Both sides of a finished search are reset for reuse.
    fn assert_clean<D: Distance + std::fmt::Debug>(search: &Search<D>) {
        for side in [&search.forward, &search.backward] {
            assert!(side.used_blocks.is_empty() && side.heap.is_empty());
            assert!(side.page_of_block.iter().all(|&page| page == NO_PAGE));
            assert!(side.pages.iter().all(|&distance| distance == D::MAX));
        }
    }

    /// Plain forward Dijkstra in u64 over an edge list, independent of the CSR code.
    fn reference_distance(
        nodes: usize,
        edges: &[(usize, usize, u32)],
        source: usize,
        target: usize,
    ) -> Option<u64> {
        let mut distances = vec![u64::MAX; nodes];
        let mut heap = BinaryHeap::from([Reverse((0_u64, source))]);
        distances[source] = 0;
        while let Some(Reverse((distance, node))) = heap.pop() {
            if distance > distances[node] {
                continue;
            }
            if node == target {
                return Some(distance);
            }
            for &(_, next, weight) in edges.iter().filter(|edge| edge.0 == node) {
                let candidate = distance + u64::from(weight);
                if candidate < distances[next] {
                    distances[next] = candidate;
                    heap.push(Reverse((candidate, next)));
                }
            }
        }
        None
    }

    #[test]
    fn bidirectional_matches_reference_across_u32_boundary() {
        let mut state = 0x2545_F491_4F6C_DD1D_u64;
        let mut next = |bound: u64| {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            state % bound
        };
        for round in 0..60 {
            let nodes = 2 + next(40) as usize;
            let count = next(4 * nodes as u64) as usize;
            // Odd rounds spread vertex IDs over many distance pages.
            let stride = if round % 2 == 1 { PAGE + 7 } else { 1 };
            let label = |node: usize| node * stride;
            let mut edges = Vec::new();
            let mut text = format!("{} {count}\n", label(nodes - 1) + 1);
            for _ in 0..count {
                let (source, target) = (next(nodes as u64) as usize, next(nodes as u64) as usize);
                // Mix zero, small, near-2^31 and near-2^32 weights so sums cross u32.
                let weight = match next(5) {
                    0 => 0,
                    1 => next(10) as u32,
                    2 => (1 << 31) + next(3) as u32 - 1,
                    3 => u32::MAX - next(3) as u32,
                    _ => next(1_000) as u32,
                };
                edges.push((source, target, weight));
                text += &format!("{} {} {weight}\n", label(source), label(target));
            }
            let graph = build_graph(text.as_bytes(), 1 + round % 6).unwrap();
            let pairs: Vec<_> = (0..nodes)
                .flat_map(|source| (0..nodes).map(move |target| (source, target)))
                .collect();
            let expected: Vec<_> = pairs
                .iter()
                .map(|&(source, target)| reference_distance(nodes, &edges, source, target))
                .collect();
            let queries: Vec<_> = pairs
                .iter()
                .map(|&(source, target)| (label(source), label(target)))
                .collect();
            for workers in [1, 3, 12] {
                assert_eq!(
                    answer_queries(&graph, &queries, workers),
                    expected,
                    "round {round}"
                );
            }
        }
    }

    #[test]
    fn swar_records_match_the_strict_parser() {
        let mut state = 0x853C_49E6_748F_EA9B_u64;
        let mut next = |bound: u64| {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            state % bound
        };
        // Bytes next to the digit range, separators and bytes with bit 4 clear.
        let noise = [
            b'/', b':', b'@', b'p', 0x10, 0xB5, b'\t', b'\r', b' ', b'0', b'9',
        ];
        let mut fast = 0;
        for _ in 0..200_000 {
            let mut text = Vec::new();
            for field in 0..3 {
                let digits = 1 + next(10) as usize;
                let mut number: String = (0..digits)
                    .map(|_| char::from(b'0' + next(10) as u8))
                    .collect();
                match next(20) {
                    0 => number = "4294967296".into(),
                    1 => number = "4294967295".into(),
                    2 => number.insert(
                        next(digits as u64) as usize,
                        char::from(noise[next(11) as usize]),
                    ),
                    _ => {}
                }
                text.extend_from_slice(number.as_bytes());
                text.extend_from_slice(match (field, next(25)) {
                    (2, 0) => b"\r\n",
                    (2, _) => b"\n",
                    (_, 0) => b"\t",
                    (_, 1) => b"  ",
                    _ => b" ",
                });
            }
            // A following record keeps the 8-byte loads inside the slice.
            text.extend_from_slice(b"1 2 3\n");
            let line = &text[..text.iter().position(|&byte| byte == b'\n').unwrap() + 1];
            match fast_record(&text) {
                Some((record, used)) => {
                    fast += 1;
                    assert_eq!(used, line.len());
                    assert_eq!(parse_fields::<3>(line).unwrap(), record, "{:?}", line);
                }
                None => {}
            }
        }
        assert!(fast > 20_000, "fast path taken {fast} times");
        for (text, expected) in [
            (&b"0 0 0\n1 2 3\n"[..], Some(([0, 0, 0], 6))),
            (
                b"9999999 1 0000042\n1 2 3\n",
                Some(([9_999_999, 1, 42], 18)),
            ),
            (b"12345678 1 2\n1 2 3\n", None),
            (b"1 2 3", None),
            (b"1 2 3\r\n1 2 3\n", None),
        ] {
            assert_eq!(fast_record(text), expected);
        }
    }

    #[test]
    fn packing_sizes_heads_and_escapes_weights() {
        for (nodes, head_bits) in [
            (0, 1),
            (1, 1),
            (2, 1),
            (3, 2),
            (1 << 21, 21),
            ((1 << 21) + 1, 22),
        ] {
            let packing = Packing::for_nodes(nodes).unwrap();
            assert_eq!(packing.head_bits, head_bits, "{nodes} vertices");
            let escape = packing.escape();
            let heavy = Mutex::new(Vec::new());
            let head = (nodes.max(1) - 1) as u32;
            for (index, weight) in [0, 1, escape - 1, escape, u32::MAX].into_iter().enumerate() {
                let packed = packing.pack(index, head, weight, &heavy);
                assert_eq!(packed & ((1 << head_bits) - 1), head);
                assert_eq!(packed >> head_bits, weight.min(escape));
            }
            assert_eq!(heavy.into_inner().unwrap(), [(3, escape), (4, u32::MAX)]);
        }
        assert!(Packing::for_nodes(1 << 31).is_ok());
        assert!(Packing::for_nodes((1 << 31) + 1).is_err());
    }

    #[test]
    fn narrow_distances_fall_back_to_u64_exactly() {
        let max = u32::MAX;
        // 0 -> 1 costs MAX - 1 (largest narrow answer); 0 -> 2 costs MAX (sentinel,
        // needs u64); 0 -> 3 is cheap via 4 although the direct edge overflows;
        // 0 -> 5 needs MAX + 2; unreachable searches that overflowed retry in u64.
        let bytes = format!(
            "8 9\n0 1 {}\n0 2 {max}\n0 4 1\n4 3 1\n0 3 {max}\n3 5 {max}\n5 5 0\n1 6 {max}\n4 6 5\n",
            max - 1
        );
        let graph = build_graph(bytes.as_bytes(), 3).unwrap();
        let cases = [
            ((0, 1), Some(u64::from(max - 1))),
            ((0, 2), Some(u64::from(max))),
            ((0, 3), Some(2)),
            ((0, 5), Some(u64::from(max) + 2)),
            ((0, 6), Some(6)),
            ((1, 2), None),
            ((0, 7), None),
            ((4, 1), None),
        ];
        let backward = transpose(&graph, 1);
        let wide = Mutex::new(None);
        let mut searcher = Searcher::new(8, &wide);
        for _ in 0..2 {
            for &((source, target), expected) in &cases {
                assert_eq!(
                    searcher.shortest_path(&graph, &backward, source, target),
                    expected
                );
                assert_clean(&searcher.narrow);
                if let Some(wide) = &*wide.lock().unwrap() {
                    assert_clean(wide);
                }
            }
        }
        assert!(wide.lock().unwrap().is_some());
        let queries: Vec<_> = cases.iter().map(|&(query, _)| query).collect();
        let expected: Vec<_> = cases.iter().map(|&(_, answer)| answer).collect();
        for workers in [1, 2, 6, 12] {
            assert_eq!(answer_queries(&graph, &queries, workers), expected);
        }
    }

    #[test]
    fn records_across_every_buffer_boundary() {
        let bytes = b" \t0 4294967295 00042\r\n1\t2 3\n4 5 6";
        for capacity in 1..=bytes.len() {
            let mut reader = BufReader::with_capacity(capacity, &bytes[..]);
            let mut buffer = Vec::new();
            assert_eq!(
                read_record(&mut reader, &mut buffer).unwrap(),
                Some([0, u32::MAX, 42])
            );
            assert_eq!(
                read_record(&mut reader, &mut buffer).unwrap(),
                Some([1, 2, 3])
            );
            assert_eq!(
                read_record(&mut reader, &mut buffer).unwrap(),
                Some([4, 5, 6])
            );
            assert_eq!(read_record::<3>(&mut reader, &mut buffer).unwrap(), None);
        }
    }

    #[test]
    fn rejects_malformed_or_overflowing_fields() {
        for bytes in [
            &b"\n"[..],
            &b" \t\r\n"[..],
            &b"1"[..],
            &b"1 2 3"[..],
            &b"-1 2"[..],
            &b"1 +2"[..],
            &b"1 2x"[..],
            &b"1 4294967296"[..],
            &b"999999999999999999999999999999 2"[..],
            &b"1 \xff"[..],
            &b"1.0 2"[..],
            &b"1\x002"[..],
        ] {
            let error = read_record::<2>(&mut &bytes[..], &mut Vec::new()).unwrap_err();
            assert_eq!(error.kind(), io::ErrorKind::InvalidData, "{bytes:?}");
        }
    }

    #[test]
    fn empty_input_is_eof() {
        assert_eq!(
            read_record::<2>(&mut &b""[..], &mut Vec::new()).unwrap(),
            None
        );
    }

    #[test]
    fn long_records_reuse_buffer() {
        let bytes = format!("{}1 2\n3 4\n", " ".repeat(32_768));
        let mut reader = BufReader::with_capacity(7, bytes.as_bytes());
        let mut buffer = Vec::new();
        assert_eq!(read_record(&mut reader, &mut buffer).unwrap(), Some([1, 2]));
        let capacity = buffer.capacity();
        assert_eq!(read_record(&mut reader, &mut buffer).unwrap(), Some([3, 4]));
        assert_eq!(buffer.capacity(), capacity);
    }

    #[test]
    fn propagates_read_failure_after_partial_record() {
        struct Fails;
        impl io::Read for Fails {
            fn read(&mut self, _: &mut [u8]) -> io::Result<usize> {
                Err(io::Error::other("injected failure"))
            }
        }
        let mut reader = BufReader::new(io::Read::chain(&b"1 "[..], Fails));
        let error = read_record::<2>(&mut reader, &mut Vec::new()).unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::Other);
    }
}
