// Keep parser regression tests outside the independently scored track directory.
include!("../track-a/src/main.rs");

#[cfg(test)]
mod parser_tests {
    use super::*;
    fn group<'a>(groups: &'a Groups, hour: &[u8; 13], endpoint: &str) -> &'a Group {
        &groups.rows[groups.hours[hour]][groups.endpoints[endpoint]]
    }

    #[test]
    fn numeric_boundaries_and_error_statuses() {
        for (status, error) in [(499, false), (500, true), (599, true), (600, false)] {
            for latency in [0, u32::MAX] {
                let line = format!("2026-10-31T23:59:59.999999Z GET /api {status} {latency}");
                let record = parse_record(line.as_bytes()).unwrap();
                assert_eq!(record.hour, *b"2026-10-31T23");
                assert_eq!(record.endpoint, "/api");
                assert_eq!(record.latency, latency);
                assert_eq!(record.error, error);
            }
        }
    }

    #[test]
    fn rejects_malformed_records() {
        for fields in [
            "GET /api 500",            // missing latency
            "GET /api 500 1 extra",    // extra field
            "GET /api x 1",            // invalid status
            "GET /api 65536 1",        // status overflow
            "GET /api 4294967296 1",   // parser overflow
            "GET /api 500 4294967296", // latency overflow
            "GET /api 500 -1",         // negative latency
            "GET /api 500 1.5",        // noninteger latency
            "GET /api 500 +1",         // nondecimal token
            "GET /api 500 ",           // empty latency
            "GET  500 1",              // empty endpoint
            " /api 500 1",             // empty method
        ] {
            let line = format!("2026-10-31T23:59:59.999999Z {fields}");
            assert!(parse_record(line.as_bytes()).is_err(), "{line:?}");
        }
        for line in [b"".as_slice(), b"short GET /api 500 1", b" GET /api 500 1"] {
            assert!(parse_record(line).is_err());
        }
        assert!(parse_record(b"2026-10-31T23:59:59.999999Z GET /\xff 500 1").is_err());
        assert!(parse_record(b"\xff026-10-31T23:59:59.999999Z GET /api 500 1").is_err());
    }

    #[test]
    fn line_endings_and_final_line() {
        for ending in ["\n", "\r\n"] {
            for final_ending in ["", ending] {
                let input = format!(
                    "2026-10-31T23:59:59.999999Z GET /api 499 0{ending}\
                     2026-11-01T00:00:00.000000Z POST /api/v1 599 4294967295{ending}\
                     2026-10-31T23:00:00.000000Z GET /api 500 10{final_ending}"
                );
                let groups = read_chunk(input.as_bytes()).unwrap();
                assert_eq!(groups.hours.len(), 2);
                let first = group(&groups, b"2026-10-31T23", "/api");
                assert_eq!(first.latencies, [0, 10]);
                assert_eq!(first.errors, 1);
                let second = group(&groups, b"2026-11-01T00", "/api/v1");
                assert_eq!(second.latencies, [u32::MAX]);
                assert_eq!(second.errors, 1);
            }
        }
    }

    #[test]
    fn chunks_cover_input_on_line_boundaries() {
        let input: Vec<u8> = (0..50)
            .flat_map(|i| format!("line{i}\n").into_bytes())
            .collect();
        for workers in [1, 2, 3, 7, 12, 40] {
            let chunks = line_chunks(&input, workers);
            assert!(chunks.len() <= workers.min(12));
            assert_eq!(chunks.concat(), input);
            assert!(chunks.iter().all(|c| c.ends_with(b"\n")));
        }
        assert!(line_chunks(b"", 4).is_empty());
        assert_eq!(line_chunks(b"no newline", 4), [b"no newline".as_slice()]);
    }

    #[test]
    fn parallel_returns_results_in_index_order() {
        for workers in [1, 3, 12] {
            assert_eq!(
                parallel(20, workers, |i| i * i),
                (0..20).map(|i| i * i).collect::<Vec<_>>()
            );
        }
        assert!(parallel(0, 4, |i| i).is_empty());
    }

    #[test]
    fn empty_input_and_blank_records() {
        assert!(read_chunk(b"").unwrap().hours.is_empty());
        for input in [b"\n".as_slice(), b"\r\n", b"incomplete"] {
            assert!(read_chunk(input).is_err());
        }
    }

    #[test]
    fn very_long_record() {
        let endpoint = format!("/{}", "x".repeat(128 * 1024));
        let input = format!(
            "2026-10-31T23:59:59.999999Z GET {endpoint} 500 7\r\n\
             2026-10-31T23:59:59.999999Z GET /next 600 8\n"
        );
        let groups = read_chunk(input.as_bytes()).unwrap();
        assert_eq!(groups.endpoints.len(), 2);
        assert_eq!(group(&groups, b"2026-10-31T23", &endpoint).latencies, [7]);
        assert_eq!(group(&groups, b"2026-10-31T23", "/next").latencies, [8]);
    }

    #[test]
    fn endpoint_ids_remain_valid_as_hours_and_endpoints_arrive() {
        let input = b"2026-11-01T00:00:00.000000Z GET /z 200 1\n\
                      2026-10-31T23:00:00.000000Z GET /a 500 2\n\
                      2026-11-01T00:00:00.000000Z GET /aa 599 3\n\
                      2026-11-01T00:00:00.000000Z GET /z 500 4\n\
                      2026-10-31T23:00:00.000000Z GET /aa 200 5\n";
        let groups = read_chunk(input).unwrap();
        assert_eq!(groups.endpoints.len(), 3);
        assert_eq!(groups.hours.len(), 2);
        assert_eq!(group(&groups, b"2026-11-01T00", "/z").latencies, [1, 4]);
        assert_eq!(group(&groups, b"2026-11-01T00", "/z").errors, 1);
        assert!(group(&groups, b"2026-11-01T00", "/a").latencies.is_empty());
        assert!(group(&groups, b"2026-10-31T23", "/z").latencies.is_empty());
        assert_eq!(group(&groups, b"2026-10-31T23", "/a").latencies, [2]);
        assert_eq!(group(&groups, b"2026-10-31T23", "/aa").latencies, [5]);
    }

    #[test]
    fn hour_cache_handles_runs_transitions_and_revisits() {
        let mut groups = Groups::default();
        // Include every-record misses, long runs, month rollover, and new endpoints.
        for (index, hour) in [23, 0, 23, 0, 0, 0, 23, 23, 0].into_iter().enumerate() {
            let timestamp = if hour == 23 {
                "2026-10-31T23"
            } else {
                "2026-11-01T00"
            };
            let endpoint = if index % 2 == 0 { "/a" } else { "/b" };
            let line = format!("{timestamp}:00:00.000000Z GET {endpoint} 500 {index}");
            add_record(&mut groups, line.as_bytes()).unwrap();
        }
        assert_eq!(groups.hours.len(), 2);
        assert_eq!(groups.rows.len(), 2);
        for (hour, endpoint, latencies) in [
            (b"2026-10-31T23", "/a", vec![0, 2, 6]),
            (b"2026-10-31T23", "/b", vec![7]),
            (b"2026-11-01T00", "/a", vec![4, 8]),
            (b"2026-11-01T00", "/b", vec![1, 3, 5]),
        ] {
            let group = group(&groups, hour, endpoint);
            assert_eq!(group.latencies, latencies);
            assert_eq!(group.errors, latencies.len() as u64);
        }
    }

    #[test]
    fn rejected_records_do_not_change_hour_cache_or_groups() {
        let mut groups = Groups::default();
        let valid = b"2026-10-31T23:00:00.000000Z GET /a 200 7";
        let invalid = b"2026-11-01T00:00:00.000000Z GET /new 500 4294967296";
        assert!(add_record(&mut groups, invalid).is_err());
        assert!(groups.last_hour.is_none());
        assert!(groups.hours.is_empty());
        add_record(&mut groups, valid).unwrap();
        let cached = groups.last_hour;
        assert!(add_record(&mut groups, invalid).is_err());
        assert_eq!(groups.last_hour, cached);
        assert_eq!(groups.hours.len(), 1);
        assert_eq!(groups.endpoints.len(), 1);
        add_record(&mut groups, valid).unwrap();
        assert_eq!(group(&groups, b"2026-10-31T23", "/a").latencies, [7, 7]);
    }
}
