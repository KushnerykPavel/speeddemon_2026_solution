// Keep parser regression tests outside the independently scored track directory.
include!("../track-a/src/main.rs");

#[cfg(test)]
mod parser_tests {
    use super::*;
    use std::io::{self, Cursor, Read};

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
    fn every_buffer_boundary_and_line_ending() {
        for ending in ["\n", "\r\n"] {
            for final_ending in ["", ending] {
                let input = format!(
                    "2026-10-31T23:59:59.999999Z GET /api 499 0{ending}\
                     2026-11-01T00:00:00.000000Z POST /api/v1 599 4294967295{ending}\
                     2026-10-31T23:00:00.000000Z GET /api 500 10{final_ending}"
                );
                // Every possible split of the first row, including CR|LF and digit splits.
                for capacity in 1..=input.len() + 1 {
                    let mut reader =
                        BufReader::with_capacity(capacity, Cursor::new(input.as_bytes()));
                    let groups = read_groups(&mut reader).unwrap();
                    assert_eq!(groups.len(), 2);
                    let first = &groups[b"2026-10-31T23"]["/api"];
                    assert_eq!(first.latencies, [0, 10]);
                    assert_eq!(first.errors, 1);
                    let second = &groups[b"2026-11-01T00"]["/api/v1"];
                    assert_eq!(second.latencies, [u32::MAX]);
                    assert_eq!(second.errors, 1);
                }
            }
        }
    }

    #[test]
    fn empty_input_and_blank_records() {
        let mut empty = BufReader::with_capacity(1, Cursor::new(b""));
        assert!(read_groups(&mut empty).unwrap().is_empty());
        for input in [b"\n".as_slice(), b"\r\n", b"incomplete"] {
            let mut reader = BufReader::with_capacity(1, Cursor::new(input));
            assert!(read_groups(&mut reader).is_err());
        }
    }

    #[test]
    fn record_longer_than_production_buffer() {
        let endpoint = format!("/{}", "x".repeat(128 * 1024));
        let input = format!(
            "2026-10-31T23:59:59.999999Z GET {endpoint} 500 7\r\n\
             2026-10-31T23:59:59.999999Z GET /next 600 8\n"
        );
        let mut reader = BufReader::with_capacity(64 * 1024, Cursor::new(input.as_bytes()));
        let groups = read_groups(&mut reader).unwrap();
        let endpoints = &groups[b"2026-10-31T23"];
        assert_eq!(endpoints.len(), 2);
        assert_eq!(endpoints[&endpoint].latencies, [7]);
        assert_eq!(endpoints["/next"].latencies, [8]);
    }

    #[test]
    fn read_errors_are_not_treated_as_eof() {
        struct BrokenReader;
        impl Read for BrokenReader {
            fn read(&mut self, _: &mut [u8]) -> io::Result<usize> {
                Err(io::Error::other("injected read failure"))
            }
        }
        let mut reader = BufReader::new(BrokenReader);
        let error = read_groups(&mut reader).err().unwrap();
        assert!(error.to_string().contains("injected read failure"));
    }
}
