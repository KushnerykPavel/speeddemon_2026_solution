#[allow(dead_code)]
mod traverse {
    include!("../track-d/src/main.rs");

    #[test]
    fn reordering_preserves_permutation_edges_incoming_and_all_distances() {
        let bytes = b"130 9\n5 64 0\n64 5 4294967295\n64 129 4294967295\n129 128 4294967295\n5 5 0\n66 67 1\n67 66 2\n5 64 7\n5 64 0\n";
        let graph = build_graph(bytes, 6).unwrap();
        let original: Vec<_> = (0..130)
            .flat_map(|source| (0..130).map(move |target| (source, target)))
            .collect();
        let expected = answer_queries(&graph, &original, 1);
        assert_eq!(expected[5 * 130 + 128], Some(2 * u64::from(u32::MAX)));
        let incoming = graph.incoming.clone();
        let old_edges: Vec<_> = (0..130)
            .flat_map(|source| {
                graph.edges[graph.offsets[source] as usize..graph.offsets[source + 1] as usize]
                    .iter()
                    .map(move |&(target, weight)| (source, target, weight))
            })
            .collect();
        let mut queries = original;
        let reordered = reorder_graph(graph, &mut queries, 6);
        let mapping: Vec<_> = (0..130)
            .map(|old| {
                let (source, target) = queries[old * 130 + old];
                assert_eq!(source, target);
                source
            })
            .collect();
        let mut permutation = mapping.clone();
        permutation.sort_unstable();
        assert_eq!(permutation, (0..130).collect::<Vec<_>>());
        let mut expected_edges: Vec<_> = old_edges
            .iter()
            .map(|&(source, target, weight)| (mapping[source], mapping[target as usize], weight))
            .collect();
        let mut actual_edges: Vec<_> = (0..130)
            .flat_map(|source| {
                reordered.edges
                    [reordered.offsets[source] as usize..reordered.offsets[source + 1] as usize]
                    .iter()
                    .map(move |&(target, weight)| (source, target as usize, weight))
            })
            .collect();
        expected_edges.sort_unstable();
        actual_edges.sort_unstable();
        assert_eq!(actual_edges, expected_edges);
        for (old, &new) in mapping.iter().enumerate() {
            assert_eq!(
                incoming[old / 64] >> (old % 64) & 1,
                reordered.incoming[new / 64] >> (new % 64) & 1
            );
        }
        for workers in [1, 2, 6, 12] {
            assert_eq!(answer_queries(&reordered, &queries, workers), expected);
        }
        let empty = reorder_graph(build_graph(b"0 0\n", 6).unwrap(), &mut [], 6);
        assert_eq!(empty.offsets, [0]);
        assert!(empty.edges.is_empty());
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
                let mut edges = graph.edges
                    [graph.offsets[node] as usize..graph.offsets[node + 1] as usize]
                    .to_vec();
                edges.sort_unstable();
                assert_eq!(&edges, expected);
            }
            assert_eq!(graph.incoming, [0b1111]);
        }
        for nodes in [0, 5, 65] {
            fs::write(&path, format!("{nodes} 0\n")).unwrap();
            let graph = read_graph(&path, 6).unwrap();
            assert_eq!(graph.offsets, vec![0; nodes + 1]);
            assert!(graph.edges.is_empty());
            assert!(graph.incoming.iter().all(|&word| word == 0));
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
                let count = parse_chunks(&chunks, |_| {
                    seen.fetch_add(1, Ordering::Relaxed);
                    Ok(())
                })
                .unwrap();
                assert_eq!(count, length + 1);
                assert_eq!(seen.into_inner(), length + 1);
            }
        }
        assert!(line_chunks(b"", 12).is_empty());
        for bytes in [&b"1 2 3\n\n"[..], &b"1 2 4294967296\n"[..], &b"1 2\n"[..]] {
            for workers in [1, 6, 12] {
                assert!(parse_chunks(&line_chunks(bytes, workers), |_| Ok(())).is_err());
            }
        }
    }

    #[test]
    fn parallel_scatter_preserves_edges_under_cursor_contention() {
        let count = 20_000;
        let mut bytes = format!("130 {count}\n").into_bytes();
        let mut expected = vec![Vec::new(); 130];
        for index in 0..count {
            let (source, target, weight) = (index % 3, index % 130, index as u32);
            writeln!(bytes, "{source} {target} {weight}").unwrap();
            expected[source].push((target as u32, weight));
        }
        for edges in &mut expected {
            edges.sort_unstable();
        }
        for workers in [1, 2, 4, 6, 12] {
            for _ in 0..3 {
                let graph = build_graph(&bytes, workers).unwrap();
                for (node, expected) in expected.iter().enumerate() {
                    let mut edges = graph.edges
                        [graph.offsets[node] as usize..graph.offsets[node + 1] as usize]
                        .to_vec();
                    edges.sort_unstable();
                    assert_eq!(&edges, expected);
                }
                assert_eq!(graph.incoming, [u64::MAX, u64::MAX, 3]);
            }
        }
    }

    #[test]
    fn parallel_answers_match_serial_with_skewed_work_and_duplicates() {
        let nodes = 130;
        let mut offsets: Vec<u32> = (0..nodes as u32).collect();
        offsets.push(nodes as u32 - 1);
        let graph = Graph {
            offsets,
            edges: (1..nodes as u32).map(|target| (target, u32::MAX)).collect(),
            incoming: vec![u64::MAX - 1, u64::MAX, 3],
        };
        let queries: Vec<_> = (0..1000)
            .map(|index| match index % 5 {
                0 => (0, 129),
                1 => (129, 0),
                2 => (index % nodes, index % nodes),
                3 => (3, 4),
                _ => (1, 64),
            })
            .collect();
        let wide = Mutex::new(None);
        let mut searcher = Searcher::new(nodes, &wide);
        let expected: Vec<_> = queries
            .iter()
            .map(|&(source, target)| searcher.shortest_path(&graph, source, target))
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
        let graph = Graph {
            offsets: vec![0, 1, 2, 2, 2],
            edges: vec![(1, 7), (2, 0)],
            incoming: vec![0b0110],
        };
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
            assert_eq!(searcher.shortest_path(&graph, source, target), expected);
            assert!(searcher.narrow.touched.is_empty());
            assert!(searcher.narrow.heap.is_empty());
            assert!(
                searcher
                    .narrow
                    .distances
                    .iter()
                    .all(|&distance| distance == u32::MAX)
            );
            assert!(wide.lock().unwrap().is_none());
        }
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
        let wide = Mutex::new(None);
        let mut searcher = Searcher::new(8, &wide);
        for _ in 0..2 {
            for &((source, target), expected) in &cases {
                assert_eq!(searcher.shortest_path(&graph, source, target), expected);
                assert!(searcher.narrow.touched.is_empty() && searcher.narrow.heap.is_empty());
                assert!(searcher.narrow.distances.iter().all(|&d| d == u32::MAX));
                if let Some(wide) = &*wide.lock().unwrap() {
                    assert!(wide.touched.is_empty() && wide.heap.is_empty());
                    assert!(wide.distances.iter().all(|&d| d == u64::MAX));
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
