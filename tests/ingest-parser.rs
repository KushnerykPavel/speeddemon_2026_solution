// Keep parser regression tests outside the independently scored track directory.
include!("../track-a/src/main.rs");

#[cfg(test)]
mod parser_tests {
    use super::*;
    use std::collections::BTreeMap;

    // Like the mapped input: `SLACK` zero bytes are readable past the end.
    fn leak(mut bytes: Vec<u8>) -> &'static [u8] {
        let length = bytes.len();
        bytes.resize(length + SLACK, 0);
        &Box::leak(bytes.into_boxed_slice())[..length]
    }

    fn run_all(input: &[u8], workers: usize) -> Result<Vec<u8>, String> {
        static NEXT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
        let n = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!("sd-ingest-test-{}-{n}", std::process::id()));
        let file = File::options()
            .read(true)
            .write(true)
            .create(true)
            .truncate(true)
            .open(&path)
            .unwrap();
        let result = summarize(leak(input.to_vec()), workers, &file).map_err(|e| e.to_string());
        let bytes = fs::read(&path).unwrap();
        fs::remove_file(&path).unwrap();
        result.map(|()| bytes)
    }

    // Straightforward reference: group, sort, nearest rank.
    fn expected(lines: &[(String, String, u32, u32)]) -> String {
        let mut groups: BTreeMap<(String, String), (Vec<u32>, u64)> = BTreeMap::new();
        for (timestamp, endpoint, status, latency) in lines {
            let group = groups
                .entry((timestamp[..13].to_string(), endpoint.clone()))
                .or_default();
            group.0.push(*latency);
            group.1 += u64::from((500..600).contains(status));
        }
        let mut out = String::new();
        for ((hour, endpoint), (mut values, errors)) in groups {
            values.sort_unstable();
            let n = values.len();
            let at = |p: usize| values[(p * n).div_ceil(100) - 1];
            let ppm = errors * 1_000_000 / n as u64;
            out += &format!(
                "{hour}:00:00Z {endpoint} {n} {} {} {} {errors} {ppm}\n",
                at(50),
                at(95),
                at(99)
            );
        }
        out
    }

    fn render(lines: &[(String, String, u32, u32)], ending: &str, final_ending: bool) -> Vec<u8> {
        let mut text = String::new();
        for (i, (timestamp, endpoint, status, latency)) in lines.iter().enumerate() {
            text += &format!("{timestamp} GET {endpoint} {status} {latency}");
            if i + 1 < lines.len() || final_ending {
                text += ending;
            }
        }
        text.into_bytes()
    }

    fn random_lines(count: usize, endpoints: usize, seed: u64) -> Vec<(String, String, u32, u32)> {
        let mut state = seed;
        let mut next = move || {
            state = state
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            (state >> 33) as u32
        };
        (0..count)
            .map(|_| {
                let hour = next() as usize % HOURS + 18;
                let (date, h) = (DATES[hour / 24], hour % 24);
                let endpoint = format!(
                    "/e{}{}",
                    next() as usize % endpoints,
                    "x".repeat(next() as usize % 40)
                );
                let latency = match next() % 20 {
                    0 => 0,
                    1 => u32::MAX,
                    _ => next() % 100_000,
                };
                let status = [200, 404, 500, 503, 599, 100][next() as usize % 6];
                (
                    format!("{date}T{h:02}:{:02}:30.123456Z", next() % 60),
                    endpoint,
                    status,
                    latency,
                )
            })
            .collect()
    }

    #[test]
    fn matches_reference_for_every_worker_count() {
        let lines = random_lines(3000, 40, 7);
        let want = expected(&lines);
        // The spec: every line, the last included, ends with a single LF.
        let input = render(&lines, "\n", true);
        for workers in [1, 2, 3, 5, 12] {
            assert_eq!(
                String::from_utf8(run_all(&input, workers).unwrap()).unwrap(),
                want
            );
        }
    }

    #[test]
    fn many_endpoints_grow_the_table_and_share_prefixes() {
        let lines = random_lines(20_000, 60, 11);
        let input = render(&lines, "\n", true);
        assert_eq!(
            String::from_utf8(run_all(&input, 4).unwrap()).unwrap(),
            expected(&lines)
        );
        let long = "x".repeat(125);
        let lines = vec![
            (
                "2026-11-01T00:00:00.000000Z".to_string(),
                format!("/{long}"),
                500,
                1,
            ),
            (
                "2026-11-01T00:00:00.000000Z".to_string(),
                format!("/{long}y"),
                200,
                2,
            ),
            (
                "2026-11-01T00:00:00.000000Z".to_string(),
                "/".to_string(),
                200,
                3,
            ),
            (
                "2026-11-01T00:00:00.000000Z".to_string(),
                "/a".to_string(),
                200,
                4,
            ),
            (
                "2026-11-01T00:00:00.000000Z".to_string(),
                "/aa".to_string(),
                200,
                5,
            ),
        ];
        assert_eq!(
            String::from_utf8(run_all(&render(&lines, "\n", true), 2).unwrap()).unwrap(),
            expected(&lines)
        );
    }

    #[test]
    fn names_with_equal_ends_stay_separate() {
        // Same length, first eight and last eight bytes: the table's hash cannot tell them apart.
        let names = [
            "/api/v2/imports/{version}/returns/{name}",
            "/api/v2/reports/{version}/ledgers/{name}",
        ];
        let lines: Vec<_> = (0..200)
            .map(|i| {
                (
                    "2026-11-01T00:00:00.000000Z".to_string(),
                    names[i % 2].to_string(),
                    200,
                    i as u32,
                )
            })
            .collect();
        for workers in [1, 4] {
            let got =
                String::from_utf8(run_all(&render(&lines, "\n", true), workers).unwrap()).unwrap();
            assert_eq!(got, expected(&lines));
            assert_eq!(got.lines().count(), 2);
        }
    }

    #[test]
    fn too_many_endpoints_is_an_error() {
        let lines: Vec<_> = (0..5000)
            .map(|i| {
                (
                    "2026-11-01T00:00:00.000000Z".to_string(),
                    format!("/e{i}"),
                    200,
                    1,
                )
            })
            .collect();
        assert!(run_all(&render(&lines, "\n", true), 3).is_err());
    }

    #[test]
    fn contract_boundary_hours_and_empty_input() {
        let lines: Vec<_> = [
            "2026-10-30T18:00:00.000000Z",
            "2026-10-31T00:00:00.000000Z",
            "2026-11-01T00:00:00.000000Z",
            "2026-11-02T17:59:59.999999Z",
        ]
        .map(|t| (t.to_string(), "/a".to_string(), 500, u32::MAX))
        .to_vec();
        assert_eq!(
            String::from_utf8(run_all(&render(&lines, "\n", true), 3).unwrap()).unwrap(),
            expected(&lines)
        );
        assert!(run_all(b"", 4).unwrap().is_empty());
    }

    #[test]
    fn chunks_cover_input_on_line_boundaries() {
        let input: Vec<u8> = (0..50)
            .flat_map(|i| format!("line{i}\n").into_bytes())
            .collect();
        for pieces in [1, 2, 3, 7, 12, 40, 400] {
            let chunks = line_chunks(&input, pieces);
            assert!(chunks.len() <= pieces);
            let mut at = 0;
            for &(start, end) in &chunks {
                assert_eq!(start, at);
                assert!(input[end - 1] == b'\n');
                at = end;
            }
            assert_eq!(at, input.len());
        }
        assert!(line_chunks(b"", 4).is_empty());
        assert_eq!(line_chunks(b"no newline", 4), [(0, 10)]);
    }
}
